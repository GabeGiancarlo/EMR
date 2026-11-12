//! Nexus API server. `NEXUS_AUTH_MODE=local` is loopback development with selected
//! operators; `NEXUS_AUTH_MODE=oidc` is the hosted mode with identity-provider sign-in.
use actix_web::{middleware::from_fn, web, App, HttpServer};
use emr_api::{
    auth::{Auth, Oidc, RetryPolicy},
    configure, db,
    hosting::{self, Mode, ServerConfig},
    observe, AppState,
};
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
    let pool = db::pool_options(&config.pool)
        .connect(&config.database_url)
        .await
        .map_err(|error| io::Error::other(hosting::database_failure(&error)))?;
    emr_api::migrate(&pool).await.map_err(io::Error::other)?;
    let auth = match &config.mode {
        Mode::Local => Auth::Local,
        // A provider that cannot be reached is retried for about two minutes, so a
        // restart during a brief provider fault does not crash-loop the service.
        Mode::Oidc(settings) => Auth::Oidc(Arc::new(
            Oidc::connect_with_retry(settings.clone(), RetryPolicy::STARTUP)
                .await
                .map_err(io::Error::other)?,
        )),
    };
    let workers = emr_api::start_background_workers(pool.clone());
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
    if !config.limits.enabled {
        println!("Request rate limiting is off (NEXUS_RATE_LIMIT=off; local mode only).");
    } else if config.hosted() && config.limits.trusted_proxies == 0 {
        // Behind a proxy every request arrives from the proxy's address, so all
        // clients would share one bucket.
        log::warn!(
            "NEXUS_TRUSTED_PROXIES is 0: request limits are counted per connecting address. If a proxy or load balancer fronts this API, set it to the number of proxies, or every client shares one limit."
        );
    }
    let hosted = config.hosted();
    let origins = config.allowed_origins.clone();
    // One state for every worker thread, so request limits and worker status are
    // shared by all of them.
    let state = AppState {
        pool,
        auth,
        limits: None,
        workers,
    }
    .with_rate_limits(&config.limits);
    HttpServer::new(move || {
        App::new()
            .wrap(hosting::security_headers(hosted))
            .wrap(hosting::cors(&origins, hosted))
            // Outermost: request id, access log, deadline and the error envelope.
            .wrap(from_fn(observe::track))
            .app_data(web::Data::new(state.clone()))
            .app_data(web::PayloadConfig::new(emr_api::MAX_WAV_BYTES))
            .configure(configure)
    })
    .client_request_timeout(hosting::CLIENT_REQUEST_TIMEOUT)
    .client_disconnect_timeout(hosting::CLIENT_DISCONNECT_TIMEOUT)
    .keep_alive(hosting::KEEP_ALIVE)
    .shutdown_timeout(hosting::SHUTDOWN_TIMEOUT_SECONDS)
    .bind((host, port))?
    .run()
    .await
}
