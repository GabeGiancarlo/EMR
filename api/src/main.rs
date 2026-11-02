//! Nexus local application. Identity-provider authentication is deliberately deferred.
use actix_web::{web, App, HttpServer};
use emr_api::{configure, AppState};
use sqlx::postgres::PgPoolOptions;
use std::{env, io};

#[actix_web::main]
async fn main() -> io::Result<()> {
    dotenvy::dotenv().ok();
    env_logger::init();
    if env::var("NEXUS_LOCAL_DEV").as_deref() != Ok("1") {
        return Err(io::Error::other(
            "Authentication is deferred. Set NEXUS_LOCAL_DEV=1 for loopback development only.",
        ));
    }
    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into());
    if !matches!(host.as_str(), "127.0.0.1" | "::1" | "localhost") {
        return Err(io::Error::other(
            "Local actor mode requires a loopback HOST.",
        ));
    }
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "8090".into())
        .parse()
        .map_err(io::Error::other)?;
    let url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://nexus:nexus_local@127.0.0.1:55432/nexus".into());
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&url)
        .await
        .map_err(io::Error::other)?;
    emr_api::migrate(&pool).await.map_err(io::Error::other)?;
    println!(
        "Nexus local API: http://{host}:{port} (authentication deferred; development actors only)"
    );
    println!(
        "Optional local whisper.cpp: {} (not a production speech service; see docs/setup/local-speech.md)",
        emr_api::speech_configuration_label()
    );
    HttpServer::new(move || {
        let cors = actix_cors::Cors::default()
            .allowed_origin("http://localhost:5173")
            .allowed_origin("http://127.0.0.1:5173")
            .allowed_methods(vec!["GET", "POST", "PATCH", "OPTIONS"])
            .allowed_headers(vec![
                "content-type",
                "x-nexus-actor",
                "x-nexus-client",
                "x-nexus-consent",
            ])
            .max_age(600);
        App::new()
            .wrap(cors)
            .app_data(web::Data::new(AppState { pool: pool.clone() }))
            .app_data(web::JsonConfig::default().limit(1_048_576))
            .app_data(web::PayloadConfig::new(emr_api::MAX_WAV_BYTES))
            .configure(configure)
    })
    .bind((host, port))?
    .run()
    .await
}
