//! Nexus API server. `NEXUS_AUTH_MODE=local` is loopback development with selected
//! operators; `NEXUS_AUTH_MODE=oidc` is the hosted mode with identity-provider sign-in.
use actix_web::{web, App, HttpServer};
use emr_api::{
    auth::{Auth, Oidc},
    configure,
    hosting::{self, Mode, ServerConfig},
    AppState,
};
use sqlx::postgres::PgPoolOptions;
use std::{io, process::ExitCode, sync::Arc};

/// Print a startup failure as one plain sentence, the way it should read in a
/// container log, instead of the debug form of an error.
#[actix_web::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Nexus API did not start: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> io::Result<()> {
    dotenvy::dotenv().ok();
    env_logger::init();
    let config = ServerConfig::from_env().map_err(io::Error::other)?;
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await
        .map_err(|error| io::Error::other(hosting::database_failure(&error)))?;
    emr_api::migrate(&pool).await.map_err(io::Error::other)?;
    let auth = match &config.mode {
        Mode::Local => Auth::Local,
        Mode::Oidc(settings) => Auth::Oidc(Arc::new(
            Oidc::connect(settings.clone())
                .await
                .map_err(io::Error::other)?,
        )),
    };
    emr_api::start_background_workers(pool.clone());
    let (host, port) = (config.host.clone(), config.port);
    if config.hosted() {
        println!(
            "Nexus hosted API: listening on {host}:{port} (identity-provider sign-in required; synthetic demonstration data only)"
        );
    } else {
        println!(
            "Nexus local API: http://{host}:{port} (authentication deferred; development actors only)"
        );
    }
    println!(
        "Optional local whisper.cpp: {} (not a production speech service; see docs/setup/local-speech.md)",
        emr_api::speech_configuration_label()
    );
    let hosted = config.hosted();
    let origins = config.allowed_origins.clone();
    HttpServer::new(move || {
        App::new()
            .wrap(hosting::security_headers(hosted))
            .wrap(hosting::cors(&origins, hosted))
            .app_data(web::Data::new(AppState {
                pool: pool.clone(),
                auth: auth.clone(),
            }))
            .app_data(web::JsonConfig::default().limit(1_048_576))
            .app_data(web::PayloadConfig::new(emr_api::MAX_WAV_BYTES))
            .configure(configure)
    })
    .bind((host, port))?
    .run()
    .await
}
