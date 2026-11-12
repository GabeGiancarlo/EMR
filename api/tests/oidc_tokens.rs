//! Token verification against a provider that exists only inside this test: the
//! signing keys are generated here and published by a loopback HTTP server.
//! No database is needed; the staff lookup is covered in `oidc_postgres`.
#[path = "support/oidc.rs"]
mod oidc;

use emr_api::auth::{Oidc, OidcSettings, Rejection, RetryPolicy};
use jsonwebtoken::{encode, get_current_timestamp, Algorithm, EncodingKey, Header};
use oidc::{Provider, SigningKey, AUDIENCE};
use serde_json::json;
use std::time::{Duration, Instant};

const EMAIL: &str = "clinician@example.invalid";

fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for index in 0..=chunk.len() {
            out.push(ALPHABET[((n >> (18 - 6 * index)) & 63) as usize] as char);
        }
    }
    out
}

#[actix_web::test]
async fn a_valid_token_yields_the_verified_email() {
    let key = SigningKey::generate("key-1");
    let provider = Provider::start(&[&key]).await;
    let verifier = provider.connect().await;
    assert_eq!(
        verifier.verify(&key.sign(&provider.claims(EMAIL))).await,
        Ok(EMAIL.to_string())
    );
    // Email comparison is case-insensitive downstream, so it is normalised here.
    let mut claims = provider.claims("  Clinician@Example.INVALID ");
    assert_eq!(
        verifier.verify(&key.sign(&claims)).await,
        Ok(EMAIL.to_string())
    );
    // Some providers send the verified flag as a string.
    claims = provider.claims(EMAIL);
    claims["email_verified"] = json!("true");
    assert_eq!(
        verifier.verify(&key.sign(&claims)).await,
        Ok(EMAIL.to_string())
    );
    // An audience list that includes this application is accepted.
    claims = provider.claims(EMAIL);
    claims["aud"] = json!(["another-audience", AUDIENCE]);
    assert_eq!(
        verifier.verify(&key.sign(&claims)).await,
        Ok(EMAIL.to_string())
    );
    let config = verifier.public_config();
    assert_eq!(config["mode"], "oidc");
    assert_eq!(config["client_id"], AUDIENCE);
    assert_eq!(
        config["token_endpoint"],
        format!("{}/token", provider.issuer)
    );
}

#[actix_web::test]
async fn wrong_issuer_audience_and_time_window_are_rejected() {
    let key = SigningKey::generate("key-1");
    let provider = Provider::start(&[&key]).await;
    let verifier = provider.connect().await;
    let now = get_current_timestamp();
    let cases = [
        (
            "wrong issuer",
            "iss",
            json!("https://another-provider.example.com/"),
        ),
        (
            "issuer with a trailing slash",
            "iss",
            json!(format!("{}/", provider.issuer)),
        ),
        ("missing issuer", "iss", json!(null)),
        ("wrong audience", "aud", json!("another-application")),
        ("missing audience", "aud", json!(null)),
        ("expired", "exp", json!(now - 3600)),
        ("missing expiry", "exp", json!(null)),
        ("not yet valid", "nbf", json!(now + 3600)),
    ];
    for (name, claim, value) in cases {
        let mut claims = provider.claims(EMAIL);
        if value.is_null() {
            claims.as_object_mut().unwrap().remove(claim);
        } else {
            claims[claim] = value;
        }
        assert_eq!(
            verifier.verify(&key.sign(&claims)).await,
            Err(Rejection::Unauthenticated),
            "{name}"
        );
    }
    // Small clock differences between provider and API are tolerated.
    let mut claims = provider.claims(EMAIL);
    claims["exp"] = json!(now - 20);
    claims["nbf"] = json!(now + 20);
    assert!(verifier.verify(&key.sign(&claims)).await.is_ok());
}

#[actix_web::test]
async fn bad_signatures_and_unsafe_algorithms_are_rejected() {
    let key = SigningKey::generate("key-1");
    let provider = Provider::start(&[&key]).await;
    let verifier = provider.connect().await;
    let claims = provider.claims(EMAIL);

    // Signed by a key the provider never published, reusing the published key id.
    let impostor = SigningKey::generate("key-1");
    assert_eq!(
        verifier.verify(&impostor.sign(&claims)).await,
        Err(Rejection::Unauthenticated),
        "foreign key"
    );
    // A genuine token whose payload was altered after signing.
    let genuine = key.sign(&claims);
    let parts: Vec<&str> = genuine.split('.').collect();
    let mut altered = provider.claims("admin@example.invalid");
    altered["exp"] = claims["exp"].clone();
    let tampered = format!(
        "{}.{}.{}",
        parts[0],
        base64url(altered.to_string().as_bytes()),
        parts[2]
    );
    assert_eq!(
        verifier.verify(&tampered).await,
        Err(Rejection::Unauthenticated),
        "tampered payload"
    );
    // The public key replayed as a shared secret (algorithm confusion).
    let modulus = key.public["n"].as_str().unwrap();
    let mut header = Header::new(Algorithm::HS256);
    header.kid = Some("key-1".into());
    let confused = encode(
        &header,
        &claims,
        &EncodingKey::from_secret(modulus.as_bytes()),
    )
    .unwrap();
    assert_eq!(
        verifier.verify(&confused).await,
        Err(Rejection::Unauthenticated),
        "HS256"
    );
    // An unsigned token.
    let unsigned = format!(
        "{}.{}.",
        base64url(br#"{"alg":"none","typ":"JWT","kid":"key-1"}"#),
        base64url(claims.to_string().as_bytes())
    );
    assert_eq!(
        verifier.verify(&unsigned).await,
        Err(Rejection::Unauthenticated),
        "alg none"
    );
    for garbage in ["", "not-a-token", "a.b.c", &"x".repeat(20_000)] {
        assert_eq!(
            verifier.verify(garbage).await,
            Err(Rejection::Unauthenticated)
        );
    }
}

#[actix_web::test]
async fn an_unverified_or_absent_email_is_refused() {
    let key = SigningKey::generate("key-1");
    let provider = Provider::start(&[&key]).await;
    let verifier = provider.connect().await;
    for (name, claim, value) in [
        ("flag false", "email_verified", json!(false)),
        ("flag absent", "email_verified", json!(null)),
        ("flag is the string false", "email_verified", json!("false")),
        ("flag is a number", "email_verified", json!(1)),
        ("email absent", "email", json!(null)),
        ("email blank", "email", json!("  ")),
    ] {
        let mut claims = provider.claims(EMAIL);
        if value.is_null() {
            claims.as_object_mut().unwrap().remove(claim);
        } else {
            claims[claim] = value;
        }
        assert_eq!(
            verifier.verify(&key.sign(&claims)).await,
            Err(Rejection::Forbidden),
            "{name}"
        );
    }
}

#[actix_web::test]
async fn rotated_keys_are_picked_up_and_retired_keys_stop_working() {
    let first = SigningKey::generate("key-1");
    let second = SigningKey::generate("key-2");
    let provider = Provider::start(&[&first]).await;
    let verifier = provider
        .connect()
        .await
        .with_refresh_cooldown(Duration::ZERO);
    let claims = provider.claims(EMAIL);
    assert!(verifier.verify(&first.sign(&claims)).await.is_ok());
    assert_eq!(provider.key_downloads(), 1, "keys are cached after startup");

    // A token from a key that is not published yet is refused.
    assert_eq!(
        verifier.verify(&second.sign(&claims)).await,
        Err(Rejection::Unauthenticated)
    );
    // The provider rotates: the new key id triggers a download and is accepted.
    provider.publish(&[&second]);
    assert!(verifier.verify(&second.sign(&claims)).await.is_ok());
    let after_rotation = provider.key_downloads();
    assert!(verifier.verify(&second.sign(&claims)).await.is_ok());
    assert_eq!(
        provider.key_downloads(),
        after_rotation,
        "no download per request"
    );
    // The retired key no longer verifies anything.
    assert_eq!(
        verifier.verify(&first.sign(&claims)).await,
        Err(Rejection::Unauthenticated)
    );
}

#[actix_web::test]
async fn unknown_key_ids_cannot_force_repeated_downloads() {
    let key = SigningKey::generate("key-1");
    let provider = Provider::start(&[&key]).await;
    let verifier = provider.connect().await; // default cooldown
    let claims = provider.claims(EMAIL);
    for index in 0..5 {
        let stranger = SigningKey::generate(&format!("unknown-{index}"));
        assert_eq!(
            verifier.verify(&stranger.sign(&claims)).await,
            Err(Rejection::Unauthenticated)
        );
    }
    assert_eq!(provider.key_downloads(), 1);
    assert!(verifier.verify(&key.sign(&claims)).await.is_ok());
}

/// Review finding: with old keys and a provider that does not answer, requests whose
/// key was already held used to queue behind one download for its whole timeout.
#[actix_web::test]
async fn held_keys_never_wait_for_a_slow_provider() {
    let key = SigningKey::generate("key-1");
    let provider = Provider::start(&[&key]).await;
    // Every request sees the held keys as old and a refresh as allowed.
    let verifier = provider
        .connect()
        .await
        .with_key_max_age(Duration::ZERO)
        .with_refresh_cooldown(Duration::ZERO);
    let token = key.sign(&provider.claims(EMAIL));
    provider.delay_keys(Duration::from_secs(5));

    let started = Instant::now();
    for _ in 0..40 {
        assert_eq!(verifier.verify(&token).await, Ok(EMAIL.to_string()));
        // Let the background refresh start and reach the provider.
        actix_web::rt::time::sleep(Duration::from_millis(5)).await;
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(3),
        "40 verifications took {elapsed:?} while one key download was stalled for 5 s"
    );
    // One at startup, and exactly one refresh in flight however many requests arrived.
    assert_eq!(provider.key_downloads(), 2);
}

#[actix_web::test]
async fn held_keys_outlive_a_failing_or_unreachable_provider() {
    let key = SigningKey::generate("key-1");
    let rotated = SigningKey::generate("key-2");
    let provider = Provider::start(&[&key]).await;
    let verifier = provider
        .connect()
        .await
        .with_key_max_age(Duration::ZERO)
        .with_refresh_cooldown(Duration::from_millis(100));
    let token = key.sign(&provider.claims(EMAIL));

    // The provider answers with errors: sign-in keeps working on the held keys, and
    // the provider is asked no more than once per cooldown.
    provider.fail_keys(true);
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(450) {
        assert_eq!(verifier.verify(&token).await, Ok(EMAIL.to_string()));
        actix_web::rt::time::sleep(Duration::from_millis(10)).await;
    }
    let attempts = provider.key_downloads() - 1;
    assert!(
        (1..=6).contains(&attempts),
        "{attempts} downloads in 450 ms with a 100 ms cooldown"
    );

    // It recovers with rotated keys: the next refresh picks them up.
    provider.fail_keys(false);
    provider.publish(&[&key, &rotated]);
    actix_web::rt::time::sleep(Duration::from_millis(150)).await;
    assert!(verifier.verify(&token).await.is_ok());
    actix_web::rt::time::sleep(Duration::from_millis(100)).await;
    assert!(verifier
        .verify(&rotated.sign(&provider.claims(EMAIL)))
        .await
        .is_ok());

    // The provider disappears entirely: held keys still verify, promptly, and a key
    // id that is not held is refused instead of hanging.
    provider.stop().await;
    actix_web::rt::time::sleep(Duration::from_millis(150)).await;
    let started = Instant::now();
    for _ in 0..20 {
        assert_eq!(verifier.verify(&token).await, Ok(EMAIL.to_string()));
    }
    let stranger = SigningKey::generate("never-published");
    assert_eq!(
        verifier
            .verify(&stranger.sign(&provider.claims(EMAIL)))
            .await,
        Err(Rejection::Unauthenticated)
    );
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
}

#[actix_web::test]
async fn startup_fails_clearly_when_the_provider_does_not_match() {
    let key = SigningKey::generate("key-1");
    let provider = Provider::start(&[&key]).await;
    provider.report_issuer("https://real-tenant.example.com/");
    let error = Oidc::connect(provider.settings()).await.err().unwrap();
    assert!(error.contains("NEXUS_OIDC_ISSUER"), "{error}");
    assert!(
        error.contains("https://real-tenant.example.com/"),
        "{error}"
    );

    let mut insecure = provider.settings();
    insecure.issuer = "http://tenant.example.com".into();
    let error = Oidc::connect(insecure).await.err().unwrap();
    assert!(error.contains("must use https"), "{error}");

    let empty = Provider::start(&[]).await;
    let error = Oidc::connect(empty.settings()).await.err().unwrap();
    assert!(
        error.contains("no RSA or elliptic-curve signing key"),
        "{error}"
    );

    let mut unreachable = provider.settings();
    unreachable.issuer = "http://127.0.0.1:9".into();
    let error = Oidc::connect(unreachable).await.err().unwrap();
    assert!(error.contains("could not be reached"), "{error}");
}

/// A loopback port that nothing is listening on, so connections are refused.
fn closed_port() -> u16 {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.local_addr().unwrap().port()
}

fn quick_retries(patience: Duration) -> RetryPolicy {
    RetryPolicy {
        patience,
        first_pause: Duration::from_millis(100),
        longest_pause: Duration::from_millis(200),
    }
}

#[actix_web::test]
async fn startup_waits_for_a_provider_that_is_briefly_unreachable() {
    let key = SigningKey::generate("key-1");
    let port = closed_port();
    let settings = OidcSettings {
        issuer: format!("http://127.0.0.1:{port}"),
        audience: AUDIENCE.into(),
        client_id: AUDIENCE.into(),
        jwks_url: None,
        scopes: "openid email profile".into(),
    };
    // One attempt, as before, fails at once.
    let error = Oidc::connect(settings.clone()).await.err().unwrap();
    assert!(error.contains("could not be reached"), "{error}");

    // With retries, startup is still waiting when the provider comes up, and connects.
    let waiting = actix_web::rt::spawn(Oidc::connect_with_retry(
        settings,
        quick_retries(Duration::from_secs(20)),
    ));
    actix_web::rt::time::sleep(Duration::from_millis(350)).await;
    assert!(!waiting.is_finished(), "startup gave up or connected early");
    let provider = Provider::start_on(port, &[&key]).await;
    let oidc = waiting.await.unwrap().expect("connects once it answers");
    assert_eq!(
        oidc.verify(&key.sign(&provider.claims(EMAIL))).await,
        Ok(EMAIL.to_string())
    );
}

#[actix_web::test]
async fn startup_retries_a_failing_key_endpoint_and_then_gives_up_plainly() {
    let key = SigningKey::generate("key-1");
    let provider = Provider::start(&[&key]).await;
    provider.fail_keys(true);
    let began = Instant::now();
    let error = Oidc::connect_with_retry(
        provider.settings(),
        quick_retries(Duration::from_millis(300)),
    )
    .await
    .err()
    .unwrap();
    // Pauses of 100 ms and 200 ms fit the patience: three attempts in all, then the
    // same plain message a single attempt gives.
    assert!(error.contains("signing keys"), "{error}");
    assert!(error.contains("answered HTTP 500"), "{error}");
    assert!(began.elapsed() >= Duration::from_millis(300));
    assert_eq!(provider.key_downloads(), 3);

    // The fault clears while startup is waiting.
    let before = provider.key_downloads();
    let waiting = actix_web::rt::spawn(Oidc::connect_with_retry(
        provider.settings(),
        quick_retries(Duration::from_secs(20)),
    ));
    actix_web::rt::time::sleep(Duration::from_millis(250)).await;
    provider.fail_keys(false);
    assert!(waiting.await.unwrap().is_ok());
    assert!(provider.key_downloads() > before + 1);
}

#[actix_web::test]
async fn startup_does_not_retry_a_fault_that_waiting_cannot_cure() {
    let key = SigningKey::generate("key-1");
    let provider = Provider::start(&[&key]).await;
    provider.report_issuer("https://real-tenant.example.com/");
    // Pauses this long would make a retry obvious.
    let slow = RetryPolicy {
        patience: Duration::from_secs(120),
        first_pause: Duration::from_secs(30),
        longest_pause: Duration::from_secs(30),
    };
    let began = Instant::now();
    let error = Oidc::connect_with_retry(provider.settings(), slow)
        .await
        .err()
        .unwrap();
    assert!(error.contains("NEXUS_OIDC_ISSUER"), "{error}");
    assert!(began.elapsed() < Duration::from_secs(10));

    let empty = Provider::start(&[]).await;
    let began = Instant::now();
    let error = Oidc::connect_with_retry(empty.settings(), slow)
        .await
        .err()
        .unwrap();
    assert!(
        error.contains("no RSA or elliptic-curve signing key"),
        "{error}"
    );
    assert!(began.elapsed() < Duration::from_secs(10));
}
