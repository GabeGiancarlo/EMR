//! Patients page component

use leptos::*;
use leptos_router::*;

/// Patients page component
#[component]
pub fn PatientsPage() -> impl IntoView {
    let (patients, set_patients) = create_signal(Vec::<serde_json::Value>::new());
    let (search_term, set_search_term) = create_signal(String::new());
    let (loading, set_loading) = create_signal(true);

    // Fetch patients on component mount
    create_effect(move |_| {
        spawn_local(async move {
            set_loading.set(true);
            
            // TODO: Replace with actual API call
            let patient_data = vec![
                serde_json::json!({
                    "id": "patient-1",
                    "name": "John Doe",
                    "birthDate": "1980-01-01",
                    "gender": "male",
                    "phone": "+1-555-123-4567",
                    "email": "john.doe@example.com"
                }),
                serde_json::json!({
                    "id": "patient-2",
                    "name": "Jane Smith",
                    "birthDate": "1985-05-15",
                    "gender": "female",
                    "phone": "+1-555-987-6543",
                    "email": "jane.smith@example.com"
                }),
                serde_json::json!({
                    "id": "patient-3",
                    "name": "Robert Johnson",
                    "birthDate": "1975-12-03",
                    "gender": "male",
                    "phone": "+1-555-456-7890",
                    "email": "robert.johnson@example.com"
                }),
            ];
            
            set_patients.set(patient_data);
            set_loading.set(false);
        });
    });

    // Filter patients based on search term
    let filtered_patients = move || {
        let term = search_term.get().to_lowercase();
        if term.is_empty() {
            patients.get()
        } else {
            patients.get().into_iter().filter(|patient| {
                let name = patient.get("name").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
                let email = patient.get("email").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
                name.contains(&term) || email.contains(&term)
            }).collect()
        }
    };

    view! {
        <div class="max-w-6xl mx-auto">
            <div class="flex items-center justify-between mb-8">
                <h1 class="text-3xl font-bold text-gray-800">
                    "Patients"
                </h1>
                <button class="bg-blue-600 text-white px-4 py-2 rounded-lg hover:bg-blue-700 transition-colors font-medium">
                    "Add New Patient"
                </button>
            </div>

            <div class="bg-white rounded-lg shadow-lg p-6 mb-6">
                <div class="flex items-center space-x-4 mb-4">
                    <div class="flex-1">
                        <input
                            type="text"
                            placeholder="Search patients by name or email..."
                            class="w-full px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                            on:input=move |ev| {
                                set_search_term.set(event_target_value(&ev));
                            }
                            prop:value=search_term
                        />
                    </div>
                    <button class="bg-gray-600 text-white px-4 py-2 rounded-lg hover:bg-gray-700 transition-colors">
                        "Filter"
                    </button>
                </div>
                
                <div class="text-sm text-gray-600">
                    {move || {
                        let count = filtered_patients().len();
                        format!("Showing {} patient{}", count, if count == 1 { "" } else { "s" })
                    }}
                </div>
            </div>

            <div class="bg-white rounded-lg shadow-lg overflow-hidden">
                <Suspense fallback=move || view! { 
                    <div class="p-8 text-center text-gray-500">
                        "Loading patients..."
                    </div>
                }>
                    {move || {
                        if loading.get() {
                            view! {
                                <div class="p-8 text-center text-gray-500">
                                    "Loading patients..."
                                </div>
                            }.into_view()
                        } else {
                            let patients = filtered_patients();
                            if patients.is_empty() {
                                view! {
                                    <div class="p-8 text-center text-gray-500">
                                        "No patients found"
                                    </div>
                                }.into_view()
                            } else {
                                view! {
                                    <div class="overflow-x-auto">
                                        <table class="w-full">
                                            <thead class="bg-gray-50">
                                                <tr>
                                                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                                                        "Name"
                                                    </th>
                                                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                                                        "Birth Date"
                                                    </th>
                                                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                                                        "Gender"
                                                    </th>
                                                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                                                        "Phone"
                                                    </th>
                                                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                                                        "Email"
                                                    </th>
                                                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                                                        "Actions"
                                                    </th>
                                                </tr>
                                            </thead>
                                            <tbody class="bg-white divide-y divide-gray-200">
                                                <For
                                                    each=move || filtered_patients()
                                                    key=|patient| patient.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string()
                                                    children=move |patient| {
                                                        let id = patient.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                        let name = patient.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
                                                        let birth_date = patient.get("birthDate").and_then(|v| v.as_str()).unwrap_or("Unknown");
                                                        let gender = patient.get("gender").and_then(|v| v.as_str()).unwrap_or("Unknown");
                                                        let phone = patient.get("phone").and_then(|v| v.as_str()).unwrap_or("Unknown");
                                                        let email = patient.get("email").and_then(|v| v.as_str()).unwrap_or("Unknown");
                                                        
                                                        view! {
                                                            <PatientRow
                                                                id=id
                                                                name=name.to_string()
                                                                birth_date=birth_date.to_string()
                                                                gender=gender.to_string()
                                                                phone=phone.to_string()
                                                                email=email.to_string()
                                                            />
                                                        }
                                                    }
                                                />
                                            </tbody>
                                        </table>
                                    </div>
                                }.into_view()
                            }
                        }
                    }}
                </Suspense>
            </div>
        </div>
    }
}

/// Patient row component
#[component]
fn PatientRow(
    id: String,
    name: String,
    birth_date: String,
    gender: String,
    phone: String,
    email: String,
) -> impl IntoView {
    view! {
        <tr class="hover:bg-gray-50">
            <td class="px-6 py-4 whitespace-nowrap">
                <div class="font-medium text-gray-900">{name}</div>
            </td>
            <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                {birth_date}
            </td>
            <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500 capitalize">
                {gender}
            </td>
            <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                {phone}
            </td>
            <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                {email}
            </td>
            <td class="px-6 py-4 whitespace-nowrap text-sm font-medium">
                <div class="flex space-x-2">
                    <A 
                        href=format!("/patients/{}", id)
                        class="text-blue-600 hover:text-blue-900"
                    >
                        "View"
                    </A>
                    <button class="text-green-600 hover:text-green-900">
                        "Edit"
                    </button>
                </div>
            </td>
        </tr>
    }
} 