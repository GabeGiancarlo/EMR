//! In-process request rate limiting for `/api`.
//!
//! Two token buckets per client address: a tighter one for the routes the staff gate
//! exempts (the unauthenticated surface: sign-in configuration, visit invitations and
//! recording uploads), and a looser one for everything. A refused request gets 429
//! with `Retry-After` and the normal error envelope.
//!
//! The limiter lives in this process. Several API instances each keep their own
//! counts, and a restart forgets them. It bounds what one address can ask of one
//! instance; it is not a substitute for a network-level limit.
use crate::{auth, ApiError, AppState};
use actix_web::{
    body::{EitherBody, MessageBody},
    dev::{ServiceRequest, ServiceResponse},
    http::{header, StatusCode},
    middleware::Next,
    web, HttpResponse,
};
use serde_json::json;
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv6Addr, SocketAddr},
    sync::{Mutex, PoisonError},
    time::{Duration, Instant},
};

/// Addresses tracked at once. Past this, new addresses are served without being
/// counted until idle entries have been swept, so the map cannot grow without bound.
const MAX_CLIENTS: usize = 100_000;
/// How often entries that have refilled completely are dropped.
const SWEEP_EVERY: Duration = Duration::from_secs(30);
/// When the map is full, sweep at most this often.
const FULL_SWEEP_EVERY: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rate {
    /// Sustained requests per minute.
    pub per_minute: u32,
    /// Requests allowed at once before the sustained rate applies.
    pub burst: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub enabled: bool,
    /// How many proxies in front of the API are trusted to report the address they
    /// received the request from. Zero trusts no forwarding header.
    pub trusted_proxies: usize,
    /// Routes that skip the staff gate.
    pub open: Rate,
    /// Every `/api` request.
    pub all: Rate,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            trusted_proxies: 0,
            // A visit polls once a second and bursts while a call connects, and several
            // participants can share one address.
            open: Rate {
                per_minute: 1200,
                burst: 240,
            },
            all: Rate {
                per_minute: 3000,
                burst: 600,
            },
        }
    }
}

struct Bucket {
    tokens: f64,
    updated: Instant,
}

struct Table {
    clients: HashMap<IpAddr, Bucket>,
    swept: Instant,
}

/// One token bucket per client address.
pub struct Limiter {
    per_second: f64,
    burst: f64,
    table: Mutex<Table>,
}

impl Limiter {
    pub fn new(rate: Rate) -> Self {
        Self {
            per_second: f64::from(rate.per_minute.max(1)) / 60.0,
            burst: f64::from(rate.burst.max(1)),
            table: Mutex::new(Table {
                clients: HashMap::new(),
                swept: Instant::now(),
            }),
        }
    }

    /// Spend one token for `client`. `Err` carries the seconds until one is available.
    pub fn take(&self, client: IpAddr, now: Instant) -> Result<(), u64> {
        let mut table = self.table.lock().unwrap_or_else(PoisonError::into_inner);
        if now.saturating_duration_since(table.swept) >= SWEEP_EVERY {
            self.sweep(&mut table, now);
        }
        if let Some(bucket) = table.clients.get_mut(&client) {
            let elapsed = now.saturating_duration_since(bucket.updated).as_secs_f64();
            bucket.tokens = (bucket.tokens + elapsed * self.per_second).min(self.burst);
            bucket.updated = now;
            if bucket.tokens >= 1.0 {
                bucket.tokens -= 1.0;
                return Ok(());
            }
            let wait = ((1.0 - bucket.tokens) / self.per_second).ceil();
            return Err((wait as u64).max(1));
        }
        if table.clients.len() >= MAX_CLIENTS {
            if now.saturating_duration_since(table.swept) >= FULL_SWEEP_EVERY {
                self.sweep(&mut table, now);
            }
            if table.clients.len() >= MAX_CLIENTS {
                return Ok(());
            }
        }
        table.clients.insert(
            client,
            Bucket {
                tokens: self.burst - 1.0,
                updated: now,
            },
        );
        Ok(())
    }

    /// Drop every bucket that has refilled: it is indistinguishable from a new one.
    fn sweep(&self, table: &mut Table, now: Instant) {
        let (per_second, burst) = (self.per_second, self.burst);
        table.clients.retain(|_, bucket| {
            let elapsed = now.saturating_duration_since(bucket.updated).as_secs_f64();
            bucket.tokens + elapsed * per_second < burst
        });
        table.swept = now;
    }

    pub fn tracked(&self) -> usize {
        self.table
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clients
            .len()
    }
}

pub struct RateLimiter {
    trusted_proxies: usize,
    open: Limiter,
    all: Limiter,
}

impl RateLimiter {
    /// `None` when rate limiting is turned off.
    pub fn from_settings(settings: &Settings) -> Option<Self> {
        settings.enabled.then(|| Self {
            trusted_proxies: settings.trusted_proxies,
            open: Limiter::new(settings.open),
            all: Limiter::new(settings.all),
        })
    }

    /// Count one request. `open` marks a route that skips the staff gate.
    pub fn check(&self, client: IpAddr, open: bool, now: Instant) -> Result<(), u64> {
        let client = bucket_key(client);
        self.all.take(client, now)?;
        if open {
            self.open.take(client, now)?;
        }
        Ok(())
    }
}

/// One bucket per IPv4 address and per IPv6 /64, because a single IPv6 customer
/// commonly holds a whole /64 and could otherwise use a fresh address per request.
fn bucket_key(address: IpAddr) -> IpAddr {
    match address {
        IpAddr::V4(_) => address,
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => {
                let mut segments = v6.segments();
                segments[4..].fill(0);
                IpAddr::V6(Ipv6Addr::from(segments))
            }
        },
    }
}

fn parse_hop(raw: &str) -> Option<IpAddr> {
    let raw = raw.trim();
    if let Ok(address) = raw.parse::<IpAddr>() {
        return Some(address);
    }
    if let Ok(socket) = raw.parse::<SocketAddr>() {
        return Some(socket.ip());
    }
    raw.strip_prefix('[')?.strip_suffix(']')?.parse().ok()
}

/// The address to hold responsible for a request.
///
/// Each proxy appends the address it received the request from to `X-Forwarded-For`,
/// so the chain reads client-first and the entries nearest the end were written by the
/// proxies nearest the API. With `trusted_proxies = n`, the last `n` hops (the
/// connecting peer and the `n - 1` entries before it) are our own proxies, and the
/// entry just before them is the rightmost address that was not written by something
/// we trust to be honest. Anything further left is whatever the client chose to send.
///
/// With `n = 0` no header is believed and the connecting peer is the client. When the
/// header is missing, too short or unreadable the connecting peer is used, which is
/// never something the caller can choose.
pub fn client_ip<'a>(
    forwarded_for: impl IntoIterator<Item = &'a str>,
    peer: Option<IpAddr>,
    trusted_proxies: usize,
) -> Option<IpAddr> {
    if trusted_proxies == 0 {
        return peer;
    }
    let hops: Vec<&str> = forwarded_for
        .into_iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|hop| !hop.is_empty())
        .collect();
    hops.len()
        .checked_sub(trusted_proxies)
        .and_then(|index| parse_hop(hops[index]))
        .or(peer)
}

fn refusal(retry_after: u64) -> HttpResponse {
    let error = ApiError {
        status: StatusCode::TOO_MANY_REQUESTS,
        code: "rate_limited",
        message: "Too many requests. Wait a moment and try again.".into(),
    };
    HttpResponse::build(error.status)
        .insert_header((header::RETRY_AFTER, retry_after.to_string()))
        .json(json!({"error": {"code": error.code, "message": error.message}}))
}

/// Wraps `/api`, outside the staff gate, so a refused request costs no token
/// verification and no database query.
pub(crate) async fn limit<B: MessageBody + 'static>(
    req: ServiceRequest,
    next: Next<B>,
) -> Result<ServiceResponse<EitherBody<B>>, actix_web::Error> {
    let limiter = req
        .app_data::<web::Data<AppState>>()
        .and_then(|state| state.limits.clone());
    if let Some(limiter) = limiter {
        let forwarded = req
            .headers()
            .get_all("x-forwarded-for")
            .filter_map(|value| value.to_str().ok());
        let peer = req.peer_addr().map(|address| address.ip());
        // Without any address there is nothing to key a bucket by; a TCP listener
        // always supplies one.
        if let Some(client) = client_ip(forwarded, peer, limiter.trusted_proxies) {
            let open = auth::carries_own_credential(req.path());
            if let Err(retry_after) = limiter.check(client, open, Instant::now()) {
                return Ok(req
                    .into_response(refusal(retry_after))
                    .map_into_right_body());
            }
        }
    }
    Ok(next.call(req).await?.map_into_left_body())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }
    const MINUTE: Rate = Rate {
        per_minute: 60,
        burst: 3,
    };

    #[test]
    fn a_bucket_allows_its_burst_then_refills_at_the_sustained_rate() {
        let limiter = Limiter::new(MINUTE);
        let start = Instant::now();
        let client = ip("203.0.113.7");
        for _ in 0..3 {
            assert_eq!(limiter.take(client, start), Ok(()));
        }
        // Empty: one token per second, so the wait is one second.
        assert_eq!(limiter.take(client, start), Err(1));
        assert_eq!(
            limiter.take(client, start + Duration::from_millis(400)),
            Err(1)
        );
        assert_eq!(
            limiter.take(client, start + Duration::from_millis(1000)),
            Ok(())
        );
        assert!(limiter
            .take(client, start + Duration::from_millis(1001))
            .is_err());
        // A long pause refills to the burst and no further.
        let later = start + Duration::from_secs(3600);
        for _ in 0..3 {
            assert_eq!(limiter.take(client, later), Ok(()));
        }
        assert!(limiter.take(client, later).is_err());
    }

    #[test]
    fn the_wait_reported_matches_a_slow_rate() {
        let limiter = Limiter::new(Rate {
            per_minute: 6,
            burst: 1,
        });
        let start = Instant::now();
        let client = ip("203.0.113.7");
        assert_eq!(limiter.take(client, start), Ok(()));
        assert_eq!(limiter.take(client, start), Err(10));
        assert_eq!(limiter.take(client, start + Duration::from_secs(4)), Err(6));
    }

    #[test]
    fn clients_do_not_share_a_bucket() {
        let limiter = Limiter::new(MINUTE);
        let start = Instant::now();
        for _ in 0..3 {
            limiter.take(ip("203.0.113.7"), start).unwrap();
        }
        assert!(limiter.take(ip("203.0.113.7"), start).is_err());
        assert_eq!(limiter.take(ip("203.0.113.8"), start), Ok(()));
    }

    #[test]
    fn refilled_buckets_are_swept_so_the_map_stays_bounded() {
        let limiter = Limiter::new(MINUTE);
        let start = Instant::now();
        for last in 0..200u8 {
            limiter
                .take(IpAddr::from([198, 51, 100, last]), start)
                .unwrap();
        }
        assert_eq!(limiter.tracked(), 200);
        // Each bucket spent one token, which refills in a second. The next request
        // after the sweep interval drops them all and tracks only itself.
        limiter
            .take(
                ip("203.0.113.7"),
                start + SWEEP_EVERY + Duration::from_secs(1),
            )
            .unwrap();
        assert_eq!(limiter.tracked(), 1);
    }

    #[test]
    fn a_full_map_serves_new_clients_without_tracking_them() {
        let limiter = Limiter::new(MINUTE);
        let start = Instant::now();
        {
            let mut table = limiter.table.lock().unwrap();
            for index in 0..MAX_CLIENTS as u32 {
                table.clients.insert(
                    IpAddr::from(0x0a00_0000u32.wrapping_add(index).to_be_bytes()),
                    Bucket {
                        tokens: 0.0,
                        updated: start,
                    },
                );
            }
        }
        assert_eq!(limiter.take(ip("203.0.113.7"), start), Ok(()));
        assert_eq!(limiter.tracked(), MAX_CLIENTS);
        // An address already tracked is still limited.
        assert!(limiter.take(ip("10.0.0.1"), start).is_err());
    }

    #[test]
    fn the_open_surface_has_its_own_tighter_bucket() {
        let limiter = RateLimiter::from_settings(&Settings {
            enabled: true,
            trusted_proxies: 0,
            open: Rate {
                per_minute: 60,
                burst: 2,
            },
            all: Rate {
                per_minute: 60,
                burst: 5,
            },
        })
        .unwrap();
        let start = Instant::now();
        let client = ip("203.0.113.7");
        assert_eq!(limiter.check(client, true, start), Ok(()));
        assert_eq!(limiter.check(client, true, start), Ok(()));
        assert!(limiter.check(client, true, start).is_err());
        // Staff routes still have room in the overall bucket...
        assert_eq!(limiter.check(client, false, start), Ok(()));
        assert_eq!(limiter.check(client, false, start), Ok(()));
        // ...until that is spent too. Five were counted: three open, two staff.
        assert!(limiter.check(client, false, start).is_err());
        assert!(RateLimiter::from_settings(&Settings {
            enabled: false,
            ..Settings::default()
        })
        .is_none());
    }

    #[test]
    fn ipv6_clients_are_counted_per_64_and_mapped_ipv4_as_ipv4() {
        assert_eq!(
            bucket_key(ip("2001:db8:1:2:aaaa:bbbb:cccc:dddd")),
            ip("2001:db8:1:2::")
        );
        assert_eq!(
            bucket_key(ip("2001:db8:1:2::1")),
            bucket_key(ip("2001:db8:1:2:ffff::9"))
        );
        assert_ne!(
            bucket_key(ip("2001:db8:1:2::1")),
            bucket_key(ip("2001:db8:1:3::1"))
        );
        assert_eq!(bucket_key(ip("::ffff:203.0.113.7")), ip("203.0.113.7"));
        assert_eq!(bucket_key(ip("203.0.113.7")), ip("203.0.113.7"));
    }

    #[test]
    fn no_forwarding_header_is_believed_by_default() {
        let peer = Some(ip("192.0.2.10"));
        assert_eq!(client_ip(["198.51.100.1"], peer, 0), peer);
        assert_eq!(client_ip([], peer, 0), peer);
        assert_eq!(client_ip(["198.51.100.1"], None, 0), None);
    }

    #[test]
    fn the_client_is_the_rightmost_hop_no_trusted_proxy_wrote() {
        let peer = Some(ip("192.0.2.10"));
        // One trusted proxy (the peer): it appended the address it saw.
        assert_eq!(
            client_ip(["198.51.100.1"], peer, 1),
            Some(ip("198.51.100.1"))
        );
        // A forged entry sits to the left of the one our proxy appended and is ignored.
        assert_eq!(
            client_ip(["10.9.9.9, 198.51.100.1"], peer, 1),
            Some(ip("198.51.100.1"))
        );
        // Two trusted proxies: the peer, and the one that wrote the last entry.
        assert_eq!(
            client_ip(["10.9.9.9, 198.51.100.1, 203.0.113.5"], peer, 2),
            Some(ip("198.51.100.1"))
        );
        // Repeated headers are one list, in order.
        assert_eq!(
            client_ip(["10.9.9.9", "198.51.100.1, 203.0.113.5"], peer, 2),
            Some(ip("198.51.100.1"))
        );
    }

    #[test]
    fn a_short_or_unreadable_chain_falls_back_to_the_connecting_peer() {
        let peer = Some(ip("192.0.2.10"));
        assert_eq!(client_ip([], peer, 1), peer);
        assert_eq!(client_ip(["198.51.100.1"], peer, 2), peer);
        assert_eq!(client_ip(["unknown"], peer, 1), peer);
        assert_eq!(client_ip(["198.51.100.1, _hidden"], peer, 1), peer);
        assert_eq!(client_ip([" , "], peer, 1), peer);
    }

    #[test]
    fn hops_may_carry_ports_and_brackets() {
        let peer = Some(ip("192.0.2.10"));
        assert_eq!(
            client_ip(["198.51.100.1:4711"], peer, 1),
            Some(ip("198.51.100.1"))
        );
        assert_eq!(
            client_ip(["[2001:db8::7]:4711"], peer, 1),
            Some(ip("2001:db8::7"))
        );
        assert_eq!(
            client_ip(["[2001:db8::7]"], peer, 1),
            Some(ip("2001:db8::7"))
        );
        assert_eq!(client_ip(["2001:db8::7"], peer, 1), Some(ip("2001:db8::7")));
    }

    #[actix_web::test]
    async fn the_api_refuses_a_client_over_its_limit_and_leaves_probes_alone() {
        use actix_web::{test as http_test, App};
        // Never connected: the routes used here do not touch the database.
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://nexus@127.0.0.1:1/unused")
            .unwrap();
        let state = AppState::local(pool).with_rate_limits(&Settings {
            enabled: true,
            trusted_proxies: 1,
            open: Rate {
                per_minute: 60,
                burst: 2,
            },
            all: Rate {
                per_minute: 60,
                burst: 50,
            },
        });
        let app = http_test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(crate::configure),
        )
        .await;
        let call = |path: &'static str, forwarded: &'static str| {
            let request = http_test::TestRequest::get()
                .uri(path)
                .peer_addr("192.0.2.10:4000".parse().unwrap())
                .insert_header(("x-forwarded-for", forwarded))
                .to_request();
            http_test::call_service(&app, request)
        };
        // `/api/auth/config` skips the staff gate, so it is on the open surface.
        for _ in 0..2 {
            let response = call("/api/auth/config", "198.51.100.1").await;
            assert_eq!(response.status(), StatusCode::OK);
        }
        // Over the limit, and a forged leading hop does not buy a fresh bucket.
        for forwarded in ["198.51.100.1", "10.1.2.3, 198.51.100.1"] {
            let response = call("/api/auth/config", forwarded).await;
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
            let retry: u64 = response
                .headers()
                .get(header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse().ok())
                .expect("Retry-After in seconds");
            assert!((1..=60).contains(&retry));
            let body: serde_json::Value = http_test::read_body_json(response).await;
            assert_eq!(body["error"]["code"], "rate_limited");
            assert!(body["error"]["message"].is_string());
        }
        // Another client is unaffected.
        let response = call("/api/auth/config", "198.51.100.2").await;
        assert_eq!(response.status(), StatusCode::OK);
        // Liveness is outside `/api` and is never limited: more calls than the looser
        // bucket's burst of 50, so this fails if the limiter ever wraps the whole app.
        for _ in 0..80 {
            let response = call("/livez", "198.51.100.1").await;
            assert_eq!(response.status(), StatusCode::OK);
        }
    }

    #[actix_web::test]
    async fn limits_are_off_unless_configured_and_ignore_forwarding_by_default() {
        use actix_web::{test as http_test, App};
        let lazy = || {
            sqlx::postgres::PgPoolOptions::new()
                .connect_lazy("postgres://nexus@127.0.0.1:1/unused")
                .unwrap()
        };
        let tight = Rate {
            per_minute: 60,
            burst: 1,
        };
        let request = |forwarded: &'static str| {
            http_test::TestRequest::get()
                .uri("/api/auth/config")
                .peer_addr("192.0.2.10:4000".parse().unwrap())
                .insert_header(("x-forwarded-for", forwarded))
                .to_request()
        };
        // Tests and local mode that never asked for limits get none.
        let unlimited = http_test::init_service(
            App::new()
                .app_data(web::Data::new(AppState::local(lazy())))
                .configure(crate::configure),
        )
        .await;
        for _ in 0..5 {
            let response = http_test::call_service(&unlimited, request("198.51.100.1")).await;
            assert_eq!(response.status(), StatusCode::OK);
        }
        // With no trusted proxy the header is not believed: changing it changes
        // nothing, because every request comes from the same connecting address.
        let state = AppState::local(lazy()).with_rate_limits(&Settings {
            enabled: true,
            trusted_proxies: 0,
            open: tight,
            all: tight,
        });
        let limited = http_test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(crate::configure),
        )
        .await;
        let first = http_test::call_service(&limited, request("198.51.100.1")).await;
        assert_eq!(first.status(), StatusCode::OK);
        let second = http_test::call_service(&limited, request("198.51.100.99")).await;
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn a_refusal_uses_the_error_envelope_and_says_when_to_retry() {
        let response = refusal(7);
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers().get(header::RETRY_AFTER).unwrap(), "7");
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/json"
        );
    }
}
