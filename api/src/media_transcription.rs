//! Local WebM decode (FFmpeg) and whisper.cpp JSON recognition.
//! Not a summarizer and not speaker identification. Clinicians must review every segment.
//! Source bytes, PCM, and JSON are plaintext in the temp directory until drop; a crash can
//! leave them, and removal is not a secure wipe. Dropping the future cancels kill_on_drop
//! children and then deletes only the unique job directory created here.

use crate::{ApiError, Result};
use actix_web::http::StatusCode;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::time::timeout;
use uuid::Uuid;

const MAX_INPUT_BYTES: usize = 256 * 1024 * 1024;
const MAX_DURATION_MS: i64 = 7_200_000;
const MAX_PARTS: usize = 24;
const DECODE_LIMIT_SECS: u64 = 7201;
const PART_SECS: u64 = 300;
const DECODE_TIMEOUT_SECS: u64 = 120;
const RECOG_TIMEOUT_SECS: u64 = 180;
const MAX_JSON_BYTES: usize = 1_048_576;
const MAX_SLICE_BYTES: usize = 7201 * 16_000 * 2 + 65_536;
const MAX_TEXT_CHARS: usize = 2_000;
const MAX_TEXT_BYTES: usize = 8_000;
const MAX_SEGMENTS_PER_SLICE: usize = 2_000;
// whisper.cpp can place the final end token beyond the PCM boundary (observed +388 ms).
// Normalize at most one second, mark any adjustment uncertain, and require human review.
const ROUND_TOLERANCE_MS: i64 = 1_000;
const NEAR_ZERO: i16 = 8;
const MAX_DIR_ENTRIES: usize = 64;
const ENGINE: &str = "whisper.cpp";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Segment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub uncertain: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Recognition {
    pub segments: Vec<Segment>,
    pub duration_ms: i64,
    pub engine: String,
    pub model_label: String,
    pub model_sha256: String,
}

#[derive(Debug)]
struct SliceMeta {
    index: u32,
    duration_ms: i64,
    silent: bool,
}

struct WavInfo {
    duration_ms: i64,
    silent: bool,
}

struct TempCleanup(PathBuf);

impl Drop for TempCleanup {
    fn drop(&mut self) {
        if is_owned_job_dir(&self.0)
            && !self.0.is_symlink()
            && std::fs::remove_dir_all(&self.0).is_err()
        {
            // kill_on_drop may still be releasing Windows handles. Retry only this owned path.
            let path = self.0.clone();
            let _ = std::thread::Builder::new()
                .name("nexus-media-cleanup".into())
                .spawn(move || {
                    for _ in 0..40 {
                        std::thread::sleep(Duration::from_millis(50));
                        if !is_owned_job_dir(&path)
                            || path.is_symlink()
                            || !path.exists()
                            || std::fs::remove_dir_all(&path).is_ok()
                        {
                            return;
                        }
                    }
                    log::warn!("Temporary media cleanup needs local review after process exit");
                });
        }
    }
}

fn err(status: StatusCode, code: &'static str, message: &'static str) -> ApiError {
    ApiError {
        status,
        code,
        message: message.into(),
    }
}

fn unconfigured() -> ApiError {
    err(
        StatusCode::SERVICE_UNAVAILABLE,
        "media_unconfigured",
        "Local transcription is unconfigured. Set NEXUS_FFMPEG_CLI, NEXUS_WHISPER_CLI, and NEXUS_WHISPER_MODEL to absolute paths of existing files, then restart the API. No hosted speech service is called.",
    )
}

fn too_large() -> ApiError {
    err(
        StatusCode::PAYLOAD_TOO_LARGE,
        "media_too_large",
        "The recording exceeds 256 MiB and was not transcribed.",
    )
}

fn too_long() -> ApiError {
    err(
        StatusCode::PAYLOAD_TOO_LARGE,
        "media_too_long",
        "The recording exceeds 2 hours or 24 five-minute parts. It was not truncated or transcribed.",
    )
}

fn empty_audio() -> ApiError {
    err(
        StatusCode::BAD_REQUEST,
        "media_invalid",
        "The recording has no audio samples. Nothing was transcribed.",
    )
}

fn decode_timed_out() -> ApiError {
    err(
        StatusCode::SERVICE_UNAVAILABLE,
        "media_timeout",
        "Local decode exceeded 120 seconds and was stopped. The recording was not transcribed.",
    )
}

fn recognize_timed_out() -> ApiError {
    err(
        StatusCode::SERVICE_UNAVAILABLE,
        "media_timeout",
        "Local recognition exceeded 180 seconds and was stopped. Nothing was saved.",
    )
}

fn decode_failed() -> ApiError {
    err(
        StatusCode::SERVICE_UNAVAILABLE,
        "media_failed",
        "FFmpeg could not decode the recording to PCM audio. Nothing was saved.",
    )
}

fn recognize_failed() -> ApiError {
    err(
        StatusCode::SERVICE_UNAVAILABLE,
        "media_failed",
        "Local recognition output was incomplete or outside the measured audio and was discarded.",
    )
}

fn output_bounded() -> ApiError {
    err(
        StatusCode::SERVICE_UNAVAILABLE,
        "media_failed",
        "Local recognition output exceeded the size bound and was discarded.",
    )
}

fn media_failed() -> ApiError {
    err(
        StatusCode::SERVICE_UNAVAILABLE,
        "media_failed",
        "Local media processing could not be completed. Nothing was saved.",
    )
}

fn temp_failed() -> ApiError {
    err(
        StatusCode::SERVICE_UNAVAILABLE,
        "media_failed",
        "A local temporary directory for transcription could not be prepared. Nothing was saved.",
    )
}

fn io_stage(io: &std::io::Error, stage: &'static str, fallback: ApiError) -> ApiError {
    log::warn!("media engine {stage} failed ({:?})", io.kind());
    fallback
}

fn reject_input(len: usize) -> Option<ApiError> {
    if len == 0 {
        Some(empty_audio())
    } else if len > MAX_INPUT_BYTES {
        Some(too_large())
    } else {
        None
    }
}

fn exceeds_recording_limit(part_count: usize, highest_index: u32, total_ms: i64) -> bool {
    part_count > MAX_PARTS || highest_index >= MAX_PARTS as u32 || total_ms > MAX_DURATION_MS
}

fn advance(offset: i64, slice_ms: i64) -> Option<i64> {
    if slice_ms < 0 {
        None
    } else {
        offset.checked_add(slice_ms)
    }
}

fn gated(silent: bool, parsed: Vec<Segment>) -> Vec<Segment> {
    if silent {
        Vec::new()
    } else {
        parsed
    }
}

fn contiguous(indexes: &[u32]) -> bool {
    !indexes.is_empty() && indexes.iter().enumerate().all(|(i, v)| *v == i as u32)
}

fn slice_index(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("slice-")?.strip_suffix(".wav")?;
    if rest.len() != 3 || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    rest.parse().ok()
}

pub(crate) fn configured() -> bool {
    match (
        env_path("NEXUS_FFMPEG_CLI"),
        env_path("NEXUS_WHISPER_CLI"),
        env_path("NEXUS_WHISPER_MODEL"),
    ) {
        (Ok(a), Ok(b), Ok(c)) => paths_configured(&a, &b, &c),
        _ => false,
    }
}

fn env_path(key: &str) -> std::result::Result<PathBuf, ()> {
    let raw = std::env::var(key).unwrap_or_default();
    if raw.is_empty() {
        Err(())
    } else {
        Ok(PathBuf::from(raw))
    }
}

fn paths_configured(ffmpeg: &Path, whisper: &Path, model: &Path) -> bool {
    absolute_file(ffmpeg) && absolute_file(whisper) && absolute_file(model)
}

fn absolute_file(path: &Path) -> bool {
    path.is_absolute() && path.is_file()
}

fn load_tools() -> Result<(PathBuf, PathBuf, PathBuf)> {
    let ffmpeg = env_path("NEXUS_FFMPEG_CLI").map_err(|_| unconfigured())?;
    let whisper = env_path("NEXUS_WHISPER_CLI").map_err(|_| unconfigured())?;
    let model = env_path("NEXUS_WHISPER_MODEL").map_err(|_| unconfigured())?;
    if paths_configured(&ffmpeg, &whisper, &model) {
        Ok((ffmpeg, whisper, model))
    } else {
        Err(unconfigured())
    }
}

fn ffmpeg_args() -> Vec<OsString> {
    [
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
        "-xerror",
        "-y",
        "-protocol_whitelist",
        "file,pipe",
        "-f",
        "webm",
        "-i",
        "source.webm",
        "-t",
        &DECODE_LIMIT_SECS.to_string(),
        "-vn",
        "-sn",
        "-dn",
        "-map",
        "0:a:0",
        "-ar",
        "16000",
        "-ac",
        "1",
        "-sample_fmt",
        "s16",
        "-c:a",
        "pcm_s16le",
        "-f",
        "segment",
        "-segment_time",
        &PART_SECS.to_string(),
        "-segment_format",
        "wav",
        "-reset_timestamps",
        "1",
        "slice-%03d.wav",
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

fn whisper_args(model: &Path, wav: &str, out_base: &str) -> Vec<OsString> {
    vec![
        OsString::from("-m"),
        model.as_os_str().to_os_string(),
        OsString::from("-f"),
        OsString::from(wav),
        OsString::from("-l"),
        OsString::from("en"),
        OsString::from("-oj"),
        OsString::from("-of"),
        OsString::from(out_base),
    ]
}

fn model_label(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty() && s.len() <= 128 && !s.contains(['/', '\\']))
        .unwrap_or("whisper-model")
        .to_string()
}

fn assemble(segments: Vec<Segment>, duration_ms: i64, label: String, sha: String) -> Recognition {
    Recognition {
        segments,
        duration_ms,
        engine: ENGINE.into(),
        model_label: label,
        model_sha256: sha,
    }
}

fn duration_ms_from_samples(samples: u64) -> Option<i64> {
    i64::try_from(samples.checked_mul(1_000)?.checked_div(16_000)?).ok()
}

fn wav_info(bytes: &[u8]) -> std::result::Result<WavInfo, ()> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(());
    }
    let declared = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let end = declared.checked_add(8).ok_or(())?;
    if end != bytes.len() {
        return Err(());
    }
    let mut offset = 12usize;
    let mut format = None;
    let mut data: Option<&[u8]> = None;
    while offset < end {
        if offset + 8 > end {
            return Err(());
        }
        let id = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let start = offset + 8;
        let stop = start.checked_add(size).ok_or(())?;
        if stop > end {
            return Err(());
        }
        let next = stop + usize::from(size % 2 == 1);
        if next > end {
            return Err(());
        }
        if id == b"fmt " {
            if format.is_some() || size < 16 {
                return Err(());
            }
            format = Some((
                u16::from_le_bytes(bytes[start..start + 2].try_into().unwrap()),
                u16::from_le_bytes(bytes[start + 2..start + 4].try_into().unwrap()),
                u32::from_le_bytes(bytes[start + 4..start + 8].try_into().unwrap()),
                u16::from_le_bytes(bytes[start + 14..start + 16].try_into().unwrap()),
            ));
        } else if id == b"data" {
            if data.is_some() {
                return Err(());
            }
            data = Some(&bytes[start..stop]);
        }
        offset = next;
    }
    if offset != end {
        return Err(());
    }
    let (fmt, channels, rate, bits) = format.ok_or(())?;
    let data = data.ok_or(())?;
    if fmt != 1 || channels != 1 || rate != 16_000 || bits != 16 || data.len() % 2 != 0 {
        return Err(());
    }
    let duration_ms = duration_ms_from_samples((data.len() / 2) as u64).ok_or(())?;
    // `data.len()` is even (checked above), so there is no remainder to consider.
    let (samples, _) = data.as_chunks::<2>();
    let silent = samples
        .iter()
        .all(|c| i16::from_le_bytes(*c).saturating_abs() <= NEAR_ZERO);
    Ok(WavInfo {
        duration_ms,
        silent,
    })
}

fn json_ms(value: Option<&Value>) -> Option<i64> {
    let number = value?.as_number()?;
    if let Some(v) = number.as_i64() {
        return Some(v);
    }
    let f = number.as_f64()?;
    if f.is_finite() && (-86_400_000.0..=86_400_000.0).contains(&f) {
        Some(f.round() as i64)
    } else {
        None
    }
}

fn bound_text(raw: &str) -> std::result::Result<(String, bool), ()> {
    if raw.len() > MAX_TEXT_BYTES || raw.contains('\0') {
        return Err(());
    }
    let text = raw.trim();
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(());
    }
    let bytes = text.as_bytes();
    let wrapped = bytes.len() >= 2
        && matches!(
            (bytes[0], *bytes.last().unwrap_or(&0)),
            (b'[', b']') | (b'(', b')') | (b'<', b'>')
        );
    Ok((text.to_string(), text.is_empty() || wrapped))
}

fn place(from: i64, to: i64, slice_ms: i64, offset_ms: i64) -> Result<(i64, i64)> {
    let limit = slice_ms.saturating_add(ROUND_TOLERANCE_MS);
    if slice_ms < 0 || to < from || from < -100 || from > limit || to > limit {
        return Err(recognize_failed());
    }
    let start = from.clamp(0, slice_ms);
    let end = to.clamp(0, slice_ms);
    if end < start {
        return Err(recognize_failed());
    }
    Ok((
        offset_ms.checked_add(start).ok_or_else(recognize_failed)?,
        offset_ms.checked_add(end).ok_or_else(recognize_failed)?,
    ))
}

fn parse_whisper_json(bytes: &[u8], slice_ms: i64, offset_ms: i64) -> Result<Vec<Segment>> {
    if bytes.len() > MAX_JSON_BYTES {
        return Err(output_bounded());
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let value: Value = serde_json::from_slice(bytes).map_err(|_| recognize_failed())?;
    let obj = value.as_object().ok_or_else(recognize_failed)?;
    let items = match obj.get("transcription") {
        None | Some(Value::Null) => return Err(recognize_failed()),
        Some(Value::Array(items)) => items,
        Some(_) => return Err(recognize_failed()),
    };
    let mut out = Vec::new();
    let mut prev_end = offset_ms;
    for item in items {
        let item = item.as_object().ok_or_else(recognize_failed)?;
        let offsets = item
            .get("offsets")
            .and_then(Value::as_object)
            .ok_or_else(recognize_failed)?;
        let from = json_ms(offsets.get("from")).ok_or_else(recognize_failed)?;
        let to = json_ms(offsets.get("to")).ok_or_else(recognize_failed)?;
        let (start, end) = place(from, to, slice_ms, offset_ms)?;
        if start < prev_end {
            return Err(recognize_failed());
        }
        let raw = match item.get("text") {
            None | Some(Value::Null) => "",
            Some(Value::String(text)) => text.as_str(),
            Some(_) => return Err(recognize_failed()),
        };
        let (text, uncertain) = bound_text(raw).map_err(|_| recognize_failed())?;
        prev_end = end;
        out.push(Segment {
            start_ms: start,
            end_ms: end,
            text,
            uncertain: uncertain || from < 0 || to > slice_ms || from > slice_ms,
        });
        if out.len() > MAX_SEGMENTS_PER_SLICE {
            return Err(output_bounded());
        }
    }
    Ok(out)
}

fn is_owned_job_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    name.strip_prefix("nexus-media-")
        .is_some_and(|suffix| Uuid::parse_str(suffix).is_ok())
        && path.parent() == Some(std::env::temp_dir().as_path())
}

fn new_job_dir() -> PathBuf {
    std::env::temp_dir().join(format!("nexus-media-{}", Uuid::new_v4()))
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

fn hash_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).map_err(|e| io_stage(&e, "model", media_failed()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65_536];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| io_stage(&e, "model", media_failed()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(to_hex(&hasher.finalize()))
}

async fn hash_model(path: &Path) -> Result<String> {
    let path = path.to_path_buf();
    match tokio::task::spawn_blocking(move || hash_file(&path)).await {
        Ok(result) => result,
        Err(_) => Err(media_failed()),
    }
}

async fn read_capped(
    path: &Path,
    max: usize,
    missing: fn() -> ApiError,
    overflow: fn() -> ApiError,
) -> Result<Vec<u8>> {
    if path.is_symlink() {
        return Err(missing());
    }
    let file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(io) => return Err(io_stage(&io, "read", missing())),
    };
    let mut buf = Vec::new();
    file.take(max as u64 + 1)
        .read_to_end(&mut buf)
        .await
        .map_err(|e| io_stage(&e, "read", missing()))?;
    if buf.len() > max {
        Err(overflow())
    } else {
        Ok(buf)
    }
}

async fn run_limited(
    cmd: &mut tokio::process::Command,
    secs: u64,
    on_timeout: fn() -> ApiError,
    on_fail: fn() -> ApiError,
) -> Result<()> {
    cmd.kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(io) => {
            log::warn!("media subprocess did not start ({:?})", io.kind());
            return Err(on_fail());
        }
    };
    match timeout(Duration::from_secs(secs), child.wait()).await {
        Ok(Ok(status)) if status.success() => Ok(()),
        Ok(Ok(_)) => {
            log::warn!("media subprocess exited unsuccessfully");
            Err(on_fail())
        }
        Ok(Err(io)) => {
            log::warn!("media subprocess wait failed ({:?})", io.kind());
            Err(on_fail())
        }
        Err(_) => {
            log::warn!("media subprocess exceeded its time limit");
            let _ = child.kill().await;
            let _ = timeout(Duration::from_secs(2), child.wait()).await;
            Err(on_timeout())
        }
    }
}

async fn collect_slices(dir: &Path) -> Result<Vec<SliceMeta>> {
    let mut indexes = Vec::new();
    let mut seen = 0usize;
    let mut rd = tokio::fs::read_dir(dir)
        .await
        .map_err(|e| io_stage(&e, "scan", media_failed()))?;
    while let Some(ent) = rd
        .next_entry()
        .await
        .map_err(|e| io_stage(&e, "scan", media_failed()))?
    {
        seen += 1;
        if seen > MAX_DIR_ENTRIES {
            return Err(media_failed());
        }
        let Some(name) = ent.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if let Some(idx) = slice_index(&name) {
            // A 25th part (index >= 24) proves the 7201s decode was too long.
            if exceeds_recording_limit(indexes.len() + 1, idx, 0) {
                return Err(too_long());
            }
            indexes.push(idx);
        }
    }
    indexes.sort_unstable();
    if !contiguous(&indexes) {
        return Err(decode_failed());
    }
    let mut slices = Vec::with_capacity(indexes.len());
    let mut total = 0i64;
    for idx in indexes {
        let path = dir.join(format!("slice-{idx:03}.wav"));
        let bytes = read_capped(&path, MAX_SLICE_BYTES, decode_failed, too_long).await?;
        let info = wav_info(&bytes).map_err(|_| decode_failed())?;
        total = advance(total, info.duration_ms).ok_or_else(too_long)?;
        if exceeds_recording_limit(slices.len() + 1, idx, total) {
            return Err(too_long());
        }
        slices.push(SliceMeta {
            index: idx,
            duration_ms: info.duration_ms,
            silent: info.silent,
        });
    }
    if total <= 0 {
        return Err(empty_audio());
    }
    Ok(slices)
}

async fn recognize_slice(whisper: &Path, model: &Path, dir: &Path, index: u32) -> Result<Vec<u8>> {
    let wav = format!("slice-{index:03}.wav");
    let base = format!("r{index:03}");
    let json_name = format!("{base}.json");
    let mut cmd = tokio::process::Command::new(whisper);
    cmd.current_dir(dir).args(whisper_args(model, &wav, &base));
    run_limited(
        &mut cmd,
        RECOG_TIMEOUT_SECS,
        recognize_timed_out,
        recognize_failed,
    )
    .await?;
    read_capped(
        &dir.join(json_name),
        MAX_JSON_BYTES,
        recognize_failed,
        output_bounded,
    )
    .await
}

pub(crate) async fn recognize(media: &[u8]) -> crate::Result<Recognition> {
    if let Some(rejected) = reject_input(media.len()) {
        return Err(rejected);
    }
    let (ffmpeg, whisper, model) = load_tools()?;
    let label = model_label(&model);
    let model_sha256 = hash_model(&model).await?;
    let dir = new_job_dir();
    let builder = tokio::fs::DirBuilder::new();
    #[cfg(unix)]
    let builder = {
        let mut builder = builder;
        builder.mode(0o700);
        builder
    };
    builder
        .create(&dir)
        .await
        .map_err(|e| io_stage(&e, "temp", temp_failed()))?;
    let _cleanup = TempCleanup(dir.clone());
    tokio::fs::write(dir.join("source.webm"), media)
        .await
        .map_err(|e| io_stage(&e, "temp", temp_failed()))?;
    let mut decode = tokio::process::Command::new(&ffmpeg);
    decode.current_dir(&dir).args(ffmpeg_args());
    run_limited(
        &mut decode,
        DECODE_TIMEOUT_SECS,
        decode_timed_out,
        decode_failed,
    )
    .await?;
    let slices = collect_slices(&dir).await?;
    let measured_ms = slices
        .iter()
        .try_fold(0i64, |acc, slice| advance(acc, slice.duration_ms))
        .ok_or_else(too_long)?;
    let highest = slices.last().map(|s| s.index).unwrap_or(0);
    if exceeds_recording_limit(slices.len(), highest, measured_ms) {
        return Err(too_long());
    }
    let mut segments = Vec::new();
    let mut offset = 0i64;
    for slice in &slices {
        let parsed = if slice.silent {
            Vec::new()
        } else {
            let json = recognize_slice(&whisper, &model, &dir, slice.index).await?;
            parse_whisper_json(&json, slice.duration_ms, offset)?
        };
        segments.extend(gated(slice.silent, parsed));
        if segments.len() > 10000
            || serde_json::to_vec(&segments)
                .map_err(|_| output_bounded())?
                .len()
                > 600000
        {
            return Err(output_bounded());
        }
        offset = advance(offset, slice.duration_ms).ok_or_else(too_long)?;
    }
    if offset != measured_ms {
        return Err(media_failed());
    }
    Ok(assemble(segments, measured_ms, label, model_sha256))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_of(raw: &[OsString]) -> Vec<String> {
        raw.iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect()
    }

    fn push_chunk(buf: &mut Vec<u8>, id: &[u8; 4], data: &[u8]) {
        buf.extend(id);
        buf.extend(&u32::try_from(data.len()).unwrap().to_le_bytes());
        buf.extend(data);
        if data.len() % 2 == 1 {
            buf.push(0);
        }
    }

    fn pcm_wav(samples: &[i16]) -> Vec<u8> {
        let mut data = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            data.extend(sample.to_le_bytes());
        }
        let mut fmt = Vec::new();
        fmt.extend(&1u16.to_le_bytes());
        fmt.extend(&1u16.to_le_bytes());
        fmt.extend(&16_000u32.to_le_bytes());
        fmt.extend(&32_000u32.to_le_bytes());
        fmt.extend(&2u16.to_le_bytes());
        fmt.extend(&16u16.to_le_bytes());
        let mut chunks = Vec::new();
        push_chunk(&mut chunks, b"fmt ", &fmt);
        push_chunk(&mut chunks, b"data", &data);
        let mut file = Vec::new();
        file.extend(b"RIFF");
        file.extend(&u32::try_from(chunks.len() + 4).unwrap().to_le_bytes());
        file.extend(b"WAVE");
        file.extend(chunks);
        file
    }

    fn assert_static(error: &ApiError) {
        assert!(!error.message.contains('/'));
        assert!(!error.message.contains('\\'));
        assert!(!error.message.contains("source.webm"));
        assert!(!error.message.contains("LEAKME"));
        assert!(error
            .code
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '_'));
    }

    #[test]
    fn ffmpeg_argv_decodes_7201_seconds_without_a_two_hour_cap() {
        let got = args_of(&ffmpeg_args());
        assert_eq!(
            got,
            vec![
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostdin",
                "-xerror",
                "-y",
                "-protocol_whitelist",
                "file,pipe",
                "-f",
                "webm",
                "-i",
                "source.webm",
                "-t",
                "7201",
                "-vn",
                "-sn",
                "-dn",
                "-map",
                "0:a:0",
                "-ar",
                "16000",
                "-ac",
                "1",
                "-sample_fmt",
                "s16",
                "-c:a",
                "pcm_s16le",
                "-f",
                "segment",
                "-segment_time",
                "300",
                "-segment_format",
                "wav",
                "-reset_timestamps",
                "1",
                "slice-%03d.wav",
            ]
        );
        assert!(!got
            .iter()
            .any(|a| a == "7200" || a == "sh" || a == "cmd" || a == "-c"));
        assert_eq!(DECODE_TIMEOUT_SECS, 120);
        assert_eq!(MAX_INPUT_BYTES, 256 * 1024 * 1024);
    }

    #[test]
    fn whisper_argv_is_fixed_json_english_without_vad_or_translation() {
        let model = Path::new("/opt/models/My Model;rm -rf.bin");
        let got = args_of(&whisper_args(model, "slice-003.wav", "r003"));
        assert_eq!(
            got,
            vec![
                "-m",
                "/opt/models/My Model;rm -rf.bin",
                "-f",
                "slice-003.wav",
                "-l",
                "en",
                "-oj",
                "-of",
                "r003",
            ]
        );
        for banned in [
            "-tr",
            "--translate",
            "-vad",
            "--vad",
            "-di",
            "--diarize",
            "-nt",
            "-t",
        ] {
            assert!(!got.iter().any(|a| a == banned), "{banned}");
        }
        assert_eq!(RECOG_TIMEOUT_SECS, 180);
    }

    #[test]
    fn configured_paths_require_three_absolute_files() {
        let dir = std::env::temp_dir().join(format!("nexus-media-cfg-{}", Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let ffmpeg = dir.join("ffmpeg");
        let whisper = dir.join("whisper");
        let model = dir.join("model.bin");
        std::fs::write(&ffmpeg, b"bin").unwrap();
        std::fs::write(&whisper, b"bin").unwrap();
        std::fs::write(&model, b"model").unwrap();
        assert!(paths_configured(&ffmpeg, &whisper, &model));
        assert!(!paths_configured(Path::new("ffmpeg"), &whisper, &model));
        assert!(!paths_configured(&ffmpeg, &whisper, Path::new("model.bin")));
        assert!(!paths_configured(&dir, &whisper, &model));
        assert!(!paths_configured(
            Path::new("/no/such/ffmpeg"),
            &whisper,
            &model
        ));
        let _ = configured();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn wav_duration_and_near_zero_signal_are_measured() {
        assert_eq!(duration_ms_from_samples(16_000), Some(1_000));
        assert_eq!(duration_ms_from_samples(4_800_000), Some(300_000));
        assert_eq!(duration_ms_from_samples(7_200 * 16_000), Some(7_200_000));
        assert!(duration_ms_from_samples(7_201 * 16_000).unwrap() > MAX_DURATION_MS);
        let silent = wav_info(&pcm_wav(&[0, 8, -8, NEAR_ZERO])).unwrap();
        assert!(silent.silent);
        assert_eq!(silent.duration_ms, 0);
        let second = wav_info(&pcm_wav(&vec![0i16; 16_000])).unwrap();
        assert!(second.silent);
        assert_eq!(second.duration_ms, 1_000);
        let loud = wav_info(&pcm_wav(&[0, 9, i16::MIN])).unwrap();
        assert!(!loud.silent);
        let mut odd = pcm_wav(&[0]);
        odd.pop();
        let n = u32::try_from(odd.len() - 8).unwrap() - 1;
        odd[4..8].copy_from_slice(&n.to_le_bytes());
        odd.pop();
        assert!(wav_info(&odd).is_err());
        let mut rate = pcm_wav(&[1]);
        rate[24..28].copy_from_slice(&44_100u32.to_le_bytes());
        assert!(wav_info(&rate).is_err());
    }

    #[test]
    fn offsets_are_milliseconds_clamped_ordered_and_uncertain() {
        let json = br#"{"systeminfo":"C:\\secret\\model.bin","transcription":[{"offsets":{"from":0,"to":5000},"text":" Hello "},{"offsets":{"from":5000,"to":5100},"text":"[BLANK_AUDIO]"}]}"#;
        let segs = parse_whisper_json(json, 5_000, 300_000).unwrap();
        assert_eq!(segs[0].start_ms, 300_000);
        assert_eq!(segs[0].end_ms, 305_000);
        assert_eq!(segs[0].text, "Hello");
        assert!(!segs[0].uncertain);
        assert_ne!(segs[0].end_ms, 350_000);
        assert!(segs[1].uncertain);
        assert_eq!(segs[1].text, "[BLANK_AUDIO]");
        assert_eq!(segs[1].end_ms, 305_000);
        let clamped = parse_whisper_json(
            br#"{"transcription":[{"offsets":{"from":-40,"to":1100},"text":"(noise)"}]}"#,
            1_000,
            0,
        )
        .unwrap();
        assert_eq!((clamped[0].start_ms, clamped[0].end_ms), (0, 1_000));
        assert!(clamped[0].uncertain);
        assert!(parse_whisper_json(
            br#"{"transcription":[{"offsets":{"from":0,"to":2001},"text":"x"}]}"#,
            1_000,
            0,
        )
        .is_err());
        assert!(parse_whisper_json(
            br#"{"transcription":[{"offsets":{"from":100,"to":200},"text":"b"},{"offsets":{"from":50,"to":80},"text":"a"}]}"#,
            1_000,
            0,
        )
        .is_err());
        assert!(parse_whisper_json(b"{}", 1_000, 0).is_err());
        assert!(parse_whisper_json(br#"{"transcription":[]}"#, 9, 0)
            .unwrap()
            .is_empty());
        assert!(parse_whisper_json(br#"{"transcription":null}"#, 9, 0).is_err());
        let leaked = parse_whisper_json(
            br#"{"transcription":[{"offsets":{"from":"0","to":"1"},"text":"LEAKME"}]}"#,
            10,
            0,
        )
        .unwrap_err();
        assert_static(&leaked);
        let empty = parse_whisper_json(
            br#"{"transcription":[{"offsets":{"from":0,"to":10},"text":"   "}]}"#,
            10,
            0,
        )
        .unwrap();
        assert!(empty[0].uncertain && empty[0].text.is_empty());
        let angle = parse_whisper_json(
            br#"{"transcription":[{"offsets":{"from":0.2,"to":10.4},"text":"<|nospeech|>"}]}"#,
            10,
            5,
        )
        .unwrap();
        assert_eq!(angle[0].start_ms, 5);
        assert!(angle[0].uncertain);
        let mixed = parse_whisper_json(
            br#"{"transcription":[{"offsets":{"from":0,"to":10},"text":"hello (cough)"}]}"#,
            10,
            0,
        )
        .unwrap();
        assert!(!mixed[0].uncertain);
        let big = "a".repeat(2001);
        let body =
            format!(r#"{{"transcription":[{{"offsets":{{"from":0,"to":1}},"text":"{big}"}}]}}"#);
        assert!(parse_whisper_json(body.as_bytes(), 10, 0).is_err());
        let ok = "b".repeat(2000);
        let body =
            format!(r#"{{"transcription":[{{"offsets":{{"from":0,"to":1}},"text":"{ok}"}}]}}"#);
        assert_eq!(
            parse_whisper_json(body.as_bytes(), 10, 0).unwrap()[0]
                .text
                .len(),
            2000
        );
    }

    #[test]
    fn real_whisper_end_token_overrun_is_bounded_and_explicitly_uncertain() {
        let segments=parse_whisper_json(br#"{"transcription":[{"offsets":{"from":299400,"to":300400},"text":"Synthetic end phrase"}]}"#,300012,0).unwrap();
        assert_eq!(segments[0].end_ms, 300012);
        assert!(segments[0].uncertain);
        assert!(parse_whisper_json(br#"{"transcription":[{"offsets":{"from":299400,"to":301013},"text":"Outside boundary"}]}"#,300012,0).is_err());
    }

    #[test]
    fn extra_part_or_measured_overrun_fails_closed() {
        assert!(!exceeds_recording_limit(24, 23, 7_200_000));
        assert!(exceeds_recording_limit(25, 24, 7_200_000));
        assert!(exceeds_recording_limit(24, 23, 7_200_001));
        assert!(exceeds_recording_limit(1, 0, 7_201_000));
        assert!(exceeds_recording_limit(1, 24, 1));
        assert!(too_long().message.contains("not truncated"));
        assert_eq!(too_long().code, "media_too_long");
        assert!(contiguous(&[0, 1, 2]));
        assert!(!contiguous(&[0, 2]));
        assert!(!contiguous(&[]));
        assert_eq!(slice_index("slice-024.wav"), Some(24));
        assert_eq!(slice_index("slice-000.wav"), Some(0));
        assert_eq!(slice_index("slice-24.wav"), None);
        assert_eq!(slice_index("source.webm"), None);
        assert!(reject_input(0).unwrap().code == "media_invalid");
        assert!(reject_input(1).is_none());
        assert!(reject_input(MAX_INPUT_BYTES).is_none());
        assert_eq!(
            reject_input(MAX_INPUT_BYTES + 1).unwrap().code,
            "media_too_large"
        );
        assert_eq!(advance(300_000, 1_000), Some(301_000));
        assert_eq!(advance(i64::MAX, 1), None);
    }

    #[test]
    fn silent_gate_drops_invented_text_without_changing_duration_math() {
        let invented = vec![Segment {
            start_ms: 0,
            end_ms: 1,
            text: "invented words".into(),
            uncertain: false,
        }];
        assert!(gated(true, invented).is_empty());
        let kept = gated(
            false,
            vec![Segment {
                start_ms: 3,
                end_ms: 4,
                text: "kept".into(),
                uncertain: false,
            }],
        );
        assert_eq!(kept[0].text, "kept");
        assert_eq!(advance(0, 1_000), Some(1_000));
    }

    #[test]
    fn errors_and_recognition_do_not_carry_paths_speakers_or_secrets() {
        for error in [
            unconfigured(),
            too_large(),
            too_long(),
            empty_audio(),
            decode_timed_out(),
            recognize_timed_out(),
            decode_failed(),
            recognize_failed(),
            output_bounded(),
            media_failed(),
            temp_failed(),
        ] {
            assert_static(&error);
        }
        let body = assemble(
            vec![Segment {
                start_ms: 0,
                end_ms: 1,
                text: "hello".into(),
                uncertain: false,
            }],
            1_000,
            "ggml-base.en.bin".into(),
            "ab".into(),
        );
        let value = serde_json::to_value(&body).unwrap();
        assert_eq!(value["engine"], "whisper.cpp");
        assert_eq!(value["duration_ms"], 1_000);
        assert!(value.get("speaker").is_none());
        assert!(value.get("manual_review_required").is_none());
        assert!(value["segments"][0].get("speaker").is_none());
        assert_eq!(
            model_label(Path::new("/secret/models/ggml-base.en.bin")),
            "ggml-base.en.bin"
        );
        assert_eq!(model_label(Path::new("/secret/models/")), "models");
    }

    #[test]
    fn model_hash_is_content_sha256_not_the_path() {
        let dir = std::env::temp_dir().join(format!("nexus-media-hash-{}", Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join("model.bin");
        std::fs::write(&file, b"abc").unwrap();
        assert_eq!(
            hash_file(&file).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(!hash_file(&file).unwrap().contains("model.bin"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cleanup_removes_only_the_owned_job_directory() {
        let own = std::env::temp_dir().join(format!("nexus-media-{}", Uuid::new_v4()));
        let sibling = std::env::temp_dir().join(format!("nexus-keep-{}", Uuid::new_v4()));
        std::fs::create_dir(&own).unwrap();
        std::fs::create_dir(&sibling).unwrap();
        std::fs::write(own.join("source.webm"), b"x").unwrap();
        assert!(is_owned_job_dir(&own));
        assert!(!is_owned_job_dir(&sibling));
        assert!(!is_owned_job_dir(
            &std::env::temp_dir().join("nexus-media-..")
        ));
        {
            let _cleanup = TempCleanup(own.clone());
            let _ignored = TempCleanup(sibling.clone());
        }
        assert!(!own.exists());
        assert!(sibling.exists());
        std::fs::remove_dir_all(&sibling).unwrap();
    }

    #[tokio::test]
    async fn twenty_fifth_part_proves_too_long_before_recognition() {
        let dir = std::env::temp_dir().join(format!("nexus-media-{}", Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        for i in 0..25 {
            std::fs::write(dir.join(format!("slice-{i:03}.wav")), b"not-audio").unwrap();
        }
        let error = collect_slices(&dir).await.unwrap_err();
        assert_eq!(error.code, "media_too_long");
        assert!(error.message.contains("not truncated"));
        assert_static(&error);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn silent_slice_preserves_measured_duration_and_gaps_fail() {
        let dir = std::env::temp_dir().join(format!("nexus-media-{}", Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("slice-000.wav"), pcm_wav(&vec![0i16; 16_000])).unwrap();
        let slices = collect_slices(&dir).await.unwrap();
        assert_eq!(slices.len(), 1);
        assert!(slices[0].silent);
        assert_eq!(slices[0].duration_ms, 1_000);
        std::fs::write(dir.join("slice-002.wav"), pcm_wav(&[9, 9])).unwrap();
        let error = collect_slices(&dir).await.unwrap_err();
        assert_eq!(error.code, "media_failed");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
