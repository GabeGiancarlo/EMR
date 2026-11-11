//! Token verification against a provider that exists only inside this test: the
//! signing keys are generated here and published by a loopback HTTP server.
//! No database is needed; the staff lookup is covered in `oidc_postgres`.
#[path = "support/oidc.rs"]
mod oidc;

use emr_api::auth::{Oidc, Rejection};
use jsonwebtoken::{encode, get_current_timestamp, Algorithm, EncodingKey, Header};
use oidc::{Provider, SigningKey, AUDIENCE};
use serde_json::json;
use std::time::Duration;

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
