CREATE TABLE care_programs (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL REFERENCES organizations(id),
 facility_id uuid NOT NULL, code text NOT NULL CHECK(length(trim(code)) BETWEEN 1 AND 40),
 name text NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 160),
 level_of_care text NOT NULL CHECK(level_of_care IN ('outpatient','iop','php')),
 min_age integer NOT NULL CHECK(min_age BETWEEN 18 AND 120), max_age integer,
 capacity integer NOT NULL CHECK(capacity BETWEEN 1 AND 10000), active boolean NOT NULL DEFAULT true,
 version integer NOT NULL DEFAULT 1 CHECK(version>0),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(id,organization_id), FOREIGN KEY(facility_id,organization_id) REFERENCES facilities(id,organization_id),
 CHECK(max_age IS NULL OR max_age BETWEEN min_age AND 130)
);
CREATE UNIQUE INDEX unique_program_code ON care_programs(organization_id,upper(trim(code)));
CREATE TABLE program_required_forms (
 program_id uuid NOT NULL, organization_id uuid NOT NULL, template_id uuid NOT NULL,
 stage text NOT NULL CHECK(stage IN ('intake','discharge')),
 PRIMARY KEY(program_id,template_id,stage),
 FOREIGN KEY(program_id,organization_id) REFERENCES care_programs(id,organization_id),
 FOREIGN KEY(template_id,organization_id) REFERENCES templates(id,organization_id)
);
CREATE TABLE program_revisions (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL, program_id uuid NOT NULL, version integer NOT NULL CHECK(version>0),
 snapshot jsonb NOT NULL CHECK(jsonb_typeof(snapshot)='object'), reason text NOT NULL CHECK(length(trim(reason)) BETWEEN 10 AND 2000),
 actor_id uuid NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(program_id,version), FOREIGN KEY(program_id,organization_id) REFERENCES care_programs(id,organization_id),
 FOREIGN KEY(actor_id,organization_id) REFERENCES staff(id,organization_id)
);
CREATE TRIGGER immutable_program_revisions BEFORE UPDATE OR DELETE ON program_revisions FOR EACH ROW EXECUTE FUNCTION protect_append_only();
CREATE TABLE program_services (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL, program_id uuid NOT NULL,
 code_system text NOT NULL CHECK(code_system IN ('internal','cpt','hcpcs')),
 service_code text NOT NULL CHECK(length(trim(service_code)) BETWEEN 1 AND 40),
 name text NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 160),
 session_kind text NOT NULL CHECK(session_kind IN ('individual','group','assessment','family','case_management')),
 min_minutes integer NOT NULL CHECK(min_minutes BETWEEN 1 AND 480),
 default_minutes integer NOT NULL, max_minutes integer NOT NULL CHECK(max_minutes BETWEEN 1 AND 480),
 default_charge_cents bigint CHECK(default_charge_cents BETWEEN 0 AND 100000000), note_template_id uuid,
 active boolean NOT NULL DEFAULT true, version integer NOT NULL DEFAULT 1 CHECK(version>0),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(id,organization_id),
 FOREIGN KEY(program_id,organization_id) REFERENCES care_programs(id,organization_id),
 FOREIGN KEY(note_template_id,organization_id) REFERENCES templates(id,organization_id),
 CHECK(min_minutes<=default_minutes AND default_minutes<=max_minutes)
);
CREATE UNIQUE INDEX unique_program_service_code ON program_services(program_id,code_system,upper(trim(service_code)));
CREATE TABLE program_service_revisions (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL, service_id uuid NOT NULL, version integer NOT NULL CHECK(version>0),
 snapshot jsonb NOT NULL CHECK(jsonb_typeof(snapshot)='object'), reason text NOT NULL CHECK(length(trim(reason)) BETWEEN 10 AND 2000),
 actor_id uuid NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(service_id,version), FOREIGN KEY(service_id,organization_id) REFERENCES program_services(id,organization_id),
 FOREIGN KEY(actor_id,organization_id) REFERENCES staff(id,organization_id)
);
CREATE TRIGGER immutable_program_service_revisions BEFORE UPDATE OR DELETE ON program_service_revisions FOR EACH ROW EXECUTE FUNCTION protect_append_only();
ALTER TABLE episodes ADD COLUMN program_id uuid, ADD FOREIGN KEY(program_id,organization_id) REFERENCES care_programs(id,organization_id);
CREATE INDEX episodes_program_status ON episodes(organization_id,program_id,status);
ALTER TABLE sessions ADD COLUMN program_service_id uuid, ADD COLUMN service_snapshot jsonb NOT NULL DEFAULT '{}',
 ADD FOREIGN KEY(program_service_id,organization_id) REFERENCES program_services(id,organization_id),
 ADD CHECK(jsonb_typeof(service_snapshot)='object' AND ((program_service_id IS NULL AND service_snapshot='{}') OR (program_service_id IS NOT NULL AND COALESCE(service_snapshot->>'id'=program_service_id::text,false))));
ALTER TABLE groups ADD COLUMN program_service_id uuid, ADD COLUMN service_snapshot jsonb NOT NULL DEFAULT '{}',
 ADD FOREIGN KEY(program_service_id,organization_id) REFERENCES program_services(id,organization_id),
 ADD CHECK(jsonb_typeof(service_snapshot)='object' AND ((program_service_id IS NULL AND service_snapshot='{}') OR (program_service_id IS NOT NULL AND COALESCE(service_snapshot->>'id'=program_service_id::text,false))));
CREATE TABLE episode_program_assignments (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL, episode_id uuid NOT NULL, client_id uuid NOT NULL,
 program_id uuid NOT NULL, actor_id uuid NOT NULL, reason text NOT NULL CHECK(length(trim(reason)) BETWEEN 10 AND 2000),
 snapshot jsonb NOT NULL CHECK(jsonb_typeof(snapshot)='object'), created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(episode_id), FOREIGN KEY(episode_id,client_id,organization_id) REFERENCES episodes(id,client_id,organization_id),
 FOREIGN KEY(program_id,organization_id) REFERENCES care_programs(id,organization_id),
 FOREIGN KEY(actor_id,organization_id) REFERENCES staff(id,organization_id)
);
CREATE TRIGGER immutable_episode_program_assignments BEFORE UPDATE OR DELETE ON episode_program_assignments FOR EACH ROW EXECUTE FUNCTION protect_append_only();
ALTER TABLE intake_events ADD CONSTRAINT intake_events_org_unique UNIQUE(id,organization_id);
CREATE TABLE intake_decision_evidence (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL, intake_case_id uuid NOT NULL, event_id uuid NOT NULL,
 client_id uuid NOT NULL, episode_id uuid NOT NULL, program_id uuid NOT NULL, program_version integer NOT NULL,
 program_snapshot jsonb NOT NULL CHECK(jsonb_typeof(program_snapshot)='object'), checklist jsonb NOT NULL CHECK(jsonb_typeof(checklist)='array'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), UNIQUE(event_id),
 FOREIGN KEY(intake_case_id,client_id,organization_id) REFERENCES intake_cases(id,client_id,organization_id),
 FOREIGN KEY(event_id,organization_id) REFERENCES intake_events(id,organization_id),
 FOREIGN KEY(episode_id,client_id,organization_id) REFERENCES episodes(id,client_id,organization_id),
 FOREIGN KEY(program_id,organization_id) REFERENCES care_programs(id,organization_id),
 FOREIGN KEY(program_id,program_version) REFERENCES program_revisions(program_id,version)
);
CREATE TRIGGER immutable_intake_decision_evidence BEFORE UPDATE OR DELETE ON intake_decision_evidence FOR EACH ROW EXECUTE FUNCTION protect_append_only();
CREATE FUNCTION preserve_program_context() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.organization_id IS DISTINCT FROM OLD.organization_id OR NEW.facility_id IS DISTINCT FROM OLD.facility_id OR NEW.code IS DISTINCT FROM OLD.code OR NEW.level_of_care IS DISTINCT FROM OLD.level_of_care THEN RAISE EXCEPTION 'Program context is immutable'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER immutable_program_context BEFORE UPDATE ON care_programs FOR EACH ROW EXECUTE FUNCTION preserve_program_context();
CREATE FUNCTION preserve_service_context() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.organization_id IS DISTINCT FROM OLD.organization_id OR NEW.program_id IS DISTINCT FROM OLD.program_id OR NEW.code_system IS DISTINCT FROM OLD.code_system OR NEW.service_code IS DISTINCT FROM OLD.service_code OR NEW.session_kind IS DISTINCT FROM OLD.session_kind THEN RAISE EXCEPTION 'Service context is immutable'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER immutable_service_context BEFORE UPDATE ON program_services FOR EACH ROW EXECUTE FUNCTION preserve_service_context();
CREATE FUNCTION protect_episode_program() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE p care_programs;
BEGIN
 IF TG_OP='UPDATE' AND OLD.program_id IS NOT NULL AND NEW.program_id IS DISTINCT FROM OLD.program_id THEN RAISE EXCEPTION 'Assigned episode program is immutable'; END IF;
 IF NEW.program_id IS NOT NULL THEN
  SELECT * INTO STRICT p FROM care_programs WHERE id=NEW.program_id AND organization_id=NEW.organization_id FOR UPDATE;
  IF NEW.facility_id<>p.facility_id OR NEW.level_of_care<>p.level_of_care THEN RAISE EXCEPTION 'Episode program context does not match'; END IF;
  IF NEW.status='active' AND (TG_OP='INSERT' OR OLD.status IS DISTINCT FROM NEW.status OR OLD.program_id IS DISTINCT FROM NEW.program_id) THEN
   IF NOT p.active OR (SELECT count(*) FROM episodes WHERE program_id=p.id AND organization_id=p.organization_id AND status='active' AND id<>NEW.id)>=p.capacity THEN RAISE EXCEPTION 'Program is inactive or full'; END IF;
  END IF;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER protect_episode_program BEFORE INSERT OR UPDATE ON episodes FOR EACH ROW EXECUTE FUNCTION protect_episode_program();
CREATE FUNCTION preserve_encounter_service_snapshot() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF OLD.program_service_id IS NOT NULL AND NEW.program_service_id IS NULL THEN RAISE EXCEPTION 'Encounter service cannot be removed'; END IF;
 IF NEW.program_service_id IS NOT DISTINCT FROM OLD.program_service_id AND NEW.service_snapshot IS DISTINCT FROM OLD.service_snapshot THEN RAISE EXCEPTION 'Encounter service snapshot is immutable'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER preserve_session_service_snapshot BEFORE UPDATE ON sessions FOR EACH ROW EXECUTE FUNCTION preserve_encounter_service_snapshot();
CREATE TRIGGER preserve_group_service_snapshot BEFORE UPDATE ON groups FOR EACH ROW EXECUTE FUNCTION preserve_encounter_service_snapshot();
