//! Common UI components

use leptos::*;

/// Button component
#[component]
pub fn Button(
    #[prop(optional)] class: Option<String>,
    #[prop(optional)] onclick: Option<Box<dyn Fn()>>,
    children: Children,
) -> impl IntoView {
    let default_class = "bg-blue-600 text-white px-4 py-2 rounded-lg hover:bg-blue-700 transition-colors font-medium";
    let button_class = class.unwrap_or_else(|| default_class.to_string());

    view! {
        <button 
            class=button_class
            on:click=move |_| {
                if let Some(ref handler) = onclick {
                    handler();
                }
            }
        >
            {children()}
        </button>
    }
}

/// Loading spinner component
#[component]
pub fn LoadingSpinner(
    #[prop(optional)] size: Option<String>,
) -> impl IntoView {
    let size_class = size.unwrap_or_else(|| "w-8 h-8".to_string());

    view! {
        <div class="flex justify-center items-center">
            <div class=format!("{} animate-spin rounded-full border-4 border-gray-300 border-t-blue-600", size_class)></div>
        </div>
    }
}

/// Card component
#[component]
pub fn Card(
    #[prop(optional)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let default_class = "bg-white rounded-lg shadow-lg p-6";
    let card_class = class.unwrap_or_else(|| default_class.to_string());

    view! {
        <div class=card_class>
            {children()}
        </div>
    }
} 