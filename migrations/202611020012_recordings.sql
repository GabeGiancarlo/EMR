CREATE TABLE recording_policies (
 id uuid PRIMARY KEY,
 organization_id uuid NOT NULL REFERENCES organizations(id),
 title text NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 120),
 purpose text NOT NULL CHECK(length(trim(purpose)) BETWEEN 10 AND 2000),
 retention_days integer NOT NULL CHECK(retention_days BETWEEN 1 AND 3650),
 allow_video boolean NOT NULL DEFAULT false,
 active boolean NOT NULL DEFAULT true,
 created_by uuid NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(),
 UNIQUE(id,organization_id),
 FOREIGN KEY(created_by,organization_id) REFERENCES staff(id,organization_id)
);
CREATE TABLE recordings (
 id uuid PRIMARY KEY,
 organization_id uuid NOT NULL,
 room_id uuid NOT NULL,
 policy_id uuid NOT NULL,
 policy_snapshot jsonb NOT NULL,
 mode text NOT NULL CHECK(mode IN ('audio','video')),
 state text NOT NULL DEFAULT 'requested' CHECK(state IN ('requested','recording','paused','ended','declined','interrupted')),
 reason text NOT NULL DEFAULT '',
 created_by uuid NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(),
 started_at timestamptz,
 stopped_at timestamptz,
 retain_until timestamptz NOT NULL,
 UNIQUE(id,organization_id), UNIQUE(id,room_id),
 FOREIGN KEY(room_id,organization_id) REFERENCES telehealth_rooms(id,organization_id),
 FOREIGN KEY(policy_id,organization_id) REFERENCES recording_policies(id,organization_id),
 FOREIGN KEY(created_by,organization_id) REFERENCES staff(id,organization_id)
);
CREATE UNIQUE INDEX recording_active_room ON recordings(room_id) WHERE state IN ('requested','recording','paused');
CREATE TABLE recording_participants (
 organization_id uuid NOT NULL,
 recording_id uuid NOT NULL,
 room_id uuid NOT NULL,
 participant_id uuid NOT NULL,
 consent_id uuid,
 consent_snapshot jsonb,
 decision text NOT NULL DEFAULT 'pending' CHECK(decision IN ('pending','accepted','declined','withdrawn')),
 decided_at timestamptz,
 PRIMARY KEY(recording_id,participant_id),
 FOREIGN KEY(recording_id,room_id) REFERENCES recordings(id,room_id),
 FOREIGN KEY(participant_id,room_id) REFERENCES telehealth_participants(id,room_id),
 FOREIGN KEY(recording_id,organization_id) REFERENCES recordings(id,organization_id),
 FOREIGN KEY(consent_id,organization_id) REFERENCES consents(id,organization_id)
);
CREATE TABLE recording_tracks (
 id uuid PRIMARY KEY,
 organization_id uuid NOT NULL,
 recording_id uuid NOT NULL,
 participant_id uuid NOT NULL,
 connection_id uuid NOT NULL,
 mime_type text NOT NULL CHECK(mime_type IN ('audio/webm;codecs=opus','audio/webm','video/webm;codecs=vp8,opus','video/webm;codecs=vp9,opus','video/webm')),
 state text NOT NULL DEFAULT 'open' CHECK(state IN ('open','complete','incomplete')),
 token_hash text NOT NULL UNIQUE,
 created_at timestamptz NOT NULL DEFAULT now(),
 finished_at timestamptz,
 expected_chunks integer CHECK(expected_chunks BETWEEN 0 AND 3600),
 byte_count bigint NOT NULL DEFAULT 0 CHECK(byte_count BETWEEN 0 AND 268435456),
 UNIQUE(id,organization_id),
 FOREIGN KEY(recording_id,organization_id) REFERENCES recordings(id,organization_id),
 FOREIGN KEY(recording_id,participant_id) REFERENCES recording_participants(recording_id,participant_id)
);
CREATE UNIQUE INDEX recording_one_open_track ON recording_tracks(recording_id,participant_id) WHERE state='open';
CREATE TABLE recording_chunks (
 track_id uuid NOT NULL,
 sequence integer NOT NULL CHECK(sequence BETWEEN 0 AND 3599),
 key_id text NOT NULL,
 nonce bytea NOT NULL CHECK(octet_length(nonce)=12),
 ciphertext bytea NOT NULL CHECK(octet_length(ciphertext) BETWEEN 17 AND 8388624),
 plain_sha256 text NOT NULL CHECK(length(plain_sha256)=64),
 plain_length integer NOT NULL CHECK(plain_length BETWEEN 1 AND 8388608),
 created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY(track_id,sequence),
 FOREIGN KEY(track_id) REFERENCES recording_tracks(id)
);
CREATE TRIGGER immutable_recording_chunks BEFORE UPDATE OR DELETE ON recording_chunks FOR EACH ROW EXECUTE FUNCTION protect_append_only();
CREATE INDEX recording_room_history ON recordings(room_id,created_at);
CREATE FUNCTION protect_recording_policy() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN
   RAISE EXCEPTION 'Recording policies must be retired, not deleted';
 END IF;
 IF (to_jsonb(NEW)-'active') IS DISTINCT FROM (to_jsonb(OLD)-'active') OR (NOT OLD.active AND NEW.active) THEN
   RAISE EXCEPTION 'Create a new recording policy version instead of changing an existing policy';
 END IF;
 RETURN NEW;
END;
$$;
CREATE TRIGGER immutable_recording_policy BEFORE UPDATE OR DELETE ON recording_policies FOR EACH ROW EXECUTE FUNCTION protect_recording_policy();
