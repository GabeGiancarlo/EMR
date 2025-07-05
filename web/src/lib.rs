#![deny(unsafe_code)]

//! EMR Web Application
//!
//! This crate provides the frontend web application for the EMR platform
//! using Leptos for both server-side rendering and client-side hydration.

use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use wasm_bindgen::prelude::wasm_bindgen;

pub mod app;
pub mod components;
pub mod config;
pub mod error_template;
pub mod pages;
pub mod state;
pub mod utils;

#[cfg(feature = "hydrate")]
#[wasm_bindgen]
pub fn hydrate() {
    use app::App;
    use tracing::info;
    
    // Initialize console error panic hook for better debugging
    console_error_panic_hook::set_once();
    
    info!("Starting client-side hydration");
    
    leptos::mount_to_body(App);
}

/// Re-export commonly used types
pub mod prelude {
    pub use crate::{
        app::App,
        components::*,
        config::WebConfig,
        error_template::{AppError, ErrorTemplate},
        pages::*,
        state::AppState,
        utils::*,
    };
    pub use leptos::*;
    pub use leptos_meta::*;
    pub use leptos_router::*;
} 