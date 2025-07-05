//! Patient-specific components

use leptos::*;

/// Patient card component
#[component]
pub fn PatientCard(
    name: String,
    id: String,
) -> impl IntoView {
    view! {
        <div class="bg-white rounded-lg shadow p-4">
            <h3 class="text-lg font-semibold">{name}</h3>
            <p class="text-gray-600 text-sm">{"ID: "}{id}</p>
        </div>
    }
} 