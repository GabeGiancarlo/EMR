-- Intake cases, disposition history, and staff identity attestations.
-- No document numbers are stored. Episode activation stays in the application
-- transition so this migration must not forbid planned to active; generic API
-- updates are rejected by admissions::before_episode_update.
CREATE TABLE intake_cases (
 id uuid PRIMARY KEY,
 organization_id uuid NOT NULL REFERENCES organizations(id),
 client_id uuid NOT NULL,
 episode_id uuid NOT NULL,
 owner_id uuid NOT NULL,
 referral_type text NOT NULL CHECK(referral_type IN ('self','provider','facility','family','justice','other')),
 referrer_name text NOT NULL DEFAULT '' CHECK(length(referrer_name)<=200),
 referrer_contact text NOT NULL DEFAULT '' CHECK(length(referrer_contact)<=200),
 referral_reason text NOT NULL CHECK(length(trim(referral_reason)) BETWEEN 1 AND 2000),
 received_on date NOT NULL,
 status text NOT NULL DEFAULT 'open' CHECK(status IN ('open','screening','ready','admitted','declined','withdrawn')),
 disposition_reason text NOT NULL DEFAULT '' CHECK(length(disposition_reason)<=2000),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(id,organization_id),
 UNIQUE(episode_id,organization_id),
 UNIQUE(id,client_id,organization_id),
 FOREIGN KEY(client_id,organization_id) REFERENCES clients(id,organization_id),
 FOREIGN KEY(episode_id,client_id,organization_id) REFERENCES episodes(id,client_id,organization_id),
 FOREIGN KEY(owner_id,organization_id) REFERENCES staff(id,organization_id),
 CHECK((status='open' AND disposition_reason='') OR (status<>'open' AND length(trim(disposition_reason)) BETWEEN 10 AND 2000))
);
CREATE UNIQUE INDEX one_nonterminal_intake_case ON intake_cases(organization_id,client_id) WHERE status IN ('open','screening','ready');
CREATE INDEX intake_cases_client ON intake_cases(organization_id,client_id,created_at DESC);
CREATE INDEX intake_cases_episode ON intake_cases(organization_id,episode_id);
CREATE TABLE intake_events (
 id uuid PRIMARY KEY,
 organization_id uuid NOT NULL REFERENCES organizations(id),
 client_id uuid NOT NULL,
 intake_case_id uuid NOT NULL,
 from_status text NOT NULL CHECK(from_status IN ('open','screening','ready','admitted','declined','withdrawn')),
 to_status text NOT NULL CHECK(to_status IN ('open','screening','ready','admitted','declined','withdrawn')),
 reason text NOT NULL CHECK(length(trim(reason)) BETWEEN 10 AND 2000),
 actor_id uuid NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 FOREIGN KEY(intake_case_id,client_id,organization_id) REFERENCES intake_cases(id,client_id,organization_id),
 FOREIGN KEY(actor_id,organization_id) REFERENCES staff(id,organization_id)
);
CREATE INDEX intake_events_case ON intake_events(organization_id,intake_case_id,created_at);
CREATE TRIGGER immutable_intake_events BEFORE UPDATE OR DELETE ON intake_events FOR EACH ROW EXECUTE FUNCTION protect_append_only();
CREATE TABLE patient_identity_checks (
 id uuid PRIMARY KEY,
 organization_id uuid NOT NULL REFERENCES organizations(id),
 client_id uuid NOT NULL,
 actor_id uuid NOT NULL,
 method text NOT NULL CHECK(method IN ('patient_report','document_review','prior_record')),
 outcome text NOT NULL CHECK(outcome IN ('confirmed','unable')),
 reason text NOT NULL CHECK(length(trim(reason)) BETWEEN 10 AND 2000),
 attested boolean NOT NULL CHECK(attested),
 identity_fingerprint text NOT NULL CHECK(length(identity_fingerprint)=64),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 FOREIGN KEY(client_id,organization_id) REFERENCES clients(id,organization_id),
 FOREIGN KEY(actor_id,organization_id) REFERENCES staff(id,organization_id)
);
CREATE INDEX identity_checks_client ON patient_identity_checks(organization_id,client_id,created_at DESC);
CREATE TRIGGER immutable_identity_checks BEFORE UPDATE OR DELETE ON patient_identity_checks FOR EACH ROW EXECUTE FUNCTION protect_append_only();
CREATE FUNCTION preserve_intake_case_links() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.client_id IS DISTINCT FROM OLD.client_id OR NEW.episode_id IS DISTINCT FROM OLD.episode_id OR NEW.organization_id IS DISTINCT FROM OLD.organization_id THEN
  RAISE EXCEPTION 'Intake case links are immutable';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER preserve_intake_case_links BEFORE UPDATE ON intake_cases FOR EACH ROW EXECUTE FUNCTION preserve_intake_case_links();
CREATE FUNCTION enforce_intake_status_transition() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.status IS DISTINCT FROM OLD.status AND NOT (
  (OLD.status='open' AND NEW.status IN ('screening','declined','withdrawn')) OR
  (OLD.status='screening' AND NEW.status IN ('ready','declined','withdrawn')) OR
  (OLD.status='ready' AND NEW.status IN ('screening','admitted','declined','withdrawn'))
 ) THEN
  RAISE EXCEPTION 'Intake status transition is not allowed';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER enforce_intake_status_transition BEFORE UPDATE ON intake_cases FOR EACH ROW EXECUTE FUNCTION enforce_intake_status_transition();
CREATE FUNCTION reject_intake_case_delete() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 RAISE EXCEPTION 'Intake cases are retained; record a disposition instead';
END $$;
CREATE TRIGGER no_delete_intake_case BEFORE DELETE ON intake_cases FOR EACH ROW EXECUTE FUNCTION reject_intake_case_delete();
