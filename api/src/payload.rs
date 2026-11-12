//! Request bodies, and the one envelope for everything that can go wrong reading them.
//!
//! Staff routes use the framework's extractors, with the error handlers below. The
//! routes that carry their own token (visit invitations, recording uploads) take the
//! raw payload instead, check the token, and only then call [`json`] or [`bytes`], so
//! an unauthenticated caller cannot make the server buffer a body.
use crate::{observe::envelope_for, ApiError, Result};
use actix_web::{
    error::{JsonPayloadError, PathError, QueryPayloadError},
    http::{header, StatusCode},
    web, HttpRequest,
};
use serde::de::DeserializeOwned;
use std::time::Duration;

/// JSON bodies on staff routes.
pub const JSON_LIMIT: usize = 1_048_576;
/// JSON bodies on visit routes. The largest legitimate one is a WebRTC session
/// description, which the signalling rules cap at 65,536 characters.
pub(crate) const VISIT_JSON_LIMIT: usize = 256 * 1024;
/// How long a client may take to deliver a body read by [`json`] or [`bytes`].
const BODY_READ_TIMEOUT: Duration = Duration::from_secs(60);

fn too_large() -> ApiError {
    envelope_for(StatusCode::PAYLOAD_TOO_LARGE)
}

fn invalid_json() -> ApiError {
    ApiError::invalid("The request body is not valid JSON for this action.")
}

fn not_json() -> ApiError {
    ApiError::invalid("Send this request as application/json.")
}

/// A JSON body the framework could not accept. The parser's own message names
/// fields and types, so it is logged nowhere and replaced with a fixed sentence.
pub(crate) fn json_error(error: &JsonPayloadError) -> ApiError {
    match error {
        JsonPayloadError::OverflowKnownLength { .. } | JsonPayloadError::Overflow { .. } => {
            too_large()
        }
        JsonPayloadError::ContentType => not_json(),
        JsonPayloadError::Deserialize(_) => invalid_json(),
        other => {
            use actix_web::ResponseError;
            envelope_for(other.status_code())
        }
    }
}

/// A path segment that is not the identifier the route expects is simply not found.
pub(crate) fn path_error(_error: &PathError) -> ApiError {
    ApiError::missing()
}

pub(crate) fn query_error(_error: &QueryPayloadError) -> ApiError {
    ApiError::invalid("The query string is not valid for this action.")
}

/// Read a body of at most `limit` bytes. Call it only after the caller's token has
/// been checked, and never while holding a database connection: the client decides
/// how slowly these bytes arrive.
pub(crate) async fn bytes(
    req: &HttpRequest,
    payload: web::Payload,
    limit: usize,
) -> Result<web::Bytes> {
    let declared = req
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok());
    if declared.is_some_and(|length| length > limit) {
        return Err(too_large());
    }
    match tokio::time::timeout(BODY_READ_TIMEOUT, payload.to_bytes_limited(limit)).await {
        Ok(Ok(Ok(body))) => Ok(body),
        Ok(Ok(Err(_))) => Err(ApiError::invalid("The request body could not be read.")),
        Ok(Err(_)) => Err(too_large()),
        Err(_) => Err(envelope_for(StatusCode::REQUEST_TIMEOUT)),
    }
}

/// Read and parse a JSON body, with the same answers the JSON extractor gives.
pub(crate) async fn json<T: DeserializeOwned>(
    req: &HttpRequest,
    payload: web::Payload,
    limit: usize,
) -> Result<T> {
    let json = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"));
    if !json {
        return Err(not_json());
    }
    let body = bytes(req, payload, limit).await?;
    serde_json::from_slice(&body).map_err(|_| invalid_json())
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App, HttpResponse};
    use serde::Deserialize;
    use serde_json::{json, Value};

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Body {
        name: String,
    }

    async fn typed(body: web::Json<Body>) -> HttpResponse {
        HttpResponse::Ok().json(json!({"name": body.name}))
    }
    async fn manual(req: HttpRequest, payload: web::Payload) -> Result<HttpResponse> {
        let body: Body = super::json(&req, payload, 64).await?;
        Ok(HttpResponse::Ok().json(json!({"name": body.name})))
    }
    async fn raw(req: HttpRequest, payload: web::Payload) -> Result<HttpResponse> {
        let body = bytes(&req, payload, 8).await?;
        Ok(HttpResponse::Ok().json(json!({"length": body.len()})))
    }

    async fn call(
        path: &str,
        content_type: Option<&str>,
        body: &'static str,
    ) -> (StatusCode, Value) {
        let app = test::init_service(
            App::new()
                .configure(crate::error_handlers)
                .route("/typed", web::post().to(typed))
                .route("/manual", web::post().to(manual))
                .route("/raw", web::put().to(raw))
                .route(
                    "/item/{id}",
                    web::get().to(|_: web::Path<uuid::Uuid>| async { "found" }),
                )
                .route(
                    "/search",
                    web::get().to(|_: web::Query<Body>| async { "found" }),
                ),
        )
        .await;
        let mut request = match path {
            "/raw" => test::TestRequest::put(),
            "/typed" | "/manual" => test::TestRequest::post(),
            _ => test::TestRequest::get(),
        }
        .uri(path)
        .set_payload(body);
        if let Some(content_type) = content_type {
            request = request.insert_header(("content-type", content_type));
        }
        let response = test::call_service(&app, request.to_request()).await;
        let status = response.status();
        let bytes = test::read_body(response).await;
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    #[actix_web::test]
    async fn extractor_and_manual_json_give_the_same_enveloped_answers() {
        let json_type = Some("application/json");
        for path in ["/typed", "/manual"] {
            let (status, body) = call(path, json_type, r#"{"name":"ok"}"#).await;
            assert_eq!(
                (status, &body["name"]),
                (StatusCode::OK, &json!("ok")),
                "{path}"
            );

            for bad in [r#"{"name":"#, r#"{"nam":"x"}"#, r#"{"name":1}"#, ""] {
                let (status, body) = call(path, json_type, bad).await;
                assert_eq!(status, StatusCode::BAD_REQUEST, "{path} {bad}");
                assert_eq!(body["error"]["code"], "validation", "{path} {bad}");
                // The parser's description of the expected shape is not repeated.
                let text = body.to_string();
                assert!(!text.contains("nam`") && !text.contains("line"), "{text}");
            }
            let (status, body) = call(path, Some("text/plain"), r#"{"name":"ok"}"#).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
            assert_eq!(body["error"]["code"], "validation");
            let (status, body) = call(path, None, r#"{"name":"ok"}"#).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
            assert_eq!(body["error"]["code"], "validation");
        }
        // The manual reader enforces its own limit.
        let long = r#"{"name":"0123456789012345678901234567890123456789012345678901234567890"}"#;
        let (status, body) = call("/manual", json_type, long).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(body["error"]["code"], "payload_too_large");
    }

    #[actix_web::test]
    async fn raw_bodies_are_bounded() {
        let (status, body) = call("/raw", None, "12345678").await;
        assert_eq!((status, &body["length"]), (StatusCode::OK, &json!(8)));
        let (status, body) = call("/raw", None, "123456789").await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(body["error"]["code"], "payload_too_large");
    }

    #[actix_web::test]
    async fn path_and_query_errors_use_the_envelope() {
        let (status, body) = call("/item/not-a-uuid", None, "").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
        assert!(!body.to_string().to_lowercase().contains("uuid"));
        let (status, body) = call("/search?nam=x", None, "").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "validation");
        assert!(!body.to_string().contains("nam"));
        let (status, _) = call("/search?name=x", None, "").await;
        assert_eq!(status, StatusCode::OK);
    }

    #[actix_web::test]
    async fn an_oversized_json_body_on_a_staff_route_is_refused_in_the_envelope() {
        let app = test::init_service(
            App::new()
                .configure(crate::error_handlers)
                .route("/typed", web::post().to(typed)),
        )
        .await;
        let request = test::TestRequest::post()
            .uri("/typed")
            .insert_header(("content-type", "application/json"))
            .set_payload(format!(r#"{{"name":"{}"}}"#, "x".repeat(JSON_LIMIT)))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["error"]["code"], "payload_too_large");
    }
}
