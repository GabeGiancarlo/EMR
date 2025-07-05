//! Form components

use leptos::*;

/// Input field component
#[component]
pub fn InputField(
    #[prop(optional)] label: Option<String>,
    #[prop(optional)] placeholder: Option<String>,
    #[prop(optional)] value: Option<String>,
) -> impl IntoView {
    view! {
        <div class="mb-4">
            {label.map(|l| view! {
                <label class="block text-sm font-medium text-gray-700 mb-1">{l}</label>
            })}
            <input
                type="text"
                placeholder=placeholder.unwrap_or_default()
                value=value.unwrap_or_default()
                class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
        </div>
    }
} 