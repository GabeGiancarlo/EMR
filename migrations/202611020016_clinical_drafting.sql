CREATE TABLE clinical_draft_jobs (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL REFERENCES organizations(id), client_id uuid NOT NULL,
 episode_id uuid, session_id uuid, requested_by uuid NOT NULL, template_id uuid NOT NULL,
 title text NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 200),
 purpose text NOT NULL CHECK(length(trim(purpose)) BETWEEN 10 AND 2000),
 template_snapshot jsonb NOT NULL CHECK(jsonb_typeof(template_snapshot)='object'),
 sources jsonb NOT NULL CHECK(jsonb_typeof(sources)='array' AND jsonb_array_length(sources) BETWEEN 1 AND 12),
 state text NOT NULL DEFAULT 'queued' CHECK(state IN ('queued','running','ready','failed','cancelled')),
 output jsonb, error_code text NOT NULL DEFAULT '', created_at timestamptz NOT NULL DEFAULT now(),
 started_at timestamptz, completed_at timestamptz,
 UNIQUE(id,organization_id), UNIQUE(id,client_id,organization_id),
 CHECK((state='ready')=(output IS NOT NULL)),
 FOREIGN KEY(client_id,organization_id) REFERENCES clients(id,organization_id),
 FOREIGN KEY(episode_id,client_id,organization_id) REFERENCES episodes(id,client_id,organization_id),
 FOREIGN KEY(session_id,client_id,organization_id) REFERENCES sessions(id,client_id,organization_id),
 FOREIGN KEY(requested_by,organization_id) REFERENCES staff(id,organization_id),
 FOREIGN KEY(template_id,organization_id) REFERENCES templates(id,organization_id)
);
CREATE UNIQUE INDEX clinical_draft_one_pending ON clinical_draft_jobs(client_id) WHERE state IN ('queued','running');
CREATE INDEX clinical_draft_queue ON clinical_draft_jobs(created_at) WHERE state='queued';
ALTER TABLE notes ADD CONSTRAINT notes_patient_identity UNIQUE(id,client_id,organization_id);
CREATE TABLE clinical_draft_reviews (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL, client_id uuid NOT NULL, job_id uuid NOT NULL UNIQUE,
 reviewer_id uuid NOT NULL, decision text NOT NULL CHECK(decision IN ('accept','reject')),
 reason text NOT NULL CHECK(length(trim(reason)) BETWEEN 10 AND 2000),
 decisions jsonb NOT NULL CHECK(jsonb_typeof(decisions)='array'),
 note_id uuid, created_at timestamptz NOT NULL DEFAULT now(),
 CHECK((decision='accept')=(note_id IS NOT NULL)),
 FOREIGN KEY(job_id,client_id,organization_id) REFERENCES clinical_draft_jobs(id,client_id,organization_id),
 FOREIGN KEY(reviewer_id,organization_id) REFERENCES staff(id,organization_id),
 FOREIGN KEY(note_id,client_id,organization_id) REFERENCES notes(id,client_id,organization_id)
);
CREATE TRIGGER clinical_review_history BEFORE UPDATE OR DELETE ON clinical_draft_reviews FOR EACH ROW EXECUTE FUNCTION protect_append_only();
ALTER TABLE notes ADD COLUMN clinical_draft_job_id uuid;
ALTER TABLE notes ADD CONSTRAINT note_generation_patient FOREIGN KEY(clinical_draft_job_id,client_id,organization_id) REFERENCES clinical_draft_jobs(id,client_id,organization_id);
CREATE FUNCTION protect_clinical_draft_job() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' OR OLD.state IN ('ready','failed','cancelled') THEN RAISE EXCEPTION 'Generation history is immutable'; END IF;
 IF (to_jsonb(NEW)-ARRAY['state','output','error_code','started_at','completed_at']) IS DISTINCT FROM (to_jsonb(OLD)-ARRAY['state','output','error_code','started_at','completed_at']) THEN RAISE EXCEPTION 'Generation input is immutable'; END IF;
 IF NOT ((OLD.state='queued' AND NEW.state IN ('running','failed','cancelled')) OR (OLD.state='running' AND NEW.state IN ('ready','failed','cancelled'))) THEN RAISE EXCEPTION 'Invalid generation transition'; END IF;
 RETURN NEW;
END; $$;
CREATE TRIGGER clinical_draft_history BEFORE UPDATE OR DELETE ON clinical_draft_jobs FOR EACH ROW EXECUTE FUNCTION protect_clinical_draft_job();
