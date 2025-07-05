//! Main application component

use leptos::*;
use leptos_meta::*;
use leptos_router::*;

use crate::{
    error_template::{AppError, ErrorTemplate},
    pages::{home::HomePage, patients::PatientsPage},
};

/// Main application component
#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        <Html lang="en"/>
        <Title text="EMR Platform"/>
        <Meta name="description" content="HIPAA-compliant Electronic Medical Record Platform"/>
        <Meta name="viewport" content="width=device-width, initial-scale=1.0"/>
        <Link rel="stylesheet" href="/assets/style.css"/>
        <Link rel="icon" href="/assets/favicon.ico"/>

        <Router fallback=|| {
            let mut outside_errors = Errors::default();
            outside_errors.insert_with_default_key(AppError::NotFound);
            view! {
                <ErrorTemplate outside_errors/>
            }
            .into_view()
        }>
            <main class="min-h-screen bg-gray-50">
                <AppHeader/>
                <div class="container mx-auto px-4 py-8">
                    <Routes>
                        <Route path="" view=HomePage/>
                        <Route path="/patients" view=PatientsPage/>
                        <Route path="/patients/:id" view=PatientDetailPage/>
                    </Routes>
                </div>
            </main>
        </Router>
    }
}

/// Application header component
#[component]
fn AppHeader() -> impl IntoView {
    view! {
        <header class="bg-blue-600 text-white shadow-lg">
            <div class="container mx-auto px-4 py-4">
                <div class="flex items-center justify-between">
                    <div class="flex items-center space-x-4">
                        <h1 class="text-2xl font-bold">
                            <A href="/" class="text-white hover:text-blue-200">
                                "EMR Platform"
                            </A>
                        </h1>
                    </div>
                    <nav class="flex space-x-4">
                        <A href="/" class="hover:text-blue-200 px-3 py-2 rounded transition-colors">
                            "Home"
                        </A>
                        <A href="/patients" class="hover:text-blue-200 px-3 py-2 rounded transition-colors">
                            "Patients"
                        </A>
                    </nav>
                </div>
            </div>
        </header>
    }
}

/// Patient detail page component
#[component]
fn PatientDetailPage() -> impl IntoView {
    let params = use_params_map();
    let patient_id = move || params.with(|p| p.get("id").cloned().unwrap_or_default());

    let (patient, set_patient) = create_signal(None::<serde_json::Value>);

    // Fetch patient data
    create_effect(move |_| {
        let id = patient_id();
        if !id.is_empty() {
            spawn_local(async move {
                // TODO: Replace with actual API call
                let patient_data = serde_json::json!({
                    "id": id,
                    "name": "John Doe",
                    "birthDate": "1980-01-01",
                    "gender": "male",
                    "phone": "+1-555-123-4567",
                    "email": "john.doe@example.com"
                });
                set_patient.set(Some(patient_data));
            });
        }
    });

    view! {
        <div class="max-w-4xl mx-auto">
            <div class="bg-white rounded-lg shadow-lg p-6">
                <h2 class="text-2xl font-bold mb-6 text-gray-800">
                    "Patient Details"
                </h2>
                
                <Suspense fallback=move || view! { <div class="text-center py-8">"Loading patient data..."</div> }>
                    {move || match patient.get() {
                        Some(patient_data) => {
                            let name = patient_data.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
                            let birth_date = patient_data.get("birthDate").and_then(|v| v.as_str()).unwrap_or("Unknown");
                            let gender = patient_data.get("gender").and_then(|v| v.as_str()).unwrap_or("Unknown");
                            let phone = patient_data.get("phone").and_then(|v| v.as_str()).unwrap_or("Unknown");
                            let email = patient_data.get("email").and_then(|v| v.as_str()).unwrap_or("Unknown");
                            
                            view! {
                                <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                                    <div class="space-y-4">
                                        <div>
                                            <label class="block text-sm font-medium text-gray-700 mb-1">
                                                "Name"
                                            </label>
                                            <div class="text-lg text-gray-900">{name}</div>
                                        </div>
                                        <div>
                                            <label class="block text-sm font-medium text-gray-700 mb-1">
                                                "Birth Date"
                                            </label>
                                            <div class="text-lg text-gray-900">{birth_date}</div>
                                        </div>
                                        <div>
                                            <label class="block text-sm font-medium text-gray-700 mb-1">
                                                "Gender"
                                            </label>
                                            <div class="text-lg text-gray-900 capitalize">{gender}</div>
                                        </div>
                                    </div>
                                    <div class="space-y-4">
                                        <div>
                                            <label class="block text-sm font-medium text-gray-700 mb-1">
                                                "Phone"
                                            </label>
                                            <div class="text-lg text-gray-900">{phone}</div>
                                        </div>
                                        <div>
                                            <label class="block text-sm font-medium text-gray-700 mb-1">
                                                "Email"
                                            </label>
                                            <div class="text-lg text-gray-900">{email}</div>
                                        </div>
                                    </div>
                                </div>
                                
                                <div class="mt-8 flex space-x-4">
                                    <button class="bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700 transition-colors">
                                        "Edit Patient"
                                    </button>
                                    <A href="/patients" class="bg-gray-600 text-white px-4 py-2 rounded hover:bg-gray-700 transition-colors">
                                        "Back to Patients"
                                    </A>
                                </div>
                            }.into_view()
                        },
                        None => view! {
                            <div class="text-center py-8 text-gray-500">
                                "Patient not found"
                            </div>
                        }.into_view()
                    }}
                </Suspense>
            </div>
        </div>
    }
} 