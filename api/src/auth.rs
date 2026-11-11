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
    sync::Arc,
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

/// The staff row the OIDC gate verified for this request.
#[derive(Clone, Copy)]
pub(crate) struct Identity(pub Uuid);

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

pub struct Oidc {
    settings: OidcSettings,
    endpoints: Endpoints,
    jwks_url: String,
    http: reqwest::Client,
    cache: RwLock<KeyCache>,
    refreshing: Mutex<()>,
    cooldown: Duration,
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
) -> Result<T, String> {
    let mut response = http
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|_| "could not be reached".to_string())?;
    if !response.status().is_success() {
        return Err(format!("answered HTTP {}", response.status().as_u16()));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "could not be read".to_string())?
    {
        if body.len() + chunk.len() > PROVIDER_DOCUMENT_LIMIT {
            return Err("returned an oversized document".into());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| "did not return the expected JSON".to_string())
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

impl Oidc {
    /// Read the provider's discovery document and signing keys. Startup fails here,
    /// with a message naming the variable to fix, instead of rejecting every sign-in later.
    pub async fn connect(settings: OidcSettings) -> Result<Self, String> {
        provider_url(&settings.issuer).map_err(|why| format!("NEXUS_OIDC_ISSUER {why}."))?;
        let http = reqwest::Client::builder()
            .timeout(PROVIDER_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| {
                "The HTTPS client for the identity provider could not start.".to_string()
            })?;
        let discovery_url = format!(
            "{}/.well-known/openid-configuration",
            settings.issuer.trim_end_matches('/')
        );
        let discovery: Discovery = fetch_json(&http, &discovery_url).await.map_err(|why| {
            format!("The identity provider's discovery document ({discovery_url}) {why}. Check NEXUS_OIDC_ISSUER.")
        })?;
        if discovery.issuer != settings.issuer {
            return Err(format!(
                "The identity provider reports its issuer as \"{}\". Set NEXUS_OIDC_ISSUER to exactly that value, including any trailing slash.",
                discovery.issuer
            ));
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
            provider_url(value).map_err(|why| format!("The identity provider's {name} {why}."))?;
        }
        let end_session = discovery
            .end_session_endpoint
            .filter(|url| provider_url(url).is_ok());
        let keys =
            signing_keys(fetch_json(&http, &jwks_url).await.map_err(|why| {
                format!("The identity provider's signing keys ({jwks_url}) {why}.")
            })?);
        if keys.is_empty() {
            return Err(format!(
                "The identity provider's signing keys ({jwks_url}) contain no RSA or elliptic-curve signing key."
            ));
        }
        let now = Instant::now();
        Ok(Self {
            settings,
            endpoints: Endpoints {
                authorization: discovery.authorization_endpoint,
                token: discovery.token_endpoint,
                end_session,
            },
            jwks_url,
            http,
            cache: RwLock::new(KeyCache {
                keys,
                fetched: now,
                attempted: now,
            }),
            refreshing: Mutex::new(()),
            cooldown: KEY_REFRESH_COOLDOWN,
        })
    }

    /// Minimum spacing between key downloads. An unknown `kid` triggers a download,
    /// so the spacing stops forged tokens from making the API hammer the provider.
    pub fn with_refresh_cooldown(mut self, cooldown: Duration) -> Self {
        self.cooldown = cooldown;
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

    async fn cached_key(&self, kid: Option<&str>, algorithm: Algorithm) -> Option<Jwk> {
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

    /// Download the key set again, at most once per cooldown. A failed download keeps
    /// the keys already held, so a provider outage does not sign everyone out.
    async fn refresh_keys(&self) {
        let _one_at_a_time = self.refreshing.lock().await;
        if self.cache.read().await.attempted.elapsed() < self.cooldown {
            return;
        }
        let fetched = fetch_json::<JwkSet>(&self.http, &self.jwks_url).await;
        let mut cache = self.cache.write().await;
        cache.attempted = Instant::now();
        match fetched.map(signing_keys) {
            Ok(keys) if !keys.is_empty() => {
                cache.keys = keys;
                cache.fetched = cache.attempted;
            }
            Ok(_) => log::warn!(
                "Identity provider returned no usable signing keys; keeping the previous set"
            ),
            Err(why) => {
                log::warn!("Identity provider signing keys {why}; keeping the previous set")
            }
        }
    }

    async fn key(&self, kid: Option<&str>, algorithm: Algorithm) -> Option<Jwk> {
        if self.cache.read().await.fetched.elapsed() > KEY_MAX_AGE {
            self.refresh_keys().await;
        }
        if let Some(key) = self.cached_key(kid, algorithm).await {
            return Some(key);
        }
        // The provider may have rotated its keys since the last download.
        self.refresh_keys().await;
        self.cached_key(kid, algorithm).await
    }

    /// Check signature, issuer, audience, expiry and not-before, then return the
    /// verified email in lower case.
    pub async fn verify(&self, token: &str) -> Result<String, Rejection> {
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
        let verified = match claims.email_verified {
            Some(Value::Bool(flag)) => flag,
            Some(Value::String(text)) => text == "true",
            _ => false,
        };
        let email = claims.email.unwrap_or_default().trim().to_lowercase();
        if verified && !email.is_empty() {
            Ok(email)
        } else {
            Err(Rejection::Forbidden)
        }
    }
}

/// Routes whose caller is not a staff member. Each checks its own single-purpose
/// token: `/api/auth/config` is public, visit routes take a patient invitation
/// token, and recording-track routes take a per-track upload token.
fn carries_own_credential(path: &str) -> bool {
    path == "/api/auth/config"
        || path == "/api/telehealth/visit"
        || path.starts_with("/api/telehealth/visit/")
        || path.starts_with("/api/recording-tracks/")
}

fn bearer(req: &ServiceRequest) -> Option<&str> {
    req.headers()
        .get(actix_web::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

async fn identify(
    req: &ServiceRequest,
    state: &AppState,
    oidc: &Oidc,
) -> Result<Identity, ApiError> {
    let token = bearer(req).ok_or_else(ApiError::unauthenticated)?;
    let email = oidc
        .verify(token)
        .await
        .map_err(|rejection| match rejection {
            Rejection::Unauthenticated => ApiError::unauthenticated(),
            Rejection::Forbidden => ApiError::no_access(),
        })?;
    // One answer for unknown, inactive and ambiguous accounts: none of them reveals
    // which staff exist.
    let matches: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM staff WHERE lower(email)=$1 AND active LIMIT 2")
            .bind(&email)
            .fetch_all(&state.pool)
            .await?;
    match matches.as_slice() {
        [id] => Ok(Identity(*id)),
        _ => Err(ApiError::no_access()),
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
    fn shared_secret_algorithms_are_never_accepted() {
        for forbidden in [Algorithm::HS256, Algorithm::HS384, Algorithm::HS512] {
            assert!(!ALGORITHMS.contains(&forbidden));
        }
    }
}
