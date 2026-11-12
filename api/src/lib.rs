//! Persistent Nexus application API. Local actors exercise policies, not authentication;
//! hosted mode authenticates staff through an OIDC identity provider (see `auth`).
mod access;
mod admissions;
mod adult;
mod applicability;
pub mod auth;
mod availability;
mod clinical;
mod clinical_model;
pub mod db;
mod drafting;
mod exports;
mod forms;
pub mod hosting;
mod knowledge;
pub mod limits;
mod media_transcription;
mod note_lineage;
mod patient_identity;
mod programs;
mod readiness;
mod records;
mod routes;
mod scheduling;
mod service_codes;
mod speech;
mod telehealth;
pub mod workers;
mod workflow;

use actix_web::{http::StatusCode, web, HttpResponse, ResponseError};
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub auth: auth::Auth,
    /// Per-client request limits for `/api`. `None` turns them off.
    pub limits: Option<Arc<limits::RateLimiter>>,
    /// The background workers started in this process, reported by `/healthz`.
    pub workers: workers::Registry,
}
impl AppState {
    /// Loopback development: operators are selected, not authenticated.
    pub fn local(pool: PgPool) -> Self {
        Self {
            pool,
            auth: auth::Auth::Local,
            limits: None,
            workers: workers::Registry::default(),
        }
    }
    /// Hosted: staff are identified by tokens from the configured identity provider.
    pub fn oidc(pool: PgPool, provider: Arc<auth::Oidc>) -> Self {
        Self {
            auth: auth::Auth::Oidc(provider),
            ..Self::local(pool)
        }
    }
    /// Apply per-client request limits. Settings with limiting turned off apply none.
    pub fn with_rate_limits(mut self, settings: &limits::Settings) -> Self {
        self.limits = limits::RateLimiter::from_settings(settings).map(Arc::new);
        self
    }
    pub fn with_workers(mut self, workers: workers::Registry) -> Self {
        self.workers = workers;
        self
    }
}
/// Start the supervised background workers. Each restarts itself with a growing
/// pause when its database session is lost, and waits when another instance owns
/// its queue. The returned registry is what `/healthz` reports.
pub fn start_background_workers(pool: PgPool) -> workers::Registry {
    let transcription = {
        let pool = pool.clone();
        workers::spawn("transcription", workers::Policy::STANDARD, move |status| {
            telehealth::recording::transcripts::worker::run(pool.clone(), status)
        })
    };
    let drafting = workers::spawn("drafting", workers::Policy::STANDARD, move |status| {
        drafting::worker::run(pool.clone(), status)
    });
    workers::registry(vec![transcription, drafting])
}
pub type Result<T> = std::result::Result<T, ApiError>;

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}
impl ApiError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "validation",
            message: message.into(),
        }
    }
    /// No usable proof of identity. The browser should sign in again.
    pub fn unauthenticated() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthenticated",
            message: "Sign in to continue.".into(),
        }
    }
    /// A genuine sign-in that maps to no usable staff account. Deliberately the same
    /// for unknown, inactive and unverified accounts.
    pub fn no_access() -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "no_access",
            message: "This account does not have access to this workspace.".into(),
        }
    }
    pub fn denied() -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: "Your selected role does not permit this action.".into(),
        }
    }
    pub fn missing() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: "Record not found in this workspace.".into(),
        }
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "conflict",
            message: message.into(),
        }
    }
    pub fn precondition(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::PRECONDITION_REQUIRED,
            code: "precondition_required",
            message: message.into(),
        }
    }
}
impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        self.status
    }
    fn error_response(&self) -> HttpResponse {
        let mut response = HttpResponse::build(self.status);
        if self.status == StatusCode::UNAUTHORIZED {
            response.insert_header(("WWW-Authenticate", "Bearer"));
        }
        if matches!(
            self.code,
            "database_busy" | "database_timeout" | "database_unavailable"
        ) {
            response.insert_header(("Retry-After", "2"));
        }
        response.json(json!({"error":{"code":self.code,"message":self.message}}))
    }
}
impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db) = &err {
            return match db.code().as_deref() {
                Some("23505") => {
                    Self::conflict("A record with this unique reference already exists.")
                }
                Some("23503") => Self::invalid(
                    "Linked records must exist and belong to the same organization and client.",
                ),
                Some("23502") => Self::invalid("A required field is missing."),
                Some("23514" | "22007" | "22008" | "22P02" | "22003") => {
                    Self::invalid("A value, date, amount, or record status is invalid.")
                }
                Some("P0001") => Self::conflict(
                    "This record is immutable; use the appropriate amendment workflow.",
                ),
                // lock_timeout: another request held a lock this one needed.
                Some("55P03") => Self {
                    status: StatusCode::SERVICE_UNAVAILABLE,
                    code: "database_busy",
                    message: "The workspace is busy. Nothing was changed; try again in a moment."
                        .into(),
                },
                // statement_timeout: the statement was cancelled before it finished.
                Some("57014") => {
                    log::warn!("A database statement exceeded its time limit");
                    Self {
                        status: StatusCode::SERVICE_UNAVAILABLE,
                        code: "database_timeout",
                        message: "The request took too long and was stopped. Nothing was changed; try again in a moment.".into(),
                    }
                }
                _ => {
                    log::error!("Database operation failed (SQLSTATE {:?})", db.code());
                    Self {
                        status: StatusCode::INTERNAL_SERVER_ERROR,
                        code: "database",
                        message: "Database operation could not be completed.".into(),
                    }
                }
            };
        }
        log::error!("Database connectivity or query execution failed");
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "database_unavailable",
            message: "The database is temporarily unavailable.".into(),
        }
    }
}
pub async fn migrate(pool: &PgPool) -> std::result::Result<(), sqlx::migrate::MigrateError> {
    use sqlx::{migrate::Migrate, Connection};
    // A dedicated connection cannot return a session advisory lock to the pool
    // on an error. Coordinate the compatibility check with SQLx's migration lock.
    // Dedicated also means no request timeouts: a migration may run long, and a
    // second instance starting at the same time waits for the first to finish.
    let mut conn = db::dedicated(pool).await?;
    conn.lock().await?;
    let result = async {
        conn.ensure_migrations_table().await?;
        let applied = conn.list_applied_migrations().await?;
        let pending_identity = !applied
            .iter()
            .any(|m| m.version == service_codes::IDENTITY_MIGRATION);
        let mut migrator = sqlx::migrate!("../migrations");
        migrator.set_locking(false); // The same SQLx lock is already held above.
        if pending_identity {
            let mut tx = conn.begin().await?;
            service_codes::preflight(&mut tx).await.map_err(|error| {
                sqlx::migrate::MigrateError::Execute(sqlx::Error::Protocol(error.message))
            })?;
            migrator.run_direct(&mut *tx).await?;
            tx.commit().await?;
        } else {
            migrator.run_direct(&mut conn).await?;
        }
        Ok::<_, sqlx::migrate::MigrateError>(())
    }
    .await;
    let unlocked = conn.unlock().await;
    let closed = conn.close().await;
    result?;
    unlocked?;
    closed?;
    Ok(())
}
pub use speech::MAX_WAV_BYTES;

pub fn speech_configuration_label() -> &'static str {
    speech::configuration_label()
}
pub fn configure(cfg: &mut web::ServiceConfig) {
    // Liveness: the process is serving. It never touches the database.
    cfg.route("/livez", web::get().to(routes::live))
        // Readiness: the database answers. Worker states are reported, not judged.
        .route("/healthz", web::get().to(routes::health))
        .service(
            web::scope("/api")
                // Always installed, so hosted mode cannot be served without it.
                .wrap(actix_web::middleware::from_fn(auth::gate))
                // Registered after the gate, so it runs before it: a refused request
                // costs no token verification and no database query.
                .wrap(actix_web::middleware::from_fn(limits::limit))
                .route("/auth/config", web::get().to(auth::config))
                .configure(telehealth::configure)
                .configure(knowledge::configure)
                .configure(drafting::configure)
                .configure(patient_identity::configure)
                .configure(admissions::configure)
                .configure(programs::configure)
                .route(
                    "/exports/accounting.csv",
                    web::get().to(exports::accounting),
                )
                .route("/exports/claims.csv", web::get().to(exports::claims))
                .route(
                    "/clients/{id}/form-requirements",
                    web::get().to(applicability::client_requirements),
                )
                .route("/setup", web::post().to(routes::setup))
                .route("/bootstrap", web::get().to(routes::bootstrap))
                .route(
                    "/scheduling/unavailability",
                    web::get().to(availability::list),
                )
                .route(
                    "/scheduling/unavailability",
                    web::post().to(availability::create),
                )
                .route(
                    "/scheduling/unavailability/{id}",
                    web::patch().to(availability::update),
                )
                .service(
                    web::resource("/speech/transcribe")
                        .app_data(web::PayloadConfig::new(speech::MAX_WAV_BYTES))
                        .route(web::post().to(speech::transcribe)),
                )
                .route(
                    "/claims/{id}/readiness",
                    web::get().to(readiness::claim_readiness),
                )
                .route(
                    "/{resource}/{id}/{action}",
                    web::post().to(workflow::action),
                )
                .route("/{resource}", web::get().to(routes::list))
                .route("/{resource}", web::post().to(routes::create))
                .route("/{resource}/{id}", web::patch().to(routes::update)),
        );
}
