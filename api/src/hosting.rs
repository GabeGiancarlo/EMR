//! Startup configuration and the HTTP hardening shared by both modes.
//!
//! `NEXUS_AUTH_MODE=local` (the default) is today's loopback development server and
//! keeps its guard: it needs `NEXUS_LOCAL_DEV=1` and a loopback `HOST`.
//! `NEXUS_AUTH_MODE=oidc` is the hosted mode. It may bind a public address, and it
//! refuses to start until every variable it needs is present and well formed.
use crate::{
    auth::{provider_url, OidcSettings},
    db::PoolSettings,
    limits,
};
use actix_cors::Cors;
use actix_web::middleware::DefaultHeaders;
use std::time::Duration;

pub const LOCAL_DATABASE_URL: &str = "postgres://nexus:nexus_local@127.0.0.1:55432/nexus";
const LOCAL_ORIGINS: [&str; 2] = ["http://localhost:5173", "http://127.0.0.1:5173"];
const DEFAULT_SCOPES: &str = "openid email profile";

/// A client must send its complete request head within this time, or the connection
/// is closed. Bodies are bounded separately (`observe::REQUEST_DEADLINE`).
pub const CLIENT_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
/// How long a connection being closed may take to finish closing.
pub const CLIENT_DISCONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// An idle kept-alive connection is closed after this long. It is longer than the
/// idle timeout of the proxies in front of a hosted API, so the proxy, not the API,
/// is the side that retires a connection and no request is sent into a closing one.
pub const KEEP_ALIVE: Duration = Duration::from_secs(75);
/// On a stop signal, requests in flight get this long to finish. Hosts allow about
/// thirty seconds before they kill the process.
pub const SHUTDOWN_TIMEOUT_SECONDS: u64 = 25;

#[derive(Clone, Debug)]
pub enum Mode {
    Local,
    Oidc(OidcSettings),
}

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub mode: Mode,
    pub host: String,
    pub port: u16,
    pub database_url: String,
    /// Browser origins allowed to call the API across origins.
    pub allowed_origins: Vec<String>,
    /// Request connection pool size and session timeouts.
    pub pool: PoolSettings,
    /// Per-client request limits and the trusted-proxy count.
    pub limits: limits::Settings,
}

/// Read an optional whole number within `range`. The error names the variable and
/// the accepted range, never the value.
fn number(
    get: &impl Fn(&str) -> Option<String>,
    name: &str,
    default: u32,
    range: std::ops::RangeInclusive<u32>,
) -> Result<u32, String> {
    match get(name) {
        None => Ok(default),
        Some(raw) => raw
            .parse::<u32>()
            .ok()
            .filter(|value| range.contains(value))
            .ok_or_else(|| {
                format!(
                    "{name} must be a whole number from {} to {}",
                    range.start(),
                    range.end()
                )
            }),
    }
}

/// Pool and rate-limit settings, shared by both modes. Every problem is reported,
/// not just the first.
fn tuning(
    get: &impl Fn(&str) -> Option<String>,
    hosted: bool,
) -> Result<(PoolSettings, limits::Settings), Vec<String>> {
    let mut invalid = Vec::new();
    let mut read = |name: &str, default: u32, range: std::ops::RangeInclusive<u32>| {
        number(get, name, default, range).unwrap_or_else(|why| {
            invalid.push(why);
            default
        })
    };
    let defaults = limits::Settings::default();
    // Zero leaves the database server's own timeout in place.
    let seconds = |value: u32| (value > 0).then(|| Duration::from_secs(u64::from(value)));
    let pool = PoolSettings {
        max_connections: read("NEXUS_DB_MAX_CONNECTIONS", 10, 1..=200),
        statement_timeout: seconds(read("NEXUS_DB_STATEMENT_TIMEOUT_SECONDS", 30, 0..=3600)),
        lock_timeout: seconds(read("NEXUS_DB_LOCK_TIMEOUT_SECONDS", 15, 0..=3600)),
    };
    let mut limits = limits::Settings {
        enabled: true,
        trusted_proxies: read("NEXUS_TRUSTED_PROXIES", 0, 0..=8) as usize,
        open: limits::Rate {
            per_minute: read(
                "NEXUS_RATE_LIMIT_OPEN_PER_MINUTE",
                defaults.open.per_minute,
                1..=1_000_000,
            ),
            burst: read(
                "NEXUS_RATE_LIMIT_OPEN_BURST",
                defaults.open.burst,
                1..=1_000_000,
            ),
        },
        all: limits::Rate {
            per_minute: read(
                "NEXUS_RATE_LIMIT_PER_MINUTE",
                defaults.all.per_minute,
                1..=1_000_000,
            ),
            burst: read("NEXUS_RATE_LIMIT_BURST", defaults.all.burst, 1..=1_000_000),
        },
    };
    match get("NEXUS_RATE_LIMIT").as_deref() {
        None | Some("on") => {}
        // A public listener is never left without request limits; raise them instead.
        Some("off") if hosted => invalid.push(
            "NEXUS_RATE_LIMIT=off is accepted only in local mode; raise the limits instead".into(),
        ),
        Some("off") => limits.enabled = false,
        Some(_) => invalid.push("NEXUS_RATE_LIMIT must be on or off".into()),
    }
    if invalid.is_empty() {
        Ok((pool, limits))
    } else {
        Err(invalid)
    }
}

fn loopback(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "::1" | "localhost")
}

/// An origin is scheme, host and optional port: no path, query or wildcard.
/// Plain HTTP is accepted only for a loopback host.
fn origin(raw: &str) -> Result<String, &'static str> {
    let url = reqwest::Url::parse(raw).map_err(|_| "is not a valid origin")?;
    let secure = url.scheme() == "https";
    let local = url.scheme() == "http"
        && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    if !secure && !local {
        return Err("must use https");
    }
    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() || raw.ends_with('/')
    {
        return Err(
            "must be an origin such as https://app.example.com, with no path or trailing slash",
        );
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("must not contain credentials");
    }
    Ok(raw.to_string())
}

impl ServerConfig {
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Build the configuration from a variable lookup. Blank values count as unset.
    /// Error text names variables, never their values.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let get = |name: &str| {
            get(name)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };
        let mode = get("NEXUS_AUTH_MODE").unwrap_or_else(|| "local".into());
        let host = get("HOST").unwrap_or_else(|| "127.0.0.1".into());
        let port = match get("PORT") {
            None => Ok(8090),
            Some(raw) => raw
                .parse::<u16>()
                .ok()
                .filter(|port| *port != 0)
                .ok_or("PORT must be a number from 1 to 65535"),
        };
        let origins = get("NEXUS_ALLOWED_ORIGINS").map(|list| {
            list.split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(|item| {
                    origin(item).map_err(|why| format!("NEXUS_ALLOWED_ORIGINS entry {why}"))
                })
                .collect::<Result<Vec<_>, _>>()
        });
        let tuning = tuning(&get, mode == "oidc");
        match mode.as_str() {
            "local" => {
                if get("NEXUS_LOCAL_DEV").as_deref() != Some("1") {
                    return Err("Authentication is deferred. Set NEXUS_LOCAL_DEV=1 for loopback development only, or NEXUS_AUTH_MODE=oidc with an identity provider to host.".into());
                }
                if !loopback(&host) {
                    return Err("Local actor mode requires a loopback HOST. Only NEXUS_AUTH_MODE=oidc may listen on a public address.".into());
                }
                let (pool, limits) = tuning.map_err(|invalid| {
                    format!("Local mode cannot start. Invalid: {}.", invalid.join("; "))
                })?;
                Ok(Self {
                    mode: Mode::Local,
                    host,
                    port: port?,
                    database_url: get("DATABASE_URL").unwrap_or_else(|| LOCAL_DATABASE_URL.into()),
                    allowed_origins: match origins {
                        Some(list) => list?,
                        None => LOCAL_ORIGINS.iter().map(|item| item.to_string()).collect(),
                    },
                    pool,
                    limits,
                })
            }
            "oidc" => {
                let mut missing = Vec::new();
                let mut invalid = Vec::new();
                let mut required = |name: &'static str| {
                    get(name).unwrap_or_else(|| {
                        missing.push(name);
                        String::new()
                    })
                };
                let database_url = required("DATABASE_URL");
                let issuer = required("NEXUS_OIDC_ISSUER");
                let audience = required("NEXUS_OIDC_AUDIENCE");
                let client_id = required("NEXUS_OIDC_CLIENT_ID");
                let allowed_origins = match origins {
                    None => {
                        missing.push("NEXUS_ALLOWED_ORIGINS");
                        Vec::new()
                    }
                    Some(Ok(list)) => list,
                    Some(Err(why)) => {
                        invalid.push(why);
                        Vec::new()
                    }
                };
                if get("NEXUS_LOCAL_DEV").as_deref() == Some("1") {
                    invalid.push(
                        "NEXUS_LOCAL_DEV=1 cannot be combined with NEXUS_AUTH_MODE=oidc; remove it"
                            .into(),
                    );
                }
                if !issuer.is_empty() {
                    if let Err(why) = provider_url(&issuer) {
                        invalid.push(format!("NEXUS_OIDC_ISSUER {why}"));
                    }
                }
                let jwks_url = get("NEXUS_OIDC_JWKS_URL");
                if let Some(Err(why)) = jwks_url.as_deref().map(provider_url) {
                    invalid.push(format!("NEXUS_OIDC_JWKS_URL {why}"));
                }
                let scopes = get("NEXUS_OIDC_SCOPES").unwrap_or_else(|| DEFAULT_SCOPES.into());
                if !scopes.split_whitespace().any(|scope| scope == "openid")
                    || !scopes.split_whitespace().any(|scope| scope == "email")
                {
                    invalid.push("NEXUS_OIDC_SCOPES must include openid and email".into());
                }
                if let Err(why) = port {
                    invalid.push(why.into());
                }
                let (pool, limits) = tuning.unwrap_or_else(|problems| {
                    invalid.extend(problems);
                    (PoolSettings::default(), limits::Settings::default())
                });
                if !missing.is_empty() || !invalid.is_empty() {
                    let mut message =
                        String::from("Hosted mode (NEXUS_AUTH_MODE=oidc) cannot start.");
                    if !missing.is_empty() {
                        message.push_str(&format!(" Missing: {}.", missing.join(", ")));
                    }
                    if !invalid.is_empty() {
                        message.push_str(&format!(" Invalid: {}.", invalid.join("; ")));
                    }
                    message.push_str(" See .env.example.");
                    return Err(message);
                }
                Ok(Self {
                    mode: Mode::Oidc(OidcSettings {
                        issuer,
                        audience,
                        client_id,
                        jwks_url,
                        scopes,
                    }),
                    host,
                    port: port.unwrap_or(8090),
                    database_url,
                    allowed_origins,
                    pool,
                    limits,
                })
            }
            _ => Err("NEXUS_AUTH_MODE must be local or oidc.".into()),
        }
    }

    pub fn hosted(&self) -> bool {
        matches!(self.mode, Mode::Oidc(_))
    }
}

/// Cross-origin access for the listed browser origins only. When hosted, a request
/// that names any other origin is refused outright instead of merely being left
/// without the headers a browser needs to read the answer.
pub fn cors(origins: &[String], hosted: bool) -> Cors {
    origins
        .iter()
        .fold(Cors::default(), |cors, origin| cors.allowed_origin(origin))
        .allowed_methods(vec!["GET", "POST", "PUT", "PATCH", "OPTIONS"])
        .allowed_headers(allowed_request_headers(hosted))
        .block_on_origin_mismatch(hosted)
        .max_age(600)
}

/// Request headers a browser may send across origins. The development actor header
/// is offered only in local mode, the one mode that reads it.
pub fn allowed_request_headers(hosted: bool) -> Vec<&'static str> {
    let mut headers = vec![
        "content-type",
        "authorization",
        "x-nexus-client",
        "x-nexus-consent",
    ];
    if !hosted {
        headers.push("x-nexus-actor");
    }
    headers
}

/// Headers for every response that does not set its own. The API serves data, never
/// pages, so nothing it returns may be framed, sniffed, cached or used to load content.
pub fn security_headers(hosted: bool) -> DefaultHeaders {
    let headers = DefaultHeaders::new()
        .add(("X-Content-Type-Options", "nosniff"))
        .add(("X-Frame-Options", "DENY"))
        .add(("Referrer-Policy", "no-referrer"))
        .add((
            "Content-Security-Policy",
            "default-src 'none'; frame-ancestors 'none'",
        ))
        .add(("Cache-Control", "no-store"));
    if hosted {
        headers.add((
            "Strict-Transport-Security",
            "max-age=31536000; includeSubDomains",
        ))
    } else {
        headers
    }
}

/// A database connection failure, described without the connection string.
pub fn database_failure(error: &sqlx::Error) -> String {
    let reason = match error {
        sqlx::Error::Configuration(_) => "DATABASE_URL is not a valid PostgreSQL connection string",
        sqlx::Error::Io(_) => "the database host could not be reached",
        sqlx::Error::Tls(_) => "the TLS handshake with the database failed",
        sqlx::Error::Database(_) => {
            "the database rejected the connection (check user, password and database name)"
        }
        sqlx::Error::PoolTimedOut => {
            "the database did not accept a connection in time (check that it is running and reachable)"
        }
        _ => "the connection failed",
    };
    format!("PostgreSQL is unavailable: {reason}.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config(pairs: &[(&str, &str)]) -> Result<ServerConfig, String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();
        ServerConfig::from_lookup(|name| map.get(name).cloned())
    }
    const HOSTED: [(&str, &str); 7] = [
        ("NEXUS_AUTH_MODE", "oidc"),
        ("HOST", "0.0.0.0"),
        (
            "DATABASE_URL",
            "postgres://user:secret-password@db.example.com/nexus",
        ),
        ("NEXUS_OIDC_ISSUER", "https://tenant.example.com/"),
        ("NEXUS_OIDC_AUDIENCE", "client-123"),
        ("NEXUS_OIDC_CLIENT_ID", "client-123"),
        ("NEXUS_ALLOWED_ORIGINS", "https://demo.example.com"),
    ];
    fn hosted_without(name: &str) -> Vec<(&'static str, &'static str)> {
        HOSTED
            .iter()
            .copied()
            .filter(|(key, _)| *key != name)
            .collect()
    }
    fn hosted_with(name: &'static str, value: &'static str) -> Vec<(&'static str, &'static str)> {
        let mut pairs = hosted_without(name);
        pairs.push((name, value));
        pairs
    }

    #[test]
    fn local_mode_keeps_the_development_guard() {
        let local = config(&[("NEXUS_LOCAL_DEV", "1")]).unwrap();
        assert!(matches!(local.mode, Mode::Local));
        assert_eq!((local.host.as_str(), local.port), ("127.0.0.1", 8090));
        assert_eq!(local.database_url, LOCAL_DATABASE_URL);
        assert_eq!(local.allowed_origins, LOCAL_ORIGINS);
        // Unset mode is local, and local still needs the explicit opt-in.
        assert!(config(&[]).unwrap_err().contains("NEXUS_LOCAL_DEV=1"));
        assert!(config(&[("NEXUS_AUTH_MODE", "local")]).is_err());
        // Local actors never listen on a public address.
        let public = config(&[("NEXUS_LOCAL_DEV", "1"), ("HOST", "0.0.0.0")]).unwrap_err();
        assert!(public.contains("loopback HOST"));
        assert!(config(&[("NEXUS_AUTH_MODE", "open")]).is_err());
    }

    #[test]
    fn hosted_mode_accepts_a_complete_configuration() {
        let hosted = config(&HOSTED).unwrap();
        assert!(hosted.hosted());
        assert_eq!(hosted.host, "0.0.0.0");
        assert_eq!(hosted.allowed_origins, ["https://demo.example.com"]);
        let Mode::Oidc(settings) = hosted.mode else {
            panic!("expected hosted mode")
        };
        assert_eq!(settings.issuer, "https://tenant.example.com/");
        assert_eq!(settings.scopes, DEFAULT_SCOPES);
        assert!(settings.jwks_url.is_none());
    }

    #[test]
    fn hosted_mode_names_every_missing_variable_and_no_values() {
        let error = config(&[("NEXUS_AUTH_MODE", "oidc")]).unwrap_err();
        for name in [
            "DATABASE_URL",
            "NEXUS_OIDC_ISSUER",
            "NEXUS_OIDC_AUDIENCE",
            "NEXUS_OIDC_CLIENT_ID",
            "NEXUS_ALLOWED_ORIGINS",
        ] {
            assert!(error.contains(name), "{name} missing from: {error}");
        }
        // The local development database is never assumed when hosted.
        let error = config(&hosted_without("DATABASE_URL")).unwrap_err();
        assert!(error.contains("Missing: DATABASE_URL."));
        let error = config(&hosted_with(
            "NEXUS_OIDC_ISSUER",
            "http://tenant.example.com",
        ))
        .unwrap_err();
        assert!(error.contains("NEXUS_OIDC_ISSUER must use https"));
        assert!(!error.contains("secret-password"));
        // Blank is unset.
        assert!(config(&hosted_with("NEXUS_OIDC_AUDIENCE", "  ")).is_err());
    }

    #[test]
    fn hosted_mode_rejects_unsafe_combinations() {
        let mut both = HOSTED.to_vec();
        both.push(("NEXUS_LOCAL_DEV", "1"));
        assert!(config(&both)
            .unwrap_err()
            .contains("NEXUS_LOCAL_DEV=1 cannot be combined"));
        for bad in [
            "*",
            "http://demo.example.com",
            "https://demo.example.com/",
            "https://demo.example.com/app",
            "demo.example.com",
        ] {
            assert!(
                config(&hosted_with("NEXUS_ALLOWED_ORIGINS", bad)).is_err(),
                "{bad}"
            );
        }
        let two = config(&hosted_with(
            "NEXUS_ALLOWED_ORIGINS",
            "https://demo.example.com, http://127.0.0.1:5173",
        ))
        .unwrap();
        assert_eq!(two.allowed_origins.len(), 2);
        assert!(config(&hosted_with("NEXUS_OIDC_SCOPES", "openid profile")).is_err());
        assert!(config(&hosted_with("PORT", "0")).is_err());
    }

    #[test]
    fn pool_and_limit_settings_have_safe_defaults_in_both_modes() {
        for settings in [
            config(&[("NEXUS_LOCAL_DEV", "1")]).unwrap(),
            config(&HOSTED).unwrap(),
        ] {
            assert_eq!(settings.pool, PoolSettings::default());
            assert_eq!(settings.pool.max_connections, 10);
            assert_eq!(
                settings.pool.statement_timeout,
                Some(Duration::from_secs(30))
            );
            assert_eq!(settings.pool.lock_timeout, Some(Duration::from_secs(15)));
            assert_eq!(settings.limits, limits::Settings::default());
            assert!(settings.limits.enabled);
            // No forwarding header is believed unless the operator says how many
            // proxies stand in front of the API.
            assert_eq!(settings.limits.trusted_proxies, 0);
        }
    }

    #[test]
    fn pool_and_limit_settings_are_read_and_validated() {
        let mut pairs = HOSTED.to_vec();
        pairs.extend([
            ("NEXUS_DB_MAX_CONNECTIONS", "25"),
            ("NEXUS_DB_STATEMENT_TIMEOUT_SECONDS", "0"),
            ("NEXUS_DB_LOCK_TIMEOUT_SECONDS", "5"),
            ("NEXUS_TRUSTED_PROXIES", "2"),
            ("NEXUS_RATE_LIMIT_OPEN_PER_MINUTE", "90"),
            ("NEXUS_RATE_LIMIT_OPEN_BURST", "30"),
            ("NEXUS_RATE_LIMIT_PER_MINUTE", "900"),
            ("NEXUS_RATE_LIMIT_BURST", "120"),
        ]);
        let hosted = config(&pairs).unwrap();
        assert_eq!(hosted.pool.max_connections, 25);
        assert_eq!(hosted.pool.statement_timeout, None);
        assert_eq!(hosted.pool.lock_timeout, Some(Duration::from_secs(5)));
        assert_eq!(hosted.limits.trusted_proxies, 2);
        assert_eq!(
            (hosted.limits.open.per_minute, hosted.limits.open.burst),
            (90, 30)
        );
        assert_eq!(
            (hosted.limits.all.per_minute, hosted.limits.all.burst),
            (900, 120)
        );

        // Every bad value is named, and no value is echoed.
        let mut bad = HOSTED.to_vec();
        bad.extend([
            ("NEXUS_DB_MAX_CONNECTIONS", "0"),
            ("NEXUS_TRUSTED_PROXIES", "many-9431"),
            ("NEXUS_RATE_LIMIT_BURST", "-7"),
        ]);
        let error = config(&bad).unwrap_err();
        for name in [
            "NEXUS_DB_MAX_CONNECTIONS",
            "NEXUS_TRUSTED_PROXIES",
            "NEXUS_RATE_LIMIT_BURST",
        ] {
            assert!(error.contains(name), "{name} missing from: {error}");
        }
        assert!(!error.contains("many-9431"));
        let local = config(&[("NEXUS_LOCAL_DEV", "1"), ("NEXUS_DB_MAX_CONNECTIONS", "x")]);
        assert!(local.unwrap_err().contains("NEXUS_DB_MAX_CONNECTIONS"));
    }

    #[test]
    fn rate_limiting_can_be_turned_off_only_in_local_mode() {
        let local = config(&[("NEXUS_LOCAL_DEV", "1"), ("NEXUS_RATE_LIMIT", "off")]).unwrap();
        assert!(!local.limits.enabled);
        let local = config(&[("NEXUS_LOCAL_DEV", "1"), ("NEXUS_RATE_LIMIT", "on")]).unwrap();
        assert!(local.limits.enabled);
        let hosted = config(&hosted_with("NEXUS_RATE_LIMIT", "off")).unwrap_err();
        assert!(hosted.contains("accepted only in local mode"), "{hosted}");
        assert!(config(&hosted_with("NEXUS_RATE_LIMIT", "on")).is_ok());
        assert!(config(&hosted_with("NEXUS_RATE_LIMIT", "maybe")).is_err());
    }

    #[test]
    fn the_development_actor_header_is_offered_only_in_local_mode() {
        assert!(allowed_request_headers(false).contains(&"x-nexus-actor"));
        assert!(!allowed_request_headers(true).contains(&"x-nexus-actor"));
        for header in [
            "content-type",
            "authorization",
            "x-nexus-client",
            "x-nexus-consent",
        ] {
            assert!(allowed_request_headers(true).contains(&header));
            assert!(allowed_request_headers(false).contains(&header));
        }
    }

    #[test]
    fn database_failures_do_not_echo_the_connection_string() {
        let error = sqlx::Error::Configuration("postgres://user:secret-password@host/db".into());
        assert!(!database_failure(&error).contains("secret-password"));
    }
}
