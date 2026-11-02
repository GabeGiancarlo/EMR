//! Optional loopback whisper.cpp adapter. Not a hosted or production speech service.
use crate::{access, access::Actor, ApiError, AppState, Result};
use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};
use chrono::NaiveDate;
use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::sync::{Semaphore, SemaphorePermit};
use tokio::time::timeout;
use uuid::Uuid;

pub const MAX_WAV_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_TRANSCRIPT_BYTES: usize = 200_000;
pub const MAX_DURATION_SECS: u64 = 600;
pub const JOB_TIMEOUT_SECS: u64 = 120;
const SAMPLE_RATE: u32 = 16_000;
const INPUT_NAME: &str = "input.wav";
const OUTPUT_BASE: &str = "output";
const OUTPUT_NAME: &str = "output.txt";
const READ_ACTION: &str = "read";
const COMPLETE_ACTION: &str = "transcribe";
const CLIENT_SQL: &str = "SELECT c.primary_clinician_id, (CURRENT_TIMESTAMP AT TIME ZONE COALESCE(f.timezone, 'UTC'))::date AS today FROM clients c LEFT JOIN facilities f ON f.id=c.facility_id AND f.organization_id=c.organization_id WHERE c.id=$1 AND c.organization_id=$2 FOR SHARE OF c";
const CONSENT_SQL: &str = "SELECT kind, granted_on, expires_on, revoked_on FROM consents WHERE id=$1 AND organization_id=$2 AND client_id=$3 FOR SHARE";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WavError {
    TooLarge,
    NotRiffWave,
    ChunkBounds,
    NotPcm16Mono16k,
    Duration,
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TranscriptError {
    Empty,
    TooLarge,
    Utf8,
    Nul,
}

fn jobs() -> &'static Semaphore {
    static JOBS: std::sync::LazyLock<Semaphore> = std::sync::LazyLock::new(|| Semaphore::new(1));
    &JOBS
}

fn try_claim(sem: &Semaphore) -> std::result::Result<SemaphorePermit<'_>, ()> {
    sem.try_acquire().map_err(|_| ())
}
pub(crate) fn recognition_permit() -> Result<SemaphorePermit<'static>> {
    try_claim(jobs()).map_err(|_| busy())
}

pub(crate) fn configuration_label() -> &'static str {
    match configured_paths(
        &std::env::var("NEXUS_WHISPER_CLI").unwrap_or_default(),
        &std::env::var("NEXUS_WHISPER_MODEL").unwrap_or_default(),
    ) {
        Ok(_) => "configured",
        Err(()) => "unconfigured",
    }
}

fn configured_paths(cli: &str, model: &str) -> std::result::Result<(PathBuf, PathBuf), ()> {
    let cli = PathBuf::from(cli);
    let model = PathBuf::from(model);
    if cli.as_os_str().is_empty() || model.as_os_str().is_empty() {
        return Err(());
    }
    if !cli.is_absolute() || !model.is_absolute() || !cli.is_file() || !model.is_file() {
        return Err(());
    }
    Ok((cli, model))
}

fn load_configured() -> Result<(PathBuf, PathBuf)> {
    configured_paths(
        &std::env::var("NEXUS_WHISPER_CLI").unwrap_or_default(),
        &std::env::var("NEXUS_WHISPER_MODEL").unwrap_or_default(),
    )
    .map_err(|_| unconfigured())
}

fn unconfigured() -> ApiError {
    ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        code: "speech_unconfigured",
        message: "Local whisper.cpp is unconfigured. Set NEXUS_WHISPER_CLI and NEXUS_WHISPER_MODEL to absolute paths of an existing whisper.cpp CLI and model file on this machine, then restart the API. No hosted speech service is called.".into(),
    }
}

fn busy() -> ApiError {
    ApiError {
        status: StatusCode::TOO_MANY_REQUESTS,
        code: "speech_busy",
        message: "A local transcription is already running. Only one job is accepted at a time. Wait for it to finish, then try again.".into(),
    }
}

fn timed_out() -> ApiError {
    ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        code: "speech_timeout",
        message: "Local transcription exceeded 120 seconds and was stopped. Nothing was saved. Try a shorter recording.".into(),
    }
}

fn spawn_failed(kind: std::io::ErrorKind) -> ApiError {
    ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        code: "speech_failed",
        message: format!("The configured whisper.cpp CLI could not be started ({kind:?}). Confirm NEXUS_WHISPER_CLI is an executable absolute path and NEXUS_WHISPER_MODEL is a readable model file. No hosted fallback was used."),
    }
}

fn process_failed(status: std::process::ExitStatus) -> ApiError {
    let code = status
        .code()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "signal".into());
    ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        code: "speech_failed",
        message: format!("Local whisper.cpp exited unsuccessfully ({code}). Nothing was saved. Confirm the configured CLI and model on this machine. No hosted fallback was used."),
    }
}

fn missing_output() -> ApiError {
    ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        code: "speech_failed",
        message: "Local whisper.cpp did not write output.txt. Nothing was saved. No hosted fallback was used.".into(),
    }
}

fn temp_failed() -> ApiError {
    ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        code: "speech_failed",
        message: "A local temporary directory for transcription could not be prepared. Nothing was saved.".into(),
    }
}

fn transcript_failure(err: TranscriptError) -> ApiError {
    let message = match err {
        TranscriptError::Empty => "Local transcription produced no text. Nothing was saved.",
        TranscriptError::TooLarge => {
            "The local transcript exceeded 200000 bytes and was discarded."
        }
        TranscriptError::Utf8 | TranscriptError::Nul => {
            "The local transcript was not plain UTF-8 and was discarded."
        }
    };
    ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        code: "speech_failed",
        message: message.into(),
    }
}

fn wav_error(err: WavError) -> ApiError {
    match err {
        WavError::TooLarge => ApiError {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            code: "payload_too_large",
            message: "The recording exceeds 20 MiB.".into(),
        },
        other => ApiError::invalid(match other {
            WavError::TooLarge => unreachable!(),
            WavError::NotRiffWave => "Send a RIFF/WAVE file.",
            WavError::ChunkBounds => "The WAV chunk sizes do not fit inside the file.",
            WavError::NotPcm16Mono16k => "The recording must be PCM 16-bit, mono, 16000 Hz.",
            WavError::Duration => "The recording must be 10 minutes or shorter.",
            WavError::Empty => "The recording has no audio samples.",
        }),
    }
}

fn consent_required() -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        code: "consent_required",
        message: "An active recording consent for this client is required. It must have kind recording, not be revoked, and be within its granted and expiry dates.".into(),
    }
}

fn role_can_transcribe(role: &str) -> bool {
    matches!(role, "admin" | "clinical_lead" | "clinician")
}

fn clinician_assigned(role: &str, actor_id: Uuid, primary: Option<Uuid>) -> bool {
    role != "clinician" || primary == Some(actor_id)
}

fn recording_consent_active(
    kind: &str,
    granted: NaiveDate,
    expires: Option<NaiveDate>,
    revoked: Option<NaiveDate>,
    today: NaiveDate,
) -> bool {
    kind == "recording"
        && revoked.is_none()
        && granted <= today
        && expires.is_none_or(|day| day >= today)
}

fn command_argv(model: &Path) -> Vec<OsString> {
    vec![
        OsString::from("-m"),
        model.as_os_str().to_os_string(),
        OsString::from("-f"),
        OsString::from(INPUT_NAME),
        OsString::from("-otxt"),
        OsString::from("-of"),
        OsString::from(OUTPUT_BASE),
        OsString::from("-nt"),
    ]
}

fn transcript_body(transcript: String) -> Value {
    json!({
        "transcript": transcript,
        "engine": "whisper.cpp",
        "manual_review_required": true
    })
}

fn validate_wav(bytes: &[u8]) -> std::result::Result<u64, WavError> {
    validate_wav_with_limits(bytes, MAX_WAV_BYTES, MAX_DURATION_SECS)
}

fn validate_wav_with_limits(
    bytes: &[u8],
    max_bytes: usize,
    max_secs: u64,
) -> std::result::Result<u64, WavError> {
    if bytes.len() > max_bytes {
        return Err(WavError::TooLarge);
    }
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(WavError::NotRiffWave);
    }
    let riff_size = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let end = riff_size.checked_add(8).ok_or(WavError::ChunkBounds)?;
    if end != bytes.len() {
        return Err(WavError::ChunkBounds);
    }
    let mut offset = 12usize;
    let mut fmt = None;
    let mut data_len = None;
    while offset < end {
        if offset.checked_add(8).is_none_or(|next| next > end) {
            return Err(WavError::ChunkBounds);
        }
        let id = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let data_start = offset + 8;
        let data_end = data_start.checked_add(size).ok_or(WavError::ChunkBounds)?;
        if data_end > end {
            return Err(WavError::ChunkBounds);
        }
        let padded = if size % 2 == 1 {
            data_end.checked_add(1).ok_or(WavError::ChunkBounds)?
        } else {
            data_end
        };
        if padded > end {
            return Err(WavError::ChunkBounds);
        }
        match id {
            b"fmt " => {
                if fmt.is_some() || size < 16 {
                    return Err(if fmt.is_some() {
                        WavError::ChunkBounds
                    } else {
                        WavError::NotPcm16Mono16k
                    });
                }
                fmt = Some((
                    u16::from_le_bytes(bytes[data_start..data_start + 2].try_into().unwrap()),
                    u16::from_le_bytes(bytes[data_start + 2..data_start + 4].try_into().unwrap()),
                    u32::from_le_bytes(bytes[data_start + 4..data_start + 8].try_into().unwrap()),
                    u32::from_le_bytes(bytes[data_start + 8..data_start + 12].try_into().unwrap()),
                    u16::from_le_bytes(bytes[data_start + 12..data_start + 14].try_into().unwrap()),
                    u16::from_le_bytes(bytes[data_start + 14..data_start + 16].try_into().unwrap()),
                ));
            }
            b"data" => {
                if data_len.is_some() {
                    return Err(WavError::ChunkBounds);
                }
                data_len = Some(size as u64);
            }
            _ => {}
        }
        offset = padded;
    }
    if offset != end {
        return Err(WavError::ChunkBounds);
    }
    let (format, channels, rate, byte_rate, block, bits) = fmt.ok_or(WavError::NotPcm16Mono16k)?;
    let data_bytes = data_len.ok_or(WavError::NotPcm16Mono16k)?;
    if format != 1
        || channels != 1
        || rate != SAMPLE_RATE
        || bits != 16
        || block != 2
        || byte_rate != SAMPLE_RATE * 2
    {
        return Err(WavError::NotPcm16Mono16k);
    }
    if data_bytes == 0 {
        return Err(WavError::Empty);
    }
    if data_bytes % 2 != 0 {
        return Err(WavError::NotPcm16Mono16k);
    }
    if data_bytes / 2 > u64::from(rate) * max_secs {
        return Err(WavError::Duration);
    }
    Ok(data_bytes)
}

fn decode_transcript(bytes: &[u8]) -> std::result::Result<String, TranscriptError> {
    if bytes.len() > MAX_TRANSCRIPT_BYTES {
        return Err(TranscriptError::TooLarge);
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = std::str::from_utf8(bytes).map_err(|_| TranscriptError::Utf8)?;
    let stripped = text.trim_end_matches(['\n', '\r']);
    if stripped.trim().is_empty() {
        return Err(TranscriptError::Empty);
    }
    if stripped.contains('\0') {
        return Err(TranscriptError::Nul);
    }
    Ok(stripped.to_string())
}

fn is_owned_job_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    name.starts_with("nexus-whisper-")
        && !name.contains("..")
        && path.starts_with(std::env::temp_dir())
}

struct TempCleanup(PathBuf);
impl Drop for TempCleanup {
    fn drop(&mut self) {
        if is_owned_job_dir(&self.0) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

fn new_job_dir() -> PathBuf {
    std::env::temp_dir().join(format!("nexus-whisper-{}", Uuid::new_v4()))
}

fn header_uuid(req: &HttpRequest, name: &str) -> Result<Uuid> {
    req.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .and_then(|v| Uuid::parse_str(v).ok())
        .ok_or_else(|| {
            ApiError::invalid("x-nexus-client and x-nexus-consent headers must be UUIDs.")
        })
}

fn require_wav_content_type(req: &HttpRequest) -> Result<()> {
    let ok = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.to_ascii_lowercase().starts_with("audio/wav"));
    if ok {
        Ok(())
    } else {
        Err(ApiError::invalid(
            "Content-Type must be audio/wav and the body must be the WAV bytes.",
        ))
    }
}

async fn authorize(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    client_id: Uuid,
    consent_id: Uuid,
) -> Result<Actor> {
    // Re-read identity, role, assignment and consent under locks. The completion
    // audit shares this transaction, so a revoked consent or offboarded operator
    // cannot pass on the strength of the pre-transcription snapshot.
    let value: Value = sqlx::query_scalar(
        "SELECT to_jsonb(s) FROM staff s WHERE id=$1 AND organization_id=$2 AND active FOR SHARE",
    )
    .bind(actor.id)
    .bind(actor.org)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(ApiError::denied)?;
    let actor = Actor {
        id: actor.id,
        org: actor.org,
        role: value["role"].as_str().unwrap_or_default().into(),
        value,
    };
    if !role_can_transcribe(&actor.role) {
        return Err(ApiError::denied());
    }
    let client = sqlx::query(CLIENT_SQL)
        .bind(client_id)
        .bind(actor.org)
        .fetch_optional(&mut **tx)
        .await?;
    let Some(client) = client else {
        return Err(ApiError::missing());
    };
    let primary: Option<Uuid> = client.try_get("primary_clinician_id")?;
    let today: NaiveDate = client.try_get("today")?;
    if !clinician_assigned(&actor.role, actor.id, primary) {
        return Err(ApiError::denied());
    }
    let consent = sqlx::query(CONSENT_SQL)
        .bind(consent_id)
        .bind(actor.org)
        .bind(client_id)
        .fetch_optional(&mut **tx)
        .await?;
    let Some(consent) = consent else {
        return Err(consent_required());
    };
    let kind: String = consent.try_get("kind")?;
    let granted: NaiveDate = consent.try_get("granted_on")?;
    let expires: Option<NaiveDate> = consent.try_get("expires_on")?;
    let revoked: Option<NaiveDate> = consent.try_get("revoked_on")?;
    if recording_consent_active(&kind, granted, expires, revoked, today) {
        Ok(actor)
    } else {
        Err(consent_required())
    }
}

async fn audit_pair(
    pool: &PgPool,
    actor: &Actor,
    action: &str,
    client_id: Uuid,
    consent_id: Uuid,
) -> Result<()> {
    // Fixed labels and record ids only. Never pass audio, transcript, or consent scope.
    let mut tx = pool.begin().await?;
    let actor = authorize(&mut tx, actor, client_id, consent_id).await?;
    access::audit(&mut tx, &actor, action, "consents", Some(consent_id)).await?;
    access::audit(&mut tx, &actor, action, "clients", Some(client_id)).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn transcribe(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Bytes,
) -> Result<HttpResponse> {
    let actor = access::actor(&req, &state.pool).await?;
    if !role_can_transcribe(&actor.role) {
        return Err(ApiError::denied());
    }
    require_wav_content_type(&req)?;
    let client_id = header_uuid(&req, "x-nexus-client")?;
    let consent_id = header_uuid(&req, "x-nexus-consent")?;
    let mut tx = state.pool.begin().await?;
    authorize(&mut tx, &actor, client_id, consent_id).await?;
    tx.commit().await?;
    if body.len() > MAX_WAV_BYTES {
        return Err(wav_error(WavError::TooLarge));
    }
    validate_wav(&body).map_err(wav_error)?;
    let (cli, model) = load_configured()?;
    audit_pair(&state.pool, &actor, READ_ACTION, client_id, consent_id).await?;
    let _permit = try_claim(jobs()).map_err(|_| busy())?;
    let dir = new_job_dir();
    tokio::fs::create_dir(&dir).await.map_err(|err| {
        log::warn!("speech temp directory was not created ({:?})", err.kind());
        temp_failed()
    })?;
    let _cleanup = TempCleanup(dir.clone());
    tokio::fs::write(dir.join(INPUT_NAME), &body)
        .await
        .map_err(|err| {
            log::warn!("speech input was not staged ({:?})", err.kind());
            temp_failed()
        })?;
    let mut command = tokio::process::Command::new(&cli);
    command
        .current_dir(&dir)
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .args(command_argv(&model));
    let mut child = command.spawn().map_err(|err| {
        log::warn!("whisper.cpp could not be started ({:?})", err.kind());
        spawn_failed(err.kind())
    })?;
    let status = match timeout(Duration::from_secs(JOB_TIMEOUT_SECS), child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(err)) => {
            log::warn!("whisper.cpp wait failed ({:?})", err.kind());
            return Err(spawn_failed(err.kind()));
        }
        Err(_) => {
            let _ = child.kill().await;
            let _ = timeout(Duration::from_secs(2), child.wait()).await;
            log::warn!("whisper.cpp exceeded the local time limit");
            return Err(timed_out());
        }
    };
    if !status.success() {
        log::warn!("whisper.cpp exited unsuccessfully");
        return Err(process_failed(status));
    }
    let file = tokio::fs::File::open(dir.join(OUTPUT_NAME))
        .await
        .map_err(|err| {
            log::warn!("whisper.cpp output could not be read ({:?})", err.kind());
            missing_output()
        })?;
    let mut output = Vec::new();
    file.take(MAX_TRANSCRIPT_BYTES as u64 + 1)
        .read_to_end(&mut output)
        .await
        .map_err(|_| missing_output())?;
    let text = decode_transcript(&output).map_err(transcript_failure)?;
    audit_pair(&state.pool, &actor, COMPLETE_ACTION, client_id, consent_id).await?;
    Ok(HttpResponse::Ok()
        .insert_header(("cache-control", "no-store"))
        .json(transcript_body(text)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_chunk(buf: &mut Vec<u8>, id: &[u8; 4], data: &[u8]) {
        buf.extend(id);
        buf.extend(&u32::try_from(data.len()).unwrap().to_le_bytes());
        buf.extend(data);
        if data.len() % 2 == 1 {
            buf.push(0);
        }
    }

    fn wrap_wave(chunks: &[u8]) -> Vec<u8> {
        let mut file = Vec::new();
        file.extend(b"RIFF");
        file.extend(&u32::try_from(chunks.len() + 4).unwrap().to_le_bytes());
        file.extend(b"WAVE");
        file.extend(chunks);
        file
    }

    fn fmt_chunk(format: u16, channels: u16, rate: u32, bits: u16) -> Vec<u8> {
        let block = channels * (bits / 8);
        let byte_rate = rate * u32::from(block);
        let mut data = Vec::new();
        data.extend(format.to_le_bytes());
        data.extend(channels.to_le_bytes());
        data.extend(rate.to_le_bytes());
        data.extend(byte_rate.to_le_bytes());
        data.extend(block.to_le_bytes());
        data.extend(bits.to_le_bytes());
        let mut chunk = Vec::new();
        push_chunk(&mut chunk, b"fmt ", &data);
        chunk
    }

    fn wav_with(format: u16, channels: u16, rate: u32, bits: u16, samples: usize) -> Vec<u8> {
        let mut chunks = fmt_chunk(format, channels, rate, bits);
        let block = usize::from(channels * (bits / 8));
        push_chunk(&mut chunks, b"data", &vec![0u8; samples * block]);
        wrap_wave(&chunks)
    }

    fn valid_wav(samples: usize) -> Vec<u8> {
        wav_with(1, 1, 16_000, 16, samples)
    }

    #[test]
    fn accepts_pcm16_mono_16k_and_bounded_extra_chunks() {
        assert!(validate_wav(&valid_wav(1)).is_ok());
        let mut chunks = fmt_chunk(1, 1, 16_000, 16);
        push_chunk(&mut chunks, b"fact", &4u32.to_le_bytes());
        push_chunk(&mut chunks, b"data", &[0, 0]);
        push_chunk(&mut chunks, b"LIST", b"abc");
        assert!(validate_wav(&wrap_wave(&chunks)).is_ok());
    }

    #[test]
    fn rejects_non_wav_and_wrong_pcm_layout() {
        assert_eq!(
            validate_wav(b"not a wav file!!"),
            Err(WavError::NotRiffWave)
        );
        assert_eq!(
            validate_wav(&wav_with(3, 1, 16_000, 16, 1)),
            Err(WavError::NotPcm16Mono16k)
        );
        assert_eq!(
            validate_wav(&wav_with(1, 2, 16_000, 16, 1)),
            Err(WavError::NotPcm16Mono16k)
        );
        assert_eq!(
            validate_wav(&wav_with(1, 1, 44_100, 16, 1)),
            Err(WavError::NotPcm16Mono16k)
        );
        assert_eq!(
            validate_wav(&wav_with(1, 1, 16_000, 8, 1)),
            Err(WavError::NotPcm16Mono16k)
        );
        assert_eq!(
            validate_wav(&wav_with(1, 1, 16_000, 16, 0)),
            Err(WavError::Empty)
        );
    }

    #[test]
    fn inconsistent_byte_rate_is_rejected() {
        let mut wav = valid_wav(1);
        wav[28] ^= 0xff;
        assert_eq!(validate_wav(&wav), Err(WavError::NotPcm16Mono16k));
    }

    #[test]
    fn chunk_bounds_and_riff_size_are_enforced() {
        let mut chunks = fmt_chunk(1, 1, 16_000, 16);
        chunks.extend(b"data");
        chunks.extend(&100u32.to_le_bytes());
        chunks.extend(&[0, 0]);
        assert_eq!(
            validate_wav(&wrap_wave(&chunks)),
            Err(WavError::ChunkBounds)
        );

        let mut odd = fmt_chunk(1, 1, 16_000, 16);
        push_chunk(&mut odd, b"data", &[0, 0]);
        odd.extend(b"LIST");
        odd.extend(&3u32.to_le_bytes());
        odd.extend(b"abc");
        assert_eq!(validate_wav(&wrap_wave(&odd)), Err(WavError::ChunkBounds));

        let mut huge = wrap_wave(&[0u8; 8]);
        huge[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(validate_wav(&huge), Err(WavError::ChunkBounds));

        let mut extra = valid_wav(1);
        extra.push(0);
        assert_eq!(validate_wav(&extra), Err(WavError::ChunkBounds));
        let mut short = valid_wav(1);
        let n = u32::try_from(short.len() - 8).unwrap() + 50;
        short[4..8].copy_from_slice(&n.to_le_bytes());
        assert_eq!(validate_wav(&short), Err(WavError::ChunkBounds));

        let mut dup = fmt_chunk(1, 1, 16_000, 16);
        push_chunk(&mut dup, b"data", &[0, 0]);
        push_chunk(&mut dup, b"data", &[0, 0]);
        assert_eq!(validate_wav(&wrap_wave(&dup)), Err(WavError::ChunkBounds));
    }

    #[test]
    fn duration_limit_is_inclusive_and_body_limit_holds_ten_minutes() {
        assert!(validate_wav_with_limits(&valid_wav(16_000), usize::MAX, 1).is_ok());
        assert_eq!(
            validate_wav_with_limits(&valid_wav(16_001), usize::MAX, 1),
            Err(WavError::Duration)
        );
        assert_eq!(
            validate_wav_with_limits(&valid_wav(1), 10, 600),
            Err(WavError::TooLarge)
        );
        assert_eq!(MAX_WAV_BYTES, 20 * 1024 * 1024);
        assert_eq!(MAX_DURATION_SECS, 600);
        assert_eq!(JOB_TIMEOUT_SECS, 120);
        assert_eq!(MAX_TRANSCRIPT_BYTES, 200_000);
        let ten_minutes = 16_000u64 * 2 * MAX_DURATION_SECS;
        assert!(ten_minutes + 128 <= MAX_WAV_BYTES as u64);
        assert!(wav_error(WavError::Duration).message.contains("10 minutes"));
        assert!(!wav_error(WavError::ChunkBounds).message.contains("RIFF"));
    }

    #[test]
    fn transcript_is_plain_text_without_added_clinical_prose() {
        assert_eq!(
            decode_transcript(b"Patient denied pain.\n").unwrap(),
            "Patient denied pain."
        );
        assert_eq!(decode_transcript(b"  hello  \n").unwrap(), "  hello  ");
        assert!(!decode_transcript(b"hello there\n")
            .unwrap()
            .to_lowercase()
            .contains("speaker"));
        assert_eq!(decode_transcript(&[0xff, 0xfe]), Err(TranscriptError::Utf8));
        assert_eq!(
            decode_transcript(&vec![b'a'; 200_001]),
            Err(TranscriptError::TooLarge)
        );
        assert_eq!(
            decode_transcript(&vec![b'a'; 200_000]).unwrap().len(),
            200_000
        );
        assert_eq!(decode_transcript(b"\n"), Err(TranscriptError::Empty));
        assert_eq!(decode_transcript(b"ok\0"), Err(TranscriptError::Nul));
    }

    #[test]
    fn response_is_a_draft_without_speaker_or_signature_fields() {
        let body = transcript_body("hello".into());
        assert_eq!(body["engine"], "whisper.cpp");
        assert_eq!(body["manual_review_required"], true);
        assert_eq!(body["transcript"], "hello");
        assert!(body.get("speakers").is_none());
        assert!(body.get("speaker_labels").is_none());
        assert!(body.get("signed").is_none());
        assert!(body.get("diagnosis").is_none());
        assert!(body.get("note").is_none());
    }

    #[test]
    fn fixed_args_do_not_come_from_a_shell_or_request_text() {
        let text: Vec<String> = command_argv(Path::new("/opt/models/My Model;rm -rf.bin"))
            .into_iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            text,
            vec![
                "-m",
                "/opt/models/My Model;rm -rf.bin",
                "-f",
                "input.wav",
                "-otxt",
                "-of",
                "output",
                "-nt",
            ]
        );
        assert_eq!(text.len(), 8);
        assert_eq!(INPUT_NAME, "input.wav");
        assert_eq!(OUTPUT_BASE, "output");
        assert_eq!(OUTPUT_NAME, "output.txt");
    }

    #[test]
    fn configured_paths_require_two_absolute_files() {
        let dir = std::env::temp_dir().join(format!("nexus-whisper-cfg-{}", Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let cli = dir.join("whisper-cli");
        let model = dir.join("model.bin");
        std::fs::write(&cli, b"bin").unwrap();
        std::fs::write(&model, b"model").unwrap();
        assert!(configured_paths(cli.to_str().unwrap(), model.to_str().unwrap()).is_ok());
        assert!(configured_paths("whisper-cli", model.to_str().unwrap()).is_err());
        assert!(configured_paths(cli.to_str().unwrap(), "model.bin").is_err());
        assert!(configured_paths("", "").is_err());
        assert!(configured_paths("/no/such/whisper-cli", model.to_str().unwrap()).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cleanup_removes_only_its_job_directory() {
        let own = std::env::temp_dir().join(format!("nexus-whisper-{}", Uuid::new_v4()));
        let sibling = std::env::temp_dir().join(format!("nexus-keep-{}", Uuid::new_v4()));
        std::fs::create_dir(&own).unwrap();
        std::fs::create_dir(&sibling).unwrap();
        std::fs::write(own.join("input.wav"), b"x").unwrap();
        std::fs::write(own.join("output.txt"), b"draft").unwrap();
        {
            let _cleanup = TempCleanup(own.clone());
        }
        assert!(!own.exists());
        assert!(sibling.exists());
        std::fs::remove_dir_all(&sibling).unwrap();
    }

    #[test]
    fn transcription_is_not_granted_by_consent_write_role() {
        assert!(role_can_transcribe("admin"));
        assert!(role_can_transcribe("clinical_lead"));
        assert!(role_can_transcribe("clinician"));
        assert!(!role_can_transcribe("support"));
        assert!(!role_can_transcribe("billing"));
        assert!(!role_can_transcribe("executive"));
        assert!(!role_can_transcribe(""));
        assert!(crate::access::role_allows("support", "consents", true));
        assert!(!role_can_transcribe("support"));
    }

    #[test]
    fn clinician_assignment_and_recording_consent_rules() {
        let actor = Uuid::new_v4();
        let other = Uuid::new_v4();
        assert!(clinician_assigned("clinician", actor, Some(actor)));
        assert!(!clinician_assigned("clinician", actor, Some(other)));
        assert!(!clinician_assigned("clinician", actor, None));
        assert!(clinician_assigned("admin", actor, None));
        assert!(clinician_assigned("clinical_lead", actor, Some(other)));
        let today = NaiveDate::from_ymd_opt(2026, 4, 15).unwrap();
        assert!(recording_consent_active(
            "recording",
            today,
            None,
            None,
            today
        ));
        assert!(!recording_consent_active(
            "Recording",
            today,
            None,
            None,
            today
        ));
        assert!(!recording_consent_active(
            "recording",
            today,
            None,
            Some(today),
            today
        ));
        assert!(!recording_consent_active(
            "recording",
            NaiveDate::from_ymd_opt(2026, 4, 16).unwrap(),
            None,
            None,
            today
        ));
        assert!(!recording_consent_active(
            "recording",
            today,
            Some(NaiveDate::from_ymd_opt(2026, 4, 14).unwrap()),
            None,
            today
        ));
        assert!(recording_consent_active(
            "recording",
            today,
            Some(today),
            None,
            today
        ));
    }

    #[test]
    fn unconfigured_busy_and_timeout_are_actionable() {
        let err = unconfigured();
        assert_eq!(err.status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(err.message.contains("NEXUS_WHISPER_CLI"));
        assert!(err.message.contains("NEXUS_WHISPER_MODEL"));
        assert!(err.message.to_ascii_lowercase().contains("unconfigured"));
        assert!(!err
            .message
            .to_ascii_lowercase()
            .contains("production-ready"));
        assert_eq!(busy().status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(timed_out().status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(timed_out().message.contains("120"));
    }
}
