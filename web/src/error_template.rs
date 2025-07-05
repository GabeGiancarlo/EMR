//! Error template and error handling

use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use thiserror::Error;

/// Application error types
#[derive(Clone, Debug, Error)]
pub enum AppError {
    #[error("Not Found")]
    NotFound,
    #[error("Internal Server Error")]
    InternalServerError,
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Forbidden")]
    Forbidden,
    #[error("Bad Request: {0}")]
    BadRequest(String),
    #[error("Service Unavailable")]
    ServiceUnavailable,
}

impl AppError {
    /// Get the HTTP status code for this error
    pub fn status_code(&self) -> u16 {
        match self {
            AppError::NotFound => 404,
            AppError::InternalServerError => 500,
            AppError::Unauthorized => 401,
            AppError::Forbidden => 403,
            AppError::BadRequest(_) => 400,
            AppError::ServiceUnavailable => 503,
        }
    }

    /// Get the error title for display
    pub fn title(&self) -> &'static str {
        match self {
            AppError::NotFound => "Page Not Found",
            AppError::InternalServerError => "Internal Server Error",
            AppError::Unauthorized => "Unauthorized",
            AppError::Forbidden => "Forbidden",
            AppError::BadRequest(_) => "Bad Request",
            AppError::ServiceUnavailable => "Service Unavailable",
        }
    }

    /// Get the error description for display
    pub fn description(&self) -> String {
        match self {
            AppError::NotFound => "The page you are looking for could not be found.".to_string(),
            AppError::InternalServerError => "An internal server error occurred.".to_string(),
            AppError::Unauthorized => "You are not authorized to access this resource.".to_string(),
            AppError::Forbidden => "Access to this resource is forbidden.".to_string(),
            AppError::BadRequest(msg) => format!("Bad request: {}", msg),
            AppError::ServiceUnavailable => "The service is temporarily unavailable.".to_string(),
        }
    }
}

/// Error template component
#[component]
pub fn ErrorTemplate(
    #[prop(optional)] outside_errors: Option<Errors>,
    #[prop(optional)] errors: Option<RwSignal<Errors>>,
) -> impl IntoView {
    let errors = match outside_errors {
        Some(e) => create_rw_signal(e),
        None => match errors {
            Some(e) => e,
            None => panic!("No Errors found and we expected errors!"),
        },
    };

    // Get the first error
    let error = move || {
        errors.with(|errors| {
            errors
                .iter()
                .next()
                .map(|(_, error)| error.clone())
                .unwrap_or_else(|| AppError::InternalServerError)
        })
    };

    let retry_action = create_action(move |_: &()| {
        // Clear the errors
        errors.update(|errors| errors.clear());
        // Reload the page
        if let Some(window) = web_sys::window() {
            let _ = window.location().reload();
        }
        async {}
    });

    view! {
        <Html lang="en"/>
        <Title text=move || format!("Error - {}", error().title())/>
        <Meta name="description" content=move || error().description()/>
        
        <div class="min-h-screen bg-gray-50 flex items-center justify-center px-4">
            <div class="max-w-md w-full bg-white rounded-lg shadow-lg p-8 text-center">
                <div class="mb-6">
                    <div class="text-6xl text-red-500 mb-4">
                        {move || match error().status_code() {
                            404 => "🔍",
                            500 => "⚠️",
                            401 => "🔐",
                            403 => "🚫",
                            400 => "❌",
                            503 => "🔧",
                            _ => "❓",
                        }}
                    </div>
                    <h1 class="text-3xl font-bold text-gray-800 mb-2">
                        {move || error().status_code().to_string()}
                    </h1>
                    <h2 class="text-xl font-semibold text-gray-700 mb-4">
                        {move || error().title()}
                    </h2>
                    <p class="text-gray-600 mb-6">
                        {move || error().description()}
                    </p>
                </div>
                
                <div class="flex flex-col sm:flex-row gap-4 justify-center">
                    <A 
                        href="/"
                        class="bg-blue-600 text-white px-6 py-3 rounded-lg hover:bg-blue-700 transition-colors font-medium"
                    >
                        "Go Home"
                    </A>
                    
                    <Show when=move || error().status_code() >= 500>
                        <button
                            on:click=move |_| retry_action.dispatch(())
                            class="bg-gray-600 text-white px-6 py-3 rounded-lg hover:bg-gray-700 transition-colors font-medium"
                        >
                            "Try Again"
                        </button>
                    </Show>
                </div>
                
                <Show when=move || error().status_code() == 404>
                    <div class="mt-8 text-sm text-gray-500">
                        <p>"If you believe this is an error, please contact support."</p>
                    </div>
                </Show>
            </div>
        </div>
    }
}

/// Error boundary component
#[component]
pub fn ErrorBoundary(
    /// The fallback component to render when an error occurs
    fallback: F,
    /// The children to render when no error occurs
    children: Children,
) -> impl IntoView
where
    F: Fn(RwSignal<Errors>) -> View + 'static,
{
    let errors = create_rw_signal(Errors::default());
    
    view! {
        <leptos::ErrorBoundary fallback=fallback>
            {children()}
        </leptos::ErrorBoundary>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_status_codes() {
        assert_eq!(AppError::NotFound.status_code(), 404);
        assert_eq!(AppError::InternalServerError.status_code(), 500);
        assert_eq!(AppError::Unauthorized.status_code(), 401);
        assert_eq!(AppError::Forbidden.status_code(), 403);
        assert_eq!(AppError::BadRequest("test".to_string()).status_code(), 400);
        assert_eq!(AppError::ServiceUnavailable.status_code(), 503);
    }

    #[test]
    fn test_app_error_titles() {
        assert_eq!(AppError::NotFound.title(), "Page Not Found");
        assert_eq!(AppError::InternalServerError.title(), "Internal Server Error");
        assert_eq!(AppError::Unauthorized.title(), "Unauthorized");
        assert_eq!(AppError::Forbidden.title(), "Forbidden");
        assert_eq!(AppError::BadRequest("test".to_string()).title(), "Bad Request");
        assert_eq!(AppError::ServiceUnavailable.title(), "Service Unavailable");
    }

    #[test]
    fn test_app_error_descriptions() {
        assert!(AppError::NotFound.description().contains("not found"));
        assert!(AppError::InternalServerError.description().contains("internal server error"));
        assert!(AppError::Unauthorized.description().contains("not authorized"));
        assert!(AppError::Forbidden.description().contains("forbidden"));
        assert!(AppError::BadRequest("test".to_string()).description().contains("test"));
        assert!(AppError::ServiceUnavailable.description().contains("temporarily unavailable"));
    }
} 