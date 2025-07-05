//! Security middleware

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
};
use futures_util::future::{ready, Ready};
use std::{
    future::{ready as fut_ready, Future, Ready as FutReady},
    pin::Pin,
    task::{Context, Poll},
};

/// Security headers middleware
pub struct SecurityHeaders;

impl<S, B> Transform<S, ServiceRequest> for SecurityHeaders
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = SecurityHeadersMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(SecurityHeadersMiddleware { service }))
    }
}

pub struct SecurityHeadersMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for SecurityHeadersMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let fut = self.service.call(req);

        Box::pin(async move {
            let mut res = fut.await?;

            // Add security headers
            res.headers_mut().insert(
                actix_web::http::header::HeaderName::from_static("x-content-type-options"),
                actix_web::http::HeaderValue::from_static("nosniff"),
            );
            res.headers_mut().insert(
                actix_web::http::header::HeaderName::from_static("x-frame-options"),
                actix_web::http::HeaderValue::from_static("DENY"),
            );
            res.headers_mut().insert(
                actix_web::http::header::HeaderName::from_static("x-xss-protection"),
                actix_web::http::HeaderValue::from_static("1; mode=block"),
            );
            res.headers_mut().insert(
                actix_web::http::header::HeaderName::from_static("strict-transport-security"),
                actix_web::http::HeaderValue::from_static("max-age=31536000; includeSubDomains"),
            );

            Ok(res)
        })
    }
} 