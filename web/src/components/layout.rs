//! Layout components

use leptos::*;

/// Container component
#[component]
pub fn Container(
    children: Children,
) -> impl IntoView {
    view! {
        <div class="container mx-auto px-4">
            {children()}
        </div>
    }
} 