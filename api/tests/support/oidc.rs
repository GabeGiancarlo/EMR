//! A throwaway identity provider for the OIDC tests: a loopback HTTP server that
//! publishes a discovery document and a key set, and signing keys generated inside
//! the test process. Nothing here is a fixture file or a reusable secret.
#![allow(dead_code)]
use actix_web::{web, App, HttpResponse, HttpServer};
use aws_lc_rs::{
    encoding::AsDer,
    rsa::{KeyPair, KeySize},
};
use emr_api::auth::{Oidc, OidcSettings};
use jsonwebtoken::{encode, get_current_timestamp, jwk::Jwk, Algorithm, EncodingKey, Header};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

pub const AUDIENCE: &str = "nexus-test-client";

/// A 2048-bit RSA key generated for one test, with its public JWK.
pub struct SigningKey {
    pub kid: String,
    pub private: EncodingKey,
    pub public: Value,
}
impl SigningKey {
    pub fn generate(kid: &str) -> Self {
        let pair = KeyPair::generate(KeySize::Rsa2048).expect("generate RSA key");
        let pkcs8 = AsDer::<aws_lc_rs::encoding::Pkcs8V1Der>::as_der(&pair).expect("encode key");
        // PKCS#8 wraps the PKCS#1 key jsonwebtoken signs with in a fixed 26-byte
        // header for a 2048-bit RSA key.
        let private = EncodingKey::from_rsa_der(&pkcs8.as_ref()[26..]);
        let mut jwk = Jwk::from_encoding_key(&private, Algorithm::RS256).expect("public JWK");
        jwk.common.key_id = Some(kid.to_string());
        Self {
            kid: kid.to_string(),
            private,
            public: serde_json::to_value(jwk).unwrap(),
        }
    }
    /// Sign `claims` as an RS256 token carrying this key's id.
    pub fn sign(&self, claims: &Value) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.kid.clone());
        encode(&header, claims, &self.private).expect("sign token")
    }
}

#[derive(Default)]
struct Published {
    keys: Vec<Value>,
    key_downloads: usize,
    reported_issuer: Option<String>,
}

pub struct Provider {
    pub issuer: String,
    published: Arc<Mutex<Published>>,
}
impl Provider {
    /// Start the provider on a free loopback port, publishing `keys`.
    pub async fn start(keys: &[&SigningKey]) -> Self {
        let published = Arc::new(Mutex::new(Published {
            keys: keys.iter().map(|key| key.public.clone()).collect(),
            ..Default::default()
        }));
        let shared = published.clone();
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let issuer = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let base = issuer.clone();
        let server = HttpServer::new(move || {
            let (base, discovery, keys) = (base.clone(), shared.clone(), shared.clone());
            App::new()
                .route(
                    "/.well-known/openid-configuration",
                    web::get().to(move || {
                        let reported = discovery.lock().unwrap().reported_issuer.clone();
                        let base = base.clone();
                        async move {
                            HttpResponse::Ok().json(json!({
                                "issuer": reported.unwrap_or_else(|| base.clone()),
                                "authorization_endpoint": format!("{base}/authorize"),
                                "token_endpoint": format!("{base}/token"),
                                "jwks_uri": format!("{base}/jwks"),
                                "end_session_endpoint": format!("{base}/logout"),
                            }))
                        }
                    }),
                )
                .route(
                    "/jwks",
                    web::get().to(move || {
                        let mut published = keys.lock().unwrap();
                        published.key_downloads += 1;
                        let body = json!({"keys": published.keys});
                        async move { HttpResponse::Ok().json(body) }
                    }),
                )
        })
        .workers(1)
        .listen(listener)
        .unwrap()
        .run();
        actix_web::rt::spawn(server);
        Self { issuer, published }
    }
    pub fn settings(&self) -> OidcSettings {
        OidcSettings {
            issuer: self.issuer.clone(),
            audience: AUDIENCE.into(),
            client_id: AUDIENCE.into(),
            jwks_url: None,
            scopes: "openid email profile".into(),
        }
    }
    pub async fn connect(&self) -> Oidc {
        Oidc::connect(self.settings())
            .await
            .expect("connect to test provider")
    }
    /// Replace the published key set, as a provider does when it rotates keys.
    pub fn publish(&self, keys: &[&SigningKey]) {
        self.published.lock().unwrap().keys = keys.iter().map(|key| key.public.clone()).collect();
    }
    pub fn key_downloads(&self) -> usize {
        self.published.lock().unwrap().key_downloads
    }
    /// Make the discovery document claim a different issuer.
    pub fn report_issuer(&self, issuer: &str) {
        self.published.lock().unwrap().reported_issuer = Some(issuer.into());
    }
    /// Claims for a token that is valid now for `email`.
    pub fn claims(&self, email: &str) -> Value {
        let now = get_current_timestamp();
        json!({
            "iss": self.issuer,
            "aud": AUDIENCE,
            "sub": format!("provider|{email}"),
            "iat": now,
            "nbf": now - 5,
            "exp": now + 600,
            "email": email,
            "email_verified": true,
        })
    }
}
