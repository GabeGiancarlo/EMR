-- EMR Platform Database Initialization
-- This script initializes the PostgreSQL database for the EMR platform

-- Enable necessary extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
CREATE EXTENSION IF NOT EXISTS "pg_trgm";

-- Create schemas
CREATE SCHEMA IF NOT EXISTS emr;
CREATE SCHEMA IF NOT EXISTS fhir;
CREATE SCHEMA IF NOT EXISTS audit;
CREATE SCHEMA IF NOT EXISTS jobs;

-- Set search path
SET search_path TO emr, fhir, audit, jobs, public;

-- Create audit table for all database changes
CREATE TABLE IF NOT EXISTS audit.audit_log (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    table_name VARCHAR(255) NOT NULL,
    operation VARCHAR(10) NOT NULL,
    old_values JSONB,
    new_values JSONB,
    changed_by VARCHAR(255),
    changed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    request_id VARCHAR(255),
    user_id UUID,
    session_id VARCHAR(255)
);

-- Create jobs table for background processing
CREATE TABLE IF NOT EXISTS jobs.job_queue (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    job_type VARCHAR(255) NOT NULL,
    job_data JSONB NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    priority INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    retry_count INTEGER NOT NULL DEFAULT 0,
    scheduled_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    started_at TIMESTAMP WITH TIME ZONE,
    completed_at TIMESTAMP WITH TIME ZONE,
    failed_at TIMESTAMP WITH TIME ZONE,
    error_message TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create indexes for jobs table
CREATE INDEX IF NOT EXISTS idx_job_queue_status ON jobs.job_queue(status);
CREATE INDEX IF NOT EXISTS idx_job_queue_scheduled_at ON jobs.job_queue(scheduled_at);
CREATE INDEX IF NOT EXISTS idx_job_queue_job_type ON jobs.job_queue(job_type);
CREATE INDEX IF NOT EXISTS idx_job_queue_priority ON jobs.job_queue(priority DESC);

-- Create patients table
CREATE TABLE IF NOT EXISTS emr.patients (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    fhir_id VARCHAR(255) UNIQUE,
    active BOOLEAN NOT NULL DEFAULT true,
    family_name VARCHAR(255),
    given_names VARCHAR(255)[],
    birth_date DATE,
    gender VARCHAR(50),
    phone VARCHAR(50),
    email VARCHAR(255),
    address_line1 VARCHAR(255),
    address_line2 VARCHAR(255),
    city VARCHAR(100),
    state VARCHAR(100),
    postal_code VARCHAR(20),
    country VARCHAR(100),
    identifiers JSONB,
    contact_points JSONB,
    fhir_data JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_by UUID,
    updated_by UUID
);

-- Create indexes for patients table
CREATE INDEX IF NOT EXISTS idx_patients_fhir_id ON emr.patients(fhir_id);
CREATE INDEX IF NOT EXISTS idx_patients_family_name ON emr.patients(family_name);
CREATE INDEX IF NOT EXISTS idx_patients_given_names ON emr.patients USING GIN(given_names);
CREATE INDEX IF NOT EXISTS idx_patients_birth_date ON emr.patients(birth_date);
CREATE INDEX IF NOT EXISTS idx_patients_active ON emr.patients(active);
CREATE INDEX IF NOT EXISTS idx_patients_identifiers ON emr.patients USING GIN(identifiers);
CREATE INDEX IF NOT EXISTS idx_patients_fhir_data ON emr.patients USING GIN(fhir_data);

-- Create organizations table
CREATE TABLE IF NOT EXISTS emr.organizations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    fhir_id VARCHAR(255) UNIQUE,
    active BOOLEAN NOT NULL DEFAULT true,
    name VARCHAR(255) NOT NULL,
    type VARCHAR(100),
    phone VARCHAR(50),
    email VARCHAR(255),
    website VARCHAR(255),
    address_line1 VARCHAR(255),
    address_line2 VARCHAR(255),
    city VARCHAR(100),
    state VARCHAR(100),
    postal_code VARCHAR(20),
    country VARCHAR(100),
    identifiers JSONB,
    contact_points JSONB,
    fhir_data JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_by UUID,
    updated_by UUID
);

-- Create indexes for organizations table
CREATE INDEX IF NOT EXISTS idx_organizations_fhir_id ON emr.organizations(fhir_id);
CREATE INDEX IF NOT EXISTS idx_organizations_name ON emr.organizations(name);
CREATE INDEX IF NOT EXISTS idx_organizations_active ON emr.organizations(active);
CREATE INDEX IF NOT EXISTS idx_organizations_identifiers ON emr.organizations USING GIN(identifiers);

-- Create practitioners table
CREATE TABLE IF NOT EXISTS emr.practitioners (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    fhir_id VARCHAR(255) UNIQUE,
    active BOOLEAN NOT NULL DEFAULT true,
    family_name VARCHAR(255),
    given_names VARCHAR(255)[],
    qualification VARCHAR(255),
    specialties VARCHAR(255)[],
    phone VARCHAR(50),
    email VARCHAR(255),
    identifiers JSONB,
    contact_points JSONB,
    fhir_data JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_by UUID,
    updated_by UUID
);

-- Create indexes for practitioners table
CREATE INDEX IF NOT EXISTS idx_practitioners_fhir_id ON emr.practitioners(fhir_id);
CREATE INDEX IF NOT EXISTS idx_practitioners_family_name ON emr.practitioners(family_name);
CREATE INDEX IF NOT EXISTS idx_practitioners_given_names ON emr.practitioners USING GIN(given_names);
CREATE INDEX IF NOT EXISTS idx_practitioners_active ON emr.practitioners(active);
CREATE INDEX IF NOT EXISTS idx_practitioners_specialties ON emr.practitioners USING GIN(specialties);

-- Create encounters table
CREATE TABLE IF NOT EXISTS emr.encounters (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    fhir_id VARCHAR(255) UNIQUE,
    status VARCHAR(50) NOT NULL,
    class VARCHAR(50),
    type VARCHAR(100),
    patient_id UUID REFERENCES emr.patients(id),
    organization_id UUID REFERENCES emr.organizations(id),
    practitioner_id UUID REFERENCES emr.practitioners(id),
    start_date TIMESTAMP WITH TIME ZONE,
    end_date TIMESTAMP WITH TIME ZONE,
    reason_code VARCHAR(255),
    reason_description TEXT,
    fhir_data JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_by UUID,
    updated_by UUID
);

-- Create indexes for encounters table
CREATE INDEX IF NOT EXISTS idx_encounters_fhir_id ON emr.encounters(fhir_id);
CREATE INDEX IF NOT EXISTS idx_encounters_patient_id ON emr.encounters(patient_id);
CREATE INDEX IF NOT EXISTS idx_encounters_organization_id ON emr.encounters(organization_id);
CREATE INDEX IF NOT EXISTS idx_encounters_practitioner_id ON emr.encounters(practitioner_id);
CREATE INDEX IF NOT EXISTS idx_encounters_start_date ON emr.encounters(start_date);
CREATE INDEX IF NOT EXISTS idx_encounters_status ON emr.encounters(status);

-- Create observations table
CREATE TABLE IF NOT EXISTS emr.observations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    fhir_id VARCHAR(255) UNIQUE,
    status VARCHAR(50) NOT NULL,
    category VARCHAR(100),
    code VARCHAR(100),
    display VARCHAR(255),
    patient_id UUID REFERENCES emr.patients(id),
    encounter_id UUID REFERENCES emr.encounters(id),
    effective_date TIMESTAMP WITH TIME ZONE,
    value_quantity_value DECIMAL,
    value_quantity_unit VARCHAR(50),
    value_string TEXT,
    value_boolean BOOLEAN,
    interpretation VARCHAR(100),
    reference_range_low DECIMAL,
    reference_range_high DECIMAL,
    fhir_data JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_by UUID,
    updated_by UUID
);

-- Create indexes for observations table
CREATE INDEX IF NOT EXISTS idx_observations_fhir_id ON emr.observations(fhir_id);
CREATE INDEX IF NOT EXISTS idx_observations_patient_id ON emr.observations(patient_id);
CREATE INDEX IF NOT EXISTS idx_observations_encounter_id ON emr.observations(encounter_id);
CREATE INDEX IF NOT EXISTS idx_observations_effective_date ON emr.observations(effective_date);
CREATE INDEX IF NOT EXISTS idx_observations_code ON emr.observations(code);
CREATE INDEX IF NOT EXISTS idx_observations_category ON emr.observations(category);

-- Create users table for authentication
CREATE TABLE IF NOT EXISTS emr.users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    username VARCHAR(255) UNIQUE NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    first_name VARCHAR(255),
    last_name VARCHAR(255),
    role VARCHAR(50) NOT NULL DEFAULT 'user',
    active BOOLEAN NOT NULL DEFAULT true,
    last_login TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create indexes for users table
CREATE INDEX IF NOT EXISTS idx_users_username ON emr.users(username);
CREATE INDEX IF NOT EXISTS idx_users_email ON emr.users(email);
CREATE INDEX IF NOT EXISTS idx_users_active ON emr.users(active);
CREATE INDEX IF NOT EXISTS idx_users_role ON emr.users(role);

-- Create sessions table
CREATE TABLE IF NOT EXISTS emr.sessions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID REFERENCES emr.users(id) ON DELETE CASCADE,
    session_token VARCHAR(255) UNIQUE NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_accessed TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    ip_address INET,
    user_agent TEXT
);

-- Create indexes for sessions table
CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON emr.sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_session_token ON emr.sessions(session_token);
CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON emr.sessions(expires_at);

-- Create audit triggers for all tables
CREATE OR REPLACE FUNCTION audit.audit_trigger_function() RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO audit.audit_log (table_name, operation, new_values, changed_by, request_id)
        VALUES (TG_TABLE_NAME, TG_OP, to_jsonb(NEW), current_user, current_setting('application.request_id', true));
        RETURN NEW;
    ELSIF TG_OP = 'UPDATE' THEN
        INSERT INTO audit.audit_log (table_name, operation, old_values, new_values, changed_by, request_id)
        VALUES (TG_TABLE_NAME, TG_OP, to_jsonb(OLD), to_jsonb(NEW), current_user, current_setting('application.request_id', true));
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        INSERT INTO audit.audit_log (table_name, operation, old_values, changed_by, request_id)
        VALUES (TG_TABLE_NAME, TG_OP, to_jsonb(OLD), current_user, current_setting('application.request_id', true));
        RETURN OLD;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Create audit triggers for all tables
CREATE TRIGGER audit_patients AFTER INSERT OR UPDATE OR DELETE ON emr.patients FOR EACH ROW EXECUTE FUNCTION audit.audit_trigger_function();
CREATE TRIGGER audit_organizations AFTER INSERT OR UPDATE OR DELETE ON emr.organizations FOR EACH ROW EXECUTE FUNCTION audit.audit_trigger_function();
CREATE TRIGGER audit_practitioners AFTER INSERT OR UPDATE OR DELETE ON emr.practitioners FOR EACH ROW EXECUTE FUNCTION audit.audit_trigger_function();
CREATE TRIGGER audit_encounters AFTER INSERT OR UPDATE OR DELETE ON emr.encounters FOR EACH ROW EXECUTE FUNCTION audit.audit_trigger_function();
CREATE TRIGGER audit_observations AFTER INSERT OR UPDATE OR DELETE ON emr.observations FOR EACH ROW EXECUTE FUNCTION audit.audit_trigger_function();
CREATE TRIGGER audit_users AFTER INSERT OR UPDATE OR DELETE ON emr.users FOR EACH ROW EXECUTE FUNCTION audit.audit_trigger_function();
CREATE TRIGGER audit_sessions AFTER INSERT OR UPDATE OR DELETE ON emr.sessions FOR EACH ROW EXECUTE FUNCTION audit.audit_trigger_function();

-- Create updated_at trigger function
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create updated_at triggers for all tables
CREATE TRIGGER update_patients_updated_at BEFORE UPDATE ON emr.patients FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_organizations_updated_at BEFORE UPDATE ON emr.organizations FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_practitioners_updated_at BEFORE UPDATE ON emr.practitioners FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_encounters_updated_at BEFORE UPDATE ON emr.encounters FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_observations_updated_at BEFORE UPDATE ON emr.observations FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON emr.users FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_sessions_updated_at BEFORE UPDATE ON emr.sessions FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Insert default admin user (password: admin123 - change in production!)
INSERT INTO emr.users (username, email, password_hash, first_name, last_name, role) 
VALUES (
    'admin',
    'admin@emr.local',
    '$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewqeA9pKpNRkGKgO', -- admin123
    'System',
    'Administrator',
    'admin'
) ON CONFLICT (username) DO NOTHING;

-- Create sample data for development
INSERT INTO emr.patients (fhir_id, family_name, given_names, birth_date, gender, phone, email) VALUES
('patient-1', 'Smith', ARRAY['John', 'David'], '1985-06-15', 'male', '+1-555-0123', 'john.smith@example.com'),
('patient-2', 'Johnson', ARRAY['Sarah', 'Marie'], '1990-03-22', 'female', '+1-555-0456', 'sarah.johnson@example.com'),
('patient-3', 'Williams', ARRAY['Michael', 'Robert'], '1978-11-08', 'male', '+1-555-0789', 'michael.williams@example.com')
ON CONFLICT (fhir_id) DO NOTHING;

INSERT INTO emr.organizations (fhir_id, name, type, phone, email) VALUES
('org-1', 'General Hospital', 'hospital', '+1-555-0100', 'info@generalhospital.com'),
('org-2', 'Family Practice Clinic', 'clinic', '+1-555-0200', 'contact@familypractice.com')
ON CONFLICT (fhir_id) DO NOTHING;

INSERT INTO emr.practitioners (fhir_id, family_name, given_names, qualification, specialties, phone, email) VALUES
('practitioner-1', 'Brown', ARRAY['Dr.', 'Emily'], 'MD', ARRAY['Family Medicine'], '+1-555-0301', 'dr.brown@example.com'),
('practitioner-2', 'Davis', ARRAY['Dr.', 'James'], 'MD', ARRAY['Cardiology'], '+1-555-0302', 'dr.davis@example.com')
ON CONFLICT (fhir_id) DO NOTHING;

-- Grant permissions
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA emr TO emr_user;
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA fhir TO emr_user;
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA audit TO emr_user;
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA jobs TO emr_user;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA emr TO emr_user;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA fhir TO emr_user;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA audit TO emr_user;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA jobs TO emr_user;
GRANT USAGE ON SCHEMA emr TO emr_user;
GRANT USAGE ON SCHEMA fhir TO emr_user;
GRANT USAGE ON SCHEMA audit TO emr_user;
GRANT USAGE ON SCHEMA jobs TO emr_user; 