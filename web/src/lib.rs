use leptos::*;
use leptos_meta::*;
use leptos_router::*;

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    
    view! {
        <Stylesheet id="leptos" href="/pkg/emr-web.css"/>
        <Html lang="en" dir="ltr" attr:data-theme="light"/>
        <Title text="EMR Platform"/>
        <Meta charset="utf-8"/>
        <Meta name="viewport" content="width=device-width, initial-scale=1"/>
        
        <Router>
            <div class="min-h-screen bg-gray-50">
                <nav class="bg-white shadow-lg">
                    <div class="max-w-7xl mx-auto px-4">
                        <div class="flex justify-between h-16">
                            <div class="flex items-center">
                                <span class="text-xl font-bold text-gray-900">
                                    "🏥 EMR Platform"
                                </span>
                            </div>
                            <div class="flex items-center space-x-4">
                                <a href="/" class="text-gray-700 hover:text-gray-900">"Home"</a>
                                <a href="/patients" class="text-gray-700 hover:text-gray-900">"Patients"</a>
                            </div>
                        </div>
                    </div>
                </nav>
                
                <main>
                    <Routes>
                        <Route path="/" view=HomePage/>
                        <Route path="/patients" view=PatientsPage/>
                        <Route path="/patients/:id" view=PatientDetail/>
                    </Routes>
                </main>
            </div>
        </Router>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    view! {
        <div class="py-12">
            <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
                <div class="text-center">
                    <h1 class="text-4xl font-bold text-gray-900 mb-8">
                        "🏥 EMR Platform"
                    </h1>
                    <p class="text-xl text-gray-600 mb-12">
                        "HIPAA-compliant Electronic Medical Record System"
                    </p>
                    
                    <div class="grid grid-cols-1 md:grid-cols-3 gap-6 max-w-4xl mx-auto">
                        <div class="bg-white rounded-lg shadow-lg p-6">
                            <h2 class="text-xl font-semibold text-gray-900 mb-4">
                                "👥 Patient Management"
                            </h2>
                            <p class="text-gray-600 mb-4">
                                "Manage patient records, demographics, and medical history"
                            </p>
                            <a href="/patients" class="inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md text-white bg-blue-600 hover:bg-blue-700">
                                "View Patients"
                            </a>
                        </div>
                        
                        <div class="bg-white rounded-lg shadow-lg p-6">
                            <h2 class="text-xl font-semibold text-gray-900 mb-4">
                                "📊 FHIR Integration"
                            </h2>
                            <p class="text-gray-600 mb-4">
                                "Full FHIR R4 compliance for interoperability"
                            </p>
                            <button class="inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md text-white bg-green-600 hover:bg-green-700">
                                "FHIR Resources"
                            </button>
                        </div>
                        
                        <div class="bg-white rounded-lg shadow-lg p-6">
                            <h2 class="text-xl font-semibold text-gray-900 mb-4">
                                "🔐 Security & Compliance"
                            </h2>
                            <p class="text-gray-600 mb-4">
                                "HIPAA-grade security with audit trails"
                            </p>
                            <button class="inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md text-white bg-purple-600 hover:bg-purple-700">
                                "Security Settings"
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn PatientsPage() -> impl IntoView {
    let (patients, set_patients) = create_signal(Vec::<Patient>::new());
    let (loading, set_loading) = create_signal(true);
    
    // Mock data for development
    create_effect(move |_| {
        let mock_patients = vec![
            Patient {
                id: "patient-001".to_string(),
                name: "John Doe".to_string(),
                email: "john.doe@example.com".to_string(),
                phone: "+1-555-0123".to_string(),
                birth_date: "1985-06-15".to_string(),
                status: "Active".to_string(),
            },
            Patient {
                id: "patient-002".to_string(),
                name: "Jane Smith".to_string(),
                email: "jane.smith@example.com".to_string(),
                phone: "+1-555-0456".to_string(),
                birth_date: "1990-03-22".to_string(),
                status: "Active".to_string(),
            },
            Patient {
                id: "patient-003".to_string(),
                name: "Bob Johnson".to_string(),
                email: "bob.johnson@example.com".to_string(),
                phone: "+1-555-0789".to_string(),
                birth_date: "1978-11-03".to_string(),
                status: "Active".to_string(),
            },
        ];
        
        set_patients(mock_patients);
        set_loading(false);
    });
    
    view! {
        <div class="py-12">
            <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
                <div class="sm:flex sm:items-center">
                    <div class="sm:flex-auto">
                        <h1 class="text-2xl font-semibold text-gray-900">"Patients"</h1>
                        <p class="mt-2 text-sm text-gray-700">
                            "Manage patient records and medical information"
                        </p>
                    </div>
                    <div class="mt-4 sm:mt-0 sm:ml-16 sm:flex-none">
                        <button class="inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md text-white bg-blue-600 hover:bg-blue-700">
                            "Add Patient"
                        </button>
                    </div>
                </div>
                
                <div class="mt-8">
                    {move || {
                        if loading() {
                            view! {
                                <div class="text-center py-12">
                                    <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600 mx-auto"></div>
                                    <p class="mt-4 text-gray-600">"Loading patients..."</p>
                                </div>
                            }
                        } else {
                            view! {
                                <div class="bg-white shadow overflow-hidden sm:rounded-lg">
                                    <table class="min-w-full divide-y divide-gray-200">
                                        <thead class="bg-gray-50">
                                            <tr>
                                                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">"Name"</th>
                                                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">"Email"</th>
                                                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">"Phone"</th>
                                                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">"Status"</th>
                                                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">"Actions"</th>
                                            </tr>
                                        </thead>
                                        <tbody class="bg-white divide-y divide-gray-200">
                                            <For
                                                each=move || patients()
                                                key=|patient| patient.id.clone()
                                                children=move |patient| {
                                                    view! {
                                                        <tr>
                                                            <td class="px-6 py-4 whitespace-nowrap text-sm font-medium text-gray-900">
                                                                {patient.name}
                                                            </td>
                                                            <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                                                                {patient.email}
                                                            </td>
                                                            <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                                                                {patient.phone}
                                                            </td>
                                                            <td class="px-6 py-4 whitespace-nowrap">
                                                                <span class="inline-flex px-2 py-1 text-xs font-semibold rounded-full bg-green-100 text-green-800">
                                                                    {patient.status}
                                                                </span>
                                                            </td>
                                                            <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                                                                <a href={format!("/patients/{}", patient.id)} class="text-blue-600 hover:text-blue-900">
                                                                    "View"
                                                                </a>
                                                            </td>
                                                        </tr>
                                                    }
                                                }
                                            />
                                        </tbody>
                                    </table>
                                </div>
                            }
                        }
                    }}
                </div>
            </div>
        </div>
    }
}

#[component]
fn PatientDetail() -> impl IntoView {
    let params = use_params_map();
    let patient_id = move || params.with(|p| p.get("id").cloned().unwrap_or_default());
    
    view! {
        <div class="py-12">
            <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
                <div class="bg-white shadow overflow-hidden sm:rounded-lg">
                    <div class="px-4 py-5 sm:px-6">
                        <h3 class="text-lg leading-6 font-medium text-gray-900">
                            "Patient Information"
                        </h3>
                        <p class="mt-1 max-w-2xl text-sm text-gray-500">
                            "ID: " {patient_id}
                        </p>
                    </div>
                    <div class="border-t border-gray-200 px-4 py-5 sm:p-0">
                        <dl class="sm:divide-y sm:divide-gray-200">
                            <div class="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                                <dt class="text-sm font-medium text-gray-500">"Full name"</dt>
                                <dd class="mt-1 text-sm text-gray-900 sm:mt-0 sm:col-span-2">"John Doe"</dd>
                            </div>
                            <div class="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                                <dt class="text-sm font-medium text-gray-500">"Email address"</dt>
                                <dd class="mt-1 text-sm text-gray-900 sm:mt-0 sm:col-span-2">"john.doe@example.com"</dd>
                            </div>
                            <div class="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                                <dt class="text-sm font-medium text-gray-500">"Phone number"</dt>
                                <dd class="mt-1 text-sm text-gray-900 sm:mt-0 sm:col-span-2">"+1-555-0123"</dd>
                            </div>
                            <div class="py-4 sm:py-5 sm:grid sm:grid-cols-3 sm:gap-4 sm:px-6">
                                <dt class="text-sm font-medium text-gray-500">"Date of birth"</dt>
                                <dd class="mt-1 text-sm text-gray-900 sm:mt-0 sm:col-span-2">"1985-06-15"</dd>
                            </div>
                        </dl>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Patient {
    id: String,
    name: String,
    email: String,
    phone: String,
    birth_date: String,
    status: String,
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount_to_body(App);
} 