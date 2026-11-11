ALTER TABLE recording_tracks ADD CONSTRAINT recording_track_recording_org UNIQUE(id,recording_id,organization_id);
CREATE TABLE transcription_jobs (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL, room_id uuid NOT NULL, recording_id uuid NOT NULL, track_id uuid NOT NULL,
 requested_by uuid NOT NULL, purpose text NOT NULL CHECK(length(trim(purpose)) BETWEEN 10 AND 2000),
 consent_snapshot jsonb NOT NULL CHECK(jsonb_typeof(consent_snapshot)='array'),
 state text NOT NULL DEFAULT 'queued' CHECK(state IN ('queued','running','ready','failed','cancelled')),
 error_code text NOT NULL DEFAULT '', created_at timestamptz NOT NULL DEFAULT now(), started_at timestamptz, completed_at timestamptz,
 UNIQUE(id,organization_id), UNIQUE(id,track_id,organization_id),
 FOREIGN KEY(room_id,organization_id) REFERENCES telehealth_rooms(id,organization_id),
 FOREIGN KEY(recording_id,room_id) REFERENCES recordings(id,room_id),
 FOREIGN KEY(track_id,recording_id,organization_id) REFERENCES recording_tracks(id,recording_id,organization_id),
 FOREIGN KEY(requested_by,organization_id) REFERENCES staff(id,organization_id)
);
CREATE UNIQUE INDEX transcription_one_active_track ON transcription_jobs(track_id) WHERE state IN ('queued','running');
CREATE INDEX transcription_queue ON transcription_jobs(created_at) WHERE state='queued';
CREATE TABLE transcripts (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL, room_id uuid NOT NULL, recording_id uuid NOT NULL, track_id uuid NOT NULL, job_id uuid NOT NULL UNIQUE,
 duration_ms bigint NOT NULL CHECK(duration_ms BETWEEN 1 AND 7200000), engine text NOT NULL, model_label text NOT NULL, model_sha256 text NOT NULL CHECK(length(model_sha256)=64), source_sha256 text NOT NULL CHECK(length(source_sha256)=64),
 recognized_segments jsonb NOT NULL CHECK(jsonb_typeof(recognized_segments)='array'),
 current_version_id uuid NOT NULL, created_at timestamptz NOT NULL DEFAULT now(), UNIQUE(id,organization_id),
 FOREIGN KEY(job_id,track_id,organization_id) REFERENCES transcription_jobs(id,track_id,organization_id),
 FOREIGN KEY(track_id,recording_id,organization_id) REFERENCES recording_tracks(id,recording_id,organization_id),
 FOREIGN KEY(recording_id,room_id) REFERENCES recordings(id,room_id)
);
CREATE TABLE transcript_versions (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL, transcript_id uuid NOT NULL, number integer NOT NULL CHECK(number>0), parent_id uuid,
 state text NOT NULL DEFAULT 'draft' CHECK(state IN ('draft','reviewed')),
 segments jsonb NOT NULL CHECK(jsonb_typeof(segments)='array' AND jsonb_array_length(segments)<=10000),
 reason text NOT NULL CHECK(length(trim(reason)) BETWEEN 10 AND 2000), created_by uuid NOT NULL, created_at timestamptz NOT NULL DEFAULT now(), reviewed_by uuid, reviewed_at timestamptz,
 UNIQUE(id,organization_id), UNIQUE(id,transcript_id), UNIQUE(transcript_id,number),
 FOREIGN KEY(transcript_id,organization_id) REFERENCES transcripts(id,organization_id),
 FOREIGN KEY(parent_id,transcript_id) REFERENCES transcript_versions(id,transcript_id),
 FOREIGN KEY(created_by,organization_id) REFERENCES staff(id,organization_id), FOREIGN KEY(reviewed_by,organization_id) REFERENCES staff(id,organization_id),
 CHECK((state='reviewed')=(reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL))
);
ALTER TABLE transcripts ADD CONSTRAINT transcript_current_version FOREIGN KEY(current_version_id,id) REFERENCES transcript_versions(id,transcript_id) DEFERRABLE INITIALLY DEFERRED;
CREATE TABLE transcript_excerpts (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL, transcript_id uuid NOT NULL, version_id uuid NOT NULL,
 client_id uuid NOT NULL, episode_id uuid NOT NULL, session_id uuid NOT NULL,
 segment_ids uuid[] NOT NULL CHECK(cardinality(segment_ids) BETWEEN 1 AND 10000), text text NOT NULL CHECK(length(trim(text)) BETWEEN 1 AND 100000), created_by uuid NOT NULL, created_at timestamptz NOT NULL DEFAULT now(),
 UNIQUE(id,client_id,organization_id),
 FOREIGN KEY(transcript_id,organization_id) REFERENCES transcripts(id,organization_id), FOREIGN KEY(version_id,transcript_id) REFERENCES transcript_versions(id,transcript_id),
 FOREIGN KEY(session_id,client_id,organization_id) REFERENCES sessions(id,client_id,organization_id), FOREIGN KEY(episode_id,client_id,organization_id) REFERENCES episodes(id,client_id,organization_id),
 FOREIGN KEY(created_by,organization_id) REFERENCES staff(id,organization_id)
);
CREATE FUNCTION protect_transcript_version() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'Transcript versions cannot be deleted'; END IF;
 IF OLD.state<>'draft' OR NEW.state<>'reviewed' OR (to_jsonb(NEW)-ARRAY['state','reviewed_by','reviewed_at']) IS DISTINCT FROM (to_jsonb(OLD)-ARRAY['state','reviewed_by','reviewed_at']) THEN
   RAISE EXCEPTION 'Save a new transcript version to preserve corrections';
 END IF;
 RETURN NEW;
END; $$;
CREATE TRIGGER immutable_transcript_version BEFORE UPDATE OR DELETE ON transcript_versions FOR EACH ROW EXECUTE FUNCTION protect_transcript_version();
CREATE FUNCTION protect_transcript_source() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'Transcript sources cannot be deleted'; END IF;
 IF (to_jsonb(NEW)-'current_version_id') IS DISTINCT FROM (to_jsonb(OLD)-'current_version_id') THEN RAISE EXCEPTION 'Recognized source is immutable'; END IF;
 RETURN NEW;
END; $$;
CREATE TRIGGER immutable_transcript_source BEFORE UPDATE OR DELETE ON transcripts FOR EACH ROW EXECUTE FUNCTION protect_transcript_source();
CREATE TRIGGER immutable_transcript_excerpt BEFORE UPDATE OR DELETE ON transcript_excerpts FOR EACH ROW EXECUTE FUNCTION protect_append_only();
CREATE INDEX transcript_room_history ON transcripts(room_id,created_at);

ALTER TABLE form_responses ADD COLUMN transcript_excerpt_id uuid;
ALTER TABLE form_responses ADD CONSTRAINT form_transcript_excerpt_patient FOREIGN KEY(transcript_excerpt_id,client_id,organization_id) REFERENCES transcript_excerpts(id,client_id,organization_id);
