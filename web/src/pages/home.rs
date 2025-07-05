//! Home page component

use leptos::*;
use leptos_router::*;

/// Home page component
#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <div class="max-w-6xl mx-auto">
            <div class="text-center mb-12">
                <h1 class="text-4xl font-bold text-gray-800 mb-4">
                    "Welcome to EMR Platform"
                </h1>
                <p class="text-xl text-gray-600 mb-8">
                    "HIPAA-compliant Electronic Medical Record system"
                </p>
                <div class="flex justify-center space-x-4">
                    <A 
                        href="/patients"
                        class="bg-blue-600 text-white px-6 py-3 rounded-lg hover:bg-blue-700 transition-colors font-medium"
                    >
                        "View Patients"
                    </A>
                    <button class="bg-green-600 text-white px-6 py-3 rounded-lg hover:bg-green-700 transition-colors font-medium">
                        "Add New Patient"
                    </button>
                </div>
            </div>

            <div class="grid grid-cols-1 md:grid-cols-3 gap-8 mb-12">
                <FeatureCard
                    title="Patient Management"
                    description="Comprehensive patient records and medical history"
                    icon="👥"
                />
                <FeatureCard
                    title="FHIR Integration"
                    description="Standards-compliant data exchange"
                    icon="🔗"
                />
                <FeatureCard
                    title="Security & Compliance"
                    description="HIPAA-grade security and audit trails"
                    icon="🔒"
                />
            </div>

            <div class="bg-white rounded-lg shadow-lg p-8">
                <h2 class="text-2xl font-bold text-gray-800 mb-6">
                    "Quick Stats"
                </h2>
                <div class="grid grid-cols-1 md:grid-cols-4 gap-6">
                    <StatCard title="Total Patients" value="1,234" />
                    <StatCard title="Active Sessions" value="23" />
                    <StatCard title="Records Today" value="45" />
                    <StatCard title="System Uptime" value="99.9%" />
                </div>
            </div>
        </div>
    }
}

/// Feature card component
#[component]
fn FeatureCard(
    title: &'static str,
    description: &'static str,
    icon: &'static str,
) -> impl IntoView {
    view! {
        <div class="bg-white rounded-lg shadow-lg p-6 text-center">
            <div class="text-4xl mb-4">{icon}</div>
            <h3 class="text-xl font-semibold text-gray-800 mb-2">{title}</h3>
            <p class="text-gray-600">{description}</p>
        </div>
    }
}

/// Statistics card component
#[component]
fn StatCard(
    title: &'static str,
    value: &'static str,
) -> impl IntoView {
    view! {
        <div class="text-center">
            <div class="text-3xl font-bold text-blue-600 mb-2">{value}</div>
            <div class="text-sm text-gray-600">{title}</div>
        </div>
    }
} 