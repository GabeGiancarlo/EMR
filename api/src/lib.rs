//! Persistent Nexus application API. Local actors exercise policies, not authentication.
mod access;
mod adult;
mod applicability;
mod clinical;
mod exports;
mod forms;
mod readiness;
mod records;
mod routes;
mod speech;
mod workflow;

use actix_web::{http::StatusCode, web, HttpResponse, ResponseError};
use serde_json::json;
use sqlx::PgPool;

pub struct AppState {
    pub pool: PgPool,
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
        HttpResponse::build(self.status)
            .json(json!({"error":{"code":self.code,"message":self.message}}))
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
    sqlx::migrate!("../migrations").run(pool).await
}
pub use speech::MAX_WAV_BYTES;

pub fn speech_configuration_label() -> &'static str {
    speech::configuration_label()
}
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/healthz", web::get().to(routes::health))
        .service(
            web::scope("/api")
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
