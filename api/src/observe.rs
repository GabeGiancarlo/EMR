//! The outermost request wrapper: a request id, the access log, a deadline, and the
//! guarantee that every error body is the API's JSON envelope.
//!
//! The access log records method, path, status, duration and the request id, and
//! nothing else. It never contains a header, a query string, a body, an email or any
//! record content, and identifiers in the path are replaced by `{id}`.
use crate::ApiError;
use actix_web::{
    body::{BoxBody, MessageBody},
    dev::{ServiceRequest, ServiceResponse},
    http::{
        header::{self, HeaderName, HeaderValue},
        StatusCode,
    },
    middleware::Next,
    HttpMessage, HttpRequest, HttpResponse,
};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// Longest a single request may take from its first byte to its response head,
/// including the time the client takes to send its body. Local speech transcription
/// (120 seconds, plus the upload) is the longest legitimate request.
pub const REQUEST_DEADLINE: Duration = Duration::from_secs(300);

/// The id generated for this request, readable from the request extensions.
#[derive(Clone, Debug)]
pub(crate) struct RequestId(pub String);

/// The request id for a log line, or `-` when the request did not pass through
/// [`track`] (as in tests that mount the routes alone).
pub(crate) fn request_id(req: &impl HttpMessage) -> String {
    req.extensions()
        .get::<RequestId>()
        .map_or_else(|| "-".to_string(), |id| id.0.clone())
}

fn opaque(segment: &str) -> bool {
    Uuid::parse_str(segment).is_ok()
        || segment.len() > 64
        || (segment.len() >= 16 && segment.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

/// The path as it may be logged: no query string (the caller passes the path alone),
/// record identifiers and token-like segments replaced, and nothing but plain
/// printable characters, so a request cannot forge or flood a log line.
pub fn logged_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len().min(200));
    for (index, segment) in path.split('/').enumerate() {
        if index > 0 {
            out.push('/');
        }
        if opaque(segment) {
            out.push_str("{id}");
        } else {
            out.extend(segment.chars().map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~' | '%') {
                    c
                } else {
                    '?'
                }
            }));
        }
        if out.len() >= 200 {
            out.truncate(200);
            out.push('…');
            break;
        }
    }
    out
}

/// One generic envelope per status, for errors raised outside the application's own
/// handlers (an unmatched method, an oversized body, a refused origin). The text says
/// nothing about how the server is built.
pub fn envelope_for(status: StatusCode) -> ApiError {
    let (code, message) = match status {
        StatusCode::BAD_REQUEST => ("bad_request", "The request could not be understood."),
        StatusCode::UNAUTHORIZED => ("unauthenticated", "Sign in to continue."),
        StatusCode::FORBIDDEN => ("forbidden", "This request is not permitted."),
        StatusCode::NOT_FOUND => ("not_found", "Nothing is available at this address."),
        StatusCode::METHOD_NOT_ALLOWED => (
            "method_not_allowed",
            "This address does not support that method.",
        ),
        StatusCode::REQUEST_TIMEOUT => ("timeout", "The request took too long to arrive."),
        StatusCode::PAYLOAD_TOO_LARGE => ("payload_too_large", "The request body is too large."),
        StatusCode::UNSUPPORTED_MEDIA_TYPE => (
            "unsupported_media_type",
            "The request body is not in a supported format.",
        ),
        StatusCode::UNPROCESSABLE_ENTITY => (
            "validation",
            "The request body is not valid for this action.",
        ),
        StatusCode::TOO_MANY_REQUESTS => (
            "rate_limited",
            "Too many requests. Wait a moment and try again.",
        ),
        StatusCode::SERVICE_UNAVAILABLE => (
            "unavailable",
            "The service is temporarily unavailable. Try again shortly.",
        ),
        _ if status.is_client_error() => ("request_error", "The request could not be completed."),
        _ => ("server_error", "The request could not be completed."),
    };
    ApiError {
        status,
        code,
        message: message.into(),
    }
}

fn is_json(response: &HttpResponse<impl MessageBody>) -> bool {
    response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
        })
}

/// Replace an error response that is not already JSON with the envelope for its
/// status, keeping its status and every header except those describing the old body.
fn enveloped<B: MessageBody + 'static>(
    request: HttpRequest,
    response: HttpResponse<B>,
) -> ServiceResponse<BoxBody> {
    let status = response.status();
    let failed = status.is_client_error() || status.is_server_error();
    if !failed || is_json(&response) {
        return ServiceResponse::new(request, response.map_into_boxed_body());
    }
    let error = envelope_for(status);
    let mut replacement = HttpResponse::build(status).json(serde_json::json!({
        "error": {"code": error.code, "message": error.message}
    }));
    for (name, value) in response.headers() {
        if !matches!(
            *name,
            header::CONTENT_TYPE | header::CONTENT_LENGTH | header::CONTENT_ENCODING
        ) {
            replacement
                .headers_mut()
                .append(name.clone(), value.clone());
        }
    }
    ServiceResponse::new(request, replacement)
}

/// Wrap the whole application with this, outermost, so it sees every request.
pub async fn track<B: MessageBody + 'static>(
    req: ServiceRequest,
    next: Next<B>,
) -> Result<ServiceResponse<BoxBody>, actix_web::Error> {
    let id = Uuid::new_v4().simple().to_string();
    let method = req.method().clone();
    let path = logged_path(req.path());
    req.extensions_mut().insert(RequestId(id.clone()));
    let started = Instant::now();
    let outcome = tokio::time::timeout(REQUEST_DEADLINE, next.call(req)).await;
    let elapsed = started.elapsed().as_millis();
    let line = |status: StatusCode| {
        // Probes arrive every few seconds; keep them out of the default log.
        let level = if matches!(path.as_str(), "/livez" | "/healthz") && status.is_success() {
            log::Level::Debug
        } else {
            log::Level::Info
        };
        log::log!(
            target: "nexus::access",
            level,
            "request_id={id} method={method} path={path} status={} duration_ms={elapsed}",
            status.as_u16()
        );
    };
    match outcome {
        Ok(Ok(response)) => {
            let (request, response) = response.into_parts();
            let mut response = enveloped(request, response);
            if let Ok(value) = HeaderValue::from_str(&id) {
                response.headers_mut().insert(REQUEST_ID_HEADER, value);
            }
            line(response.status());
            Ok(response)
        }
        // An error raised by a wrapper rather than a handler. Its own text is not sent.
        Ok(Err(error)) => {
            let status = error.as_response_error().status_code();
            line(status);
            Err(envelope_for(status).into())
        }
        Err(_deadline) => {
            line(StatusCode::SERVICE_UNAVAILABLE);
            Err(ApiError {
                status: StatusCode::SERVICE_UNAVAILABLE,
                code: "timeout",
                message: "The request took too long and was stopped.".into(),
            }
            .into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{middleware::from_fn, test as http_test, web, App};

    #[test]
    fn logged_paths_hide_identifiers_tokens_and_odd_characters() {
        assert_eq!(logged_path("/api/bootstrap"), "/api/bootstrap");
        assert_eq!(
            logged_path("/api/clients/4b0e1c5e-6f0a-4d0c-9a53-0d6f0e6f2a11/form-requirements"),
            "/api/clients/{id}/form-requirements"
        );
        assert_eq!(
            logged_path("/api/recording-tracks/4b0e1c5e6f0a4d0c9a530d6f0e6f2a11/chunks/12"),
            "/api/recording-tracks/{id}/chunks/12"
        );
        // A token pasted into a path by mistake is not written out.
        let token = "a".repeat(64);
        assert_eq!(logged_path(&format!("/api/{token}")), "/api/{id}");
        // Percent-encoded line breaks stay encoded; raw oddities are masked.
        assert_eq!(
            logged_path("/api/x%0Aforged status=200"),
            "/api/x%0Aforged?status?200"
        );
        assert!(logged_path(&"/segment".repeat(100)).chars().count() <= 201);
    }

    #[test]
    fn generic_envelopes_never_describe_the_server() {
        for status in [
            StatusCode::BAD_REQUEST,
            StatusCode::NOT_FOUND,
            StatusCode::METHOD_NOT_ALLOWED,
            StatusCode::PAYLOAD_TOO_LARGE,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            StatusCode::IM_A_TEAPOT,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::BAD_GATEWAY,
        ] {
            let error = envelope_for(status);
            assert_eq!(error.status, status);
            let text = error.message.to_lowercase();
            for word in ["actix", "serde", "json", "sql", "rust", "panic"] {
                assert!(!text.contains(word), "{status}: {text}");
            }
        }
        assert_eq!(envelope_for(StatusCode::IM_A_TEAPOT).code, "request_error");
        assert_eq!(envelope_for(StatusCode::BAD_GATEWAY).code, "server_error");
    }

    #[actix_web::test]
    async fn every_response_carries_a_request_id_and_errors_use_the_envelope() {
        let app = http_test::init_service(
            App::new()
                .wrap(from_fn(track))
                .route("/ok", web::get().to(|| async { "fine" }))
                .route(
                    "/plain-error",
                    web::get().to(|| async {
                        HttpResponse::PayloadTooLarge()
                            .insert_header(("x-kept", "yes"))
                            .body("payload reached size limit at src/internal.rs")
                    }),
                )
                .route(
                    "/own-error",
                    web::get().to(|| async { Err::<HttpResponse, _>(ApiError::denied()) }),
                ),
        )
        .await;

        let ok =
            http_test::call_service(&app, http_test::TestRequest::get().uri("/ok").to_request())
                .await;
        assert_eq!(ok.status(), StatusCode::OK);
        let first = ok.headers().get(&REQUEST_ID_HEADER).unwrap().clone();
        assert_eq!(first.len(), 32);
        assert_eq!(http_test::read_body(ok).await, "fine");

        // A client-supplied id is not adopted, and each request gets its own.
        let request = http_test::TestRequest::get()
            .uri("/ok")
            .insert_header(("x-request-id", "chosen-by-client"))
            .to_request();
        let second = http_test::call_service(&app, request).await;
        let second = second.headers().get(&REQUEST_ID_HEADER).unwrap().clone();
        assert_ne!(second, "chosen-by-client");
        assert_ne!(second, first);

        let plain = http_test::call_service(
            &app,
            http_test::TestRequest::get()
                .uri("/plain-error")
                .to_request(),
        )
        .await;
        assert_eq!(plain.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(plain.headers().get("x-kept").unwrap(), "yes");
        assert!(plain.headers().contains_key(&REQUEST_ID_HEADER));
        let body: serde_json::Value = http_test::read_body_json(plain).await;
        assert_eq!(body["error"]["code"], "payload_too_large");
        assert!(!body.to_string().contains("internal.rs"));

        // The application's own errors pass through untouched.
        let own = http_test::call_service(
            &app,
            http_test::TestRequest::get().uri("/own-error").to_request(),
        )
        .await;
        assert_eq!(own.status(), StatusCode::FORBIDDEN);
        let body: serde_json::Value = http_test::read_body_json(own).await;
        assert_eq!(body["error"]["code"], "forbidden");
        assert_eq!(
            body["error"]["message"],
            "Your selected role does not permit this action."
        );

        // No route at all: actix's empty 404 becomes the envelope.
        let missing = http_test::call_service(
            &app,
            http_test::TestRequest::get().uri("/nowhere").to_request(),
        )
        .await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        let body: serde_json::Value = http_test::read_body_json(missing).await;
        assert_eq!(body["error"]["code"], "not_found");
    }
}
