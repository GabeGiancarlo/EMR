//! Who is making a request.
//!
//! `Auth::Local` is the loopback development mode: the browser names a staff row in
//! `x-nexus-actor` and nothing proves it. `Auth::Oidc` is the hosted mode: every staff
//! route requires a JWT signed by the configured identity provider, and the verified
//! email in it must match exactly one active `staff` row. The development header is
//! never read in hosted mode.
use crate::{ApiError, AppState};
use actix_web::{
    body::{EitherBody, MessageBody},
    dev::{ServiceRequest, ServiceResponse},
    middleware::Next,
    web, HttpMessage, HttpResponse, ResponseError,
};
use jsonwebtoken::{
    decode, decode_header,
    jwk::{AlgorithmParameters, EllipticCurve, Jwk, JwkSet, PublicKeyUse},
    Algorithm, DecodingKey, Validation,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

/// Asymmetric signatures only. A shared-secret (HS*) token is never accepted, so a
/// public key can never be replayed as an HMAC secret.
const ALGORITHMS: [Algorithm; 8] = [
    Algorithm::RS256,
    Algorithm::RS384,
    Algorithm::RS512,
    Algorithm::PS256,
    Algorithm::PS384,
    Algorithm::PS512,
    Algorithm::ES256,
    Algorithm::ES384,
];
const CLOCK_LEEWAY_SECONDS: u64 = 60;
const KEY_MAX_AGE: Duration = Duration::from_secs(60 * 60);
const KEY_REFRESH_COOLDOWN: Duration = Duration::from_secs(30);
const PROVIDER_TIMEOUT: Duration = Duration::from_secs(10);
const PROVIDER_DOCUMENT_LIMIT: usize = 512 * 1024;
const MAX_TOKEN_BYTES: usize = 16 * 1024;

/// How patiently startup waits for an identity provider that cannot be reached.
#[derive(Clone, Copy, Debug)]
pub struct RetryPolicy {
    /// Stop retrying once this much time has passed since the first attempt.
    pub patience: Duration,
    /// Pause after the first failure; doubles after each further one.
    pub first_pause: Duration,
    pub longest_pause: Duration,
}
impl RetryPolicy {
    /// A provider restart or a brief network fault at boot: about two minutes.
    pub const STARTUP: RetryPolicy = RetryPolicy {
        patience: Duration::from_secs(120),
        first_pause: Duration::from_secs(1),
        longest_pause: Duration::from_secs(30),
    };

    /// The pauses between attempts: doubling up to the ceiling, for as long as the
    /// next pause still fits inside `patience`.
    pub fn pauses(&self) -> Vec<Duration> {
        let mut pauses = Vec::new();
        let (mut next, mut total) = (self.first_pause, Duration::ZERO);
        while !next.is_zero() && total + next <= self.patience {
            pauses.push(next);
            total += next;
            next = (next * 2).min(self.longest_pause);
        }
        pauses
    }
}

/// Why the provider could not be read at startup. `transient` is true only for
/// faults that waiting can cure: the provider could not be reached, could not be
/// read, or answered with a server-side or throttling status.
struct ConnectError {
    message: String,
    transient: bool,
}
impl ConnectError {
    fn permanent(message: String) -> Self {
        Self {
            message,
            transient: false,
        }
    }
}

struct FetchError {
    why: String,
    transient: bool,
}

#[derive(Clone)]
pub enum Auth {
    /// Loopback development. Operator selection is not authentication.
    Local,
    /// Hosted. Bearer tokens from one identity provider.
    Oidc(Arc<Oidc>),
}
impl Auth {
    pub fn is_local(&self) -> bool {
        matches!(self, Auth::Local)
    }
    /// The value reported to clients in `/healthz` and `/api/bootstrap`.
    pub fn label(&self) -> &'static str {
        match self {
            Auth::Local => "deferred-local-only",
            Auth::Oidc(_) => "oidc",
        }
    }
}

/// The staff member the OIDC gate verified for this request, with the row it read,
/// so the handler does not read the same row again.
#[derive(Clone)]
pub(crate) struct Identity {
    pub id: Uuid,
    pub staff: Value,
}

#[derive(Clone, Debug)]
pub struct OidcSettings {
    /// Must equal the token's `iss` and the provider's discovery document exactly.
    pub issuer: String,
    /// Must appear in the token's `aud`.
    pub audience: String,
    /// Public client identifier the browser uses for the sign-in redirect.
    pub client_id: String,
    /// Overrides the `jwks_uri` from the provider's discovery document.
    pub jwks_url: Option<String>,
    pub scopes: String,
}

#[derive(Clone, Debug)]
struct Endpoints {
    authorization: String,
    token: String,
    end_session: Option<String>,
}

struct KeyCache {
    keys: Vec<Jwk>,
    fetched: Instant,
    attempted: Instant,
}

/// The provider's signing keys. Shared behind an `Arc` so a refresh can run in the
/// background while requests keep verifying against the keys already held.
struct KeyStore {
    url: String,
    http: reqwest::Client,
    cache: RwLock<KeyCache>,
    /// Held for the length of one download, so there is never more than one.
    downloading: Mutex<()>,
    cooldown_ms: AtomicU64,
    max_age_ms: AtomicU64,
}

pub struct Oidc {
    settings: OidcSettings,
    endpoints: Endpoints,
    keys: Arc<KeyStore>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Rejection {
    /// No usable proof of identity: missing, malformed, expired or wrongly signed.
    Unauthenticated,
    /// The token is genuine but does not carry a verified email.
    Forbidden,
}

#[derive(Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
    end_session_endpoint: Option<String>,
}

#[derive(Deserialize)]
struct Claims {
    email: Option<String>,
    email_verified: Option<Value>,
}

/// Provider URLs must be HTTPS. Plain HTTP is accepted only for a loopback host,
/// which is how the tests and a local mock provider are reached.
pub(crate) fn provider_url(raw: &str) -> Result<reqwest::Url, &'static str> {
    let url = reqwest::Url::parse(raw).map_err(|_| "is not a valid URL")?;
    let loopback = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    match url.scheme() {
        "https" => Ok(url),
        "http" if loopback => Ok(url),
        _ => Err("must use https"),
    }
}

async fn fetch_json<T: serde::de::DeserializeOwned>(
    http: &reqwest::Client,
    url: &str,
) -> Result<T, FetchError> {
    let failed = |why: &str, transient| FetchError {
        why: why.to_string(),
        transient,
    };
    let mut response = http
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|_| failed("could not be reached", true))?;
    let status = response.status();
    if !status.is_success() {
        let transient = status.is_server_error() || matches!(status.as_u16(), 408 | 429);
        return Err(failed(
            &format!("answered HTTP {}", status.as_u16()),
            transient,
        ));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| failed("could not be read", true))?
    {
        if body.len() + chunk.len() > PROVIDER_DOCUMENT_LIMIT {
            return Err(failed("returned an oversized document", false));
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| failed("did not return the expected JSON", false))
}

/// Keep only keys a signature can be checked against: RSA or P-256/P-384 public
/// keys that are not marked for encryption.
fn signing_keys(set: JwkSet) -> Vec<Jwk> {
    set.keys
        .into_iter()
        .filter(|key| {
            !matches!(
                key.common.public_key_use,
                Some(PublicKeyUse::Encryption | PublicKeyUse::Other(_))
            )
        })
        .filter(|key| match &key.algorithm {
            AlgorithmParameters::RSA(_) => true,
            AlgorithmParameters::EllipticCurve(params) => {
                matches!(params.curve, EllipticCurve::P256 | EllipticCurve::P384)
            }
            _ => false,
        })
        .collect()
}

fn key_fits(key: &Jwk, algorithm: Algorithm) -> bool {
    if let Some(pinned) = key.common.key_algorithm {
        if Algorithm::try_from(pinned).ok() != Some(algorithm) {
            return false;
        }
    }
    match (&key.algorithm, algorithm) {
        (
            AlgorithmParameters::RSA(_),
            Algorithm::RS256
            | Algorithm::RS384
            | Algorithm::RS512
            | Algorithm::PS256
            | Algorithm::PS384
            | Algorithm::PS512,
        ) => true,
        (AlgorithmParameters::EllipticCurve(params), Algorithm::ES256) => {
            params.curve == EllipticCurve::P256
        }
        (AlgorithmParameters::EllipticCurve(params), Algorithm::ES384) => {
            params.curve == EllipticCurve::P384
        }
        _ => false,
    }
}

impl KeyStore {
    async fn held(&self, kid: Option<&str>, algorithm: Algorithm) -> Option<Jwk> {
        let cache = self.cache.read().await;
        let mut fitting = cache.keys.iter().filter(|key| key_fits(key, algorithm));
        match kid {
            Some(kid) => fitting
                .find(|key| key.common.key_id.as_deref() == Some(kid))
                .cloned(),
            // Without a key id the choice must be unambiguous.
            None => match (fitting.next(), fitting.next()) {
                (Some(only), None) => Some(only.clone()),
                _ => None,
            },
        }
    }

    /// Whether a download may start now. Every attempt, successful or not, starts the
    /// cooldown, so an unreachable provider is asked at most once per cooldown.
    /// `only_if_old` adds the age test used for routine refreshes.
    async fn refresh_due(&self, only_if_old: bool) -> bool {
        let cache = self.cache.read().await;
        let cooldown = Duration::from_millis(self.cooldown_ms.load(Ordering::Relaxed));
        let max_age = Duration::from_millis(self.max_age_ms.load(Ordering::Relaxed));
        cache.attempted.elapsed() >= cooldown && (!only_if_old || cache.fetched.elapsed() > max_age)
    }

    /// Download the key set if one is due. The caller holds `downloading`. A failed
    /// download keeps the keys already held, so a provider outage does not sign
    /// everyone out.
    async fn download(&self, only_if_old: bool) {
        if !self.refresh_due(only_if_old).await {
            return;
        }
        // Start the cooldown before the request, so nothing else queues another
        // download while this one is waiting on a slow provider.
        self.cache.write().await.attempted = Instant::now();
        let fetched = fetch_json::<JwkSet>(&self.http, &self.url).await;
        let mut cache = self.cache.write().await;
        match fetched.map(signing_keys) {
            Ok(keys) if !keys.is_empty() => {
                cache.keys = keys;
                cache.fetched = Instant::now();
            }
            Ok(_) => log::warn!(
                "Identity provider returned no usable signing keys; keeping the previous set"
            ),
            Err(error) => {
                log::warn!(
                    "Identity provider signing keys {}; keeping the previous set",
                    error.why
                )
            }
        }
    }
}

impl Oidc {
    /// Read the provider's discovery document and signing keys. Startup fails here,
    /// with a message naming the variable to fix, instead of rejecting every sign-in later.
    pub async fn connect(settings: OidcSettings) -> Result<Self, String> {
        Self::connect_once(settings)
            .await
            .map_err(|error| error.message)
    }

    /// [`Oidc::connect`], retried while the provider cannot be reached, so a restart
    /// does not crash-loop because the provider blinked. A fault that waiting cannot
    /// cure (a wrong issuer, a document that is not the expected JSON, no usable key)
    /// fails at once, and when patience runs out the last plain message is returned.
    pub async fn connect_with_retry(
        settings: OidcSettings,
        policy: RetryPolicy,
    ) -> Result<Self, String> {
        let mut pauses = policy.pauses().into_iter();
        loop {
            let error = match Self::connect_once(settings.clone()).await {
                Ok(connected) => return Ok(connected),
                Err(error) => error,
            };
            match pauses.next() {
                Some(pause) if error.transient => {
                    log::warn!(
                        "{} Trying again in {} seconds.",
                        error.message,
                        pause.as_secs_f32()
                    );
                    tokio::time::sleep(pause).await;
                }
                _ => return Err(error.message),
            }
        }
    }

    async fn connect_once(settings: OidcSettings) -> Result<Self, ConnectError> {
        provider_url(&settings.issuer)
            .map_err(|why| ConnectError::permanent(format!("NEXUS_OIDC_ISSUER {why}.")))?;
        let http = reqwest::Client::builder()
            .timeout(PROVIDER_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| {
                ConnectError::permanent(
                    "The HTTPS client for the identity provider could not start.".to_string(),
                )
            })?;
        let discovery_url = format!(
            "{}/.well-known/openid-configuration",
            settings.issuer.trim_end_matches('/')
        );
        let discovery: Discovery = fetch_json(&http, &discovery_url).await.map_err(|error| ConnectError {
            message: format!("The identity provider's discovery document ({discovery_url}) {}. Check NEXUS_OIDC_ISSUER.", error.why),
            transient: error.transient,
        })?;
        if discovery.issuer != settings.issuer {
            return Err(ConnectError::permanent(format!(
                "The identity provider reports its issuer as \"{}\". Set NEXUS_OIDC_ISSUER to exactly that value, including any trailing slash.",
                discovery.issuer
            )));
        }
        let jwks_url = settings
            .jwks_url
            .clone()
            .unwrap_or_else(|| discovery.jwks_uri.clone());
        for (name, value) in [
            ("signing-key URL (NEXUS_OIDC_JWKS_URL)", jwks_url.as_str()),
            (
                "authorization endpoint",
                discovery.authorization_endpoint.as_str(),
            ),
            ("token endpoint", discovery.token_endpoint.as_str()),
        ] {
            provider_url(value).map_err(|why| {
                ConnectError::permanent(format!("The identity provider's {name} {why}."))
            })?;
        }
        let end_session = discovery
            .end_session_endpoint
            .filter(|url| provider_url(url).is_ok());
        let keys =
            signing_keys(
                fetch_json(&http, &jwks_url)
                    .await
                    .map_err(|error| ConnectError {
                        message: format!(
                            "The identity provider's signing keys ({jwks_url}) {}.",
                            error.why
                        ),
                        transient: error.transient,
                    })?,
            );
        if keys.is_empty() {
            return Err(ConnectError::permanent(format!(
                "The identity provider's signing keys ({jwks_url}) contain no RSA or elliptic-curve signing key."
            )));
        }
        let now = Instant::now();
        Ok(Self {
            settings,
            endpoints: Endpoints {
                authorization: discovery.authorization_endpoint,
                token: discovery.token_endpoint,
                end_session,
            },
            keys: Arc::new(KeyStore {
                url: jwks_url,
                http,
                cache: RwLock::new(KeyCache {
                    keys,
                    fetched: now,
                    attempted: now,
                }),
                downloading: Mutex::new(()),
                cooldown_ms: AtomicU64::new(KEY_REFRESH_COOLDOWN.as_millis() as u64),
                max_age_ms: AtomicU64::new(KEY_MAX_AGE.as_millis() as u64),
            }),
        })
    }

    /// Minimum spacing between key downloads. An unknown `kid` triggers a download,
    /// so the spacing stops forged tokens from making the API hammer the provider.
    pub fn with_refresh_cooldown(self, cooldown: Duration) -> Self {
        self.keys
            .cooldown_ms
            .store(cooldown.as_millis() as u64, Ordering::Relaxed);
        self
    }

    /// How old the held keys may get before a background refresh is started.
    pub fn with_key_max_age(self, max_age: Duration) -> Self {
        self.keys
            .max_age_ms
            .store(max_age.as_millis() as u64, Ordering::Relaxed);
        self
    }

    /// What the browser needs to start sign-in. Nothing here is secret.
    pub fn public_config(&self) -> Value {
        json!({
            "mode": "oidc",
            "issuer": self.settings.issuer,
            "client_id": self.settings.client_id,
            "scopes": self.settings.scopes,
            "authorization_endpoint": self.endpoints.authorization,
            "token_endpoint": self.endpoints.token,
            "end_session_endpoint": self.endpoints.end_session,
        })
    }

    /// The key a token names. A key already held is returned at once and never waits
    /// on the provider: if the held set is old, a refresh starts in the background.
    /// Only a key id that is not held waits for a download, because the provider may
    /// have rotated its keys and nothing else can verify that token.
    async fn key(&self, kid: Option<&str>, algorithm: Algorithm) -> Option<Jwk> {
        if let Some(key) = self.keys.held(kid, algorithm).await {
            if self.keys.refresh_due(true).await {
                let keys = self.keys.clone();
                tokio::spawn(async move {
                    // Another request may already be downloading; one is enough.
                    if let Ok(_only_one) = keys.downloading.try_lock() {
                        keys.download(true).await;
                    }
                });
            }
            return Some(key);
        }
        {
            let _only_one = self.keys.downloading.lock().await;
            self.keys.download(false).await;
        }
        self.keys.held(kid, algorithm).await
    }

    /// Check signature, issuer, audience, expiry and not-before, then return the
    /// verified email in lower case.
    pub async fn verify(&self, token: &str) -> Result<String, Rejection> {
        self.verify_for(token, "-").await
    }

    /// [`Oidc::verify`], naming the request in the operator's log line when a
    /// correctly signed token is refused.
    pub(crate) async fn verify_for(
        &self,
        token: &str,
        request_id: &str,
    ) -> Result<String, Rejection> {
        if token.len() > MAX_TOKEN_BYTES {
            return Err(Rejection::Unauthenticated);
        }
        let header = decode_header(token).map_err(|_| Rejection::Unauthenticated)?;
        if !ALGORITHMS.contains(&header.alg) {
            return Err(Rejection::Unauthenticated);
        }
        let jwk = self
            .key(header.kid.as_deref(), header.alg)
            .await
            .ok_or(Rejection::Unauthenticated)?;
        let key = DecodingKey::from_jwk(&jwk).map_err(|_| Rejection::Unauthenticated)?;
        let mut validation = Validation::new(header.alg);
        validation.set_issuer(&[&self.settings.issuer]);
        validation.set_audience(&[&self.settings.audience]);
        validation.set_required_spec_claims(&["exp", "iss", "aud"]);
        validation.validate_nbf = true;
        validation.leeway = CLOCK_LEEWAY_SECONDS;
        let claims = decode::<Claims>(token, &key, &validation)
            .map_err(|_| Rejection::Unauthenticated)?
            .claims;
        // Some providers send the flag as the string "true".
        let verified = match &claims.email_verified {
            Some(Value::Bool(flag)) => *flag,
            Some(Value::String(text)) => text == "true",
            _ => false,
        };
        let email = claims.email.unwrap_or_default().trim().to_lowercase();
        if verified && !email.is_empty() {
            return Ok(email);
        }
        // The person sees one generic refusal. The operator needs the cause, which is
        // usually provider configuration. Neither the token nor the email is logged.
        let cause = if email.is_empty() {
            "it has no email claim; the provider must be asked for the email scope"
        } else if claims.email_verified.is_none() {
            "it has no email_verified claim; this provider does not state that the email is verified, which Nexus requires"
        } else {
            "its email_verified claim is not true; the account's email is unverified at the provider"
        };
        log::warn!(
            "Sign-in refused for a correctly signed identity token: {cause} (request_id={request_id})"
        );
        Err(Rejection::Forbidden)
    }
}

/// Routes whose caller is not a staff member. Each checks its own single-purpose
/// token: `/api/auth/config` is public, visit routes take a patient invitation
/// token, and recording-track routes take a per-track upload token.
pub(crate) fn carries_own_credential(path: &str) -> bool {
    path == "/api/auth/config"
        || path == "/api/telehealth/visit"
        || path.starts_with("/api/telehealth/visit/")
        || path.starts_with("/api/recording-tracks/")
}

/// The credential from an `Authorization: Bearer <token>` header. The scheme name
/// is case-insensitive (RFC 9110).
fn bearer_token(header: &str) -> Option<&str> {
    let (scheme, token) = header.trim_start().split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty() && !token.contains(' '))
        .then_some(token)
}

fn bearer(req: &ServiceRequest) -> Option<&str> {
    bearer_token(
        req.headers()
            .get(actix_web::http::header::AUTHORIZATION)?
            .to_str()
            .ok()?,
    )
}

async fn identify(
    req: &ServiceRequest,
    state: &AppState,
    oidc: &Oidc,
) -> Result<Identity, ApiError> {
    let request_id = crate::observe::request_id(req);
    let token = bearer(req).ok_or_else(ApiError::unauthenticated)?;
    let email = oidc
        .verify_for(token, &request_id)
        .await
        .map_err(|rejection| match rejection {
            Rejection::Unauthenticated => ApiError::unauthenticated(),
            Rejection::Forbidden => ApiError::no_access(),
        })?;
    // One answer for unknown, inactive and ambiguous accounts: none of them reveals
    // which staff exist.
    let mut matches: Vec<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(s) FROM staff s WHERE lower(email)=$1 AND active LIMIT 2",
    )
    .bind(&email)
    .fetch_all(&state.pool)
    .await?;
    let id = match matches.as_slice() {
        [staff] => staff["id"].as_str().and_then(|id| Uuid::parse_str(id).ok()),
        _ => None,
    };
    match (id, matches.pop()) {
        (Some(id), Some(staff)) => Ok(Identity { id, staff }),
        _ => {
            log::info!(
                "Sign-in refused: the verified email matches no single active staff member (request_id={request_id})"
            );
            Err(ApiError::no_access())
        }
    }
}

/// Wraps every `/api` route. Local mode passes through unchanged. Hosted mode lets
/// a request reach a staff route only after its bearer token resolved to a staff row.
pub(crate) async fn gate<B: MessageBody + 'static>(
    req: ServiceRequest,
    next: Next<B>,
) -> Result<ServiceResponse<EitherBody<B>>, actix_web::Error> {
    let Some(state) = req.app_data::<web::Data<AppState>>().cloned() else {
        let response = ApiError::unauthenticated().error_response();
        return Ok(req.into_response(response).map_into_right_body());
    };
    let Auth::Oidc(oidc) = &state.auth else {
        return Ok(next.call(req).await?.map_into_left_body());
    };
    if carries_own_credential(req.path()) {
        return Ok(next.call(req).await?.map_into_left_body());
    }
    match identify(&req, &state, oidc).await {
        Ok(identity) => {
            req.extensions_mut().insert(identity);
            Ok(next.call(req).await?.map_into_left_body())
        }
        Err(error) => Ok(req
            .into_response(error.error_response())
            .map_into_right_body()),
    }
}

/// `GET /api/auth/config`: how the browser should sign in.
pub(crate) async fn config(state: web::Data<AppState>) -> HttpResponse {
    let body = match &state.auth {
        Auth::Local => json!({"mode": "local"}),
        Auth::Oidc(oidc) => oidc.public_config(),
    };
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_urls_must_be_https_unless_loopback() {
        assert!(provider_url("https://tenant.example.com/").is_ok());
        assert!(provider_url("http://127.0.0.1:9000").is_ok());
        assert!(provider_url("http://localhost:9000/realm").is_ok());
        assert!(provider_url("http://tenant.example.com/").is_err());
        assert!(provider_url("ftp://tenant.example.com/").is_err());
        assert!(provider_url("tenant.example.com").is_err());
    }

    #[test]
    fn startup_retries_double_to_a_ceiling_and_stay_inside_their_patience() {
        let seconds = |policy: RetryPolicy| -> Vec<u64> {
            policy.pauses().iter().map(Duration::as_secs).collect()
        };
        assert_eq!(
            seconds(RetryPolicy::STARTUP),
            [1, 2, 4, 8, 16, 30, 30],
            "about a minute and a half of pauses, then the last attempt"
        );
        assert!(RetryPolicy::STARTUP.pauses().iter().sum::<Duration>() <= Duration::from_secs(120));
        let none = RetryPolicy {
            patience: Duration::ZERO,
            ..RetryPolicy::STARTUP
        };
        assert!(none.pauses().is_empty());
        let stuck = RetryPolicy {
            first_pause: Duration::ZERO,
            ..RetryPolicy::STARTUP
        };
        assert!(stuck.pauses().is_empty());
    }

    #[test]
    fn only_capability_routes_skip_the_staff_gate() {
        for open in [
            "/api/auth/config",
            "/api/telehealth/visit",
            "/api/telehealth/visit/poll",
            "/api/telehealth/visit/recordings/start",
            "/api/recording-tracks/7/chunks/0",
        ] {
            assert!(carries_own_credential(open), "{open}");
        }
        for guarded in [
            "/api/bootstrap",
            "/api/setup",
            "/api/staff",
            "/api/telehealth/rooms",
            "/api/telehealth/visitors",
            "/api/auth/config/extra",
            "/api/recording-policies",
        ] {
            assert!(!carries_own_credential(guarded), "{guarded}");
        }
    }

    #[test]
    fn the_bearer_scheme_is_case_insensitive_and_needs_one_token() {
        for header in ["Bearer abc", "bearer abc", "BEARER abc", "BeArEr   abc  "] {
            assert_eq!(bearer_token(header), Some("abc"), "{header}");
        }
        for header in [
            "",
            "Bearer",
            "Bearer ",
            "Basic abc",
            "abc",
            "Bearerabc",
            "Bearer a b",
        ] {
            assert_eq!(bearer_token(header), None, "{header}");
        }
    }

    #[test]
    fn shared_secret_algorithms_are_never_accepted() {
        for forbidden in [Algorithm::HS256, Algorithm::HS384, Algorithm::HS512] {
            assert!(!ALGORITHMS.contains(&forbidden));
        }
    }
}
