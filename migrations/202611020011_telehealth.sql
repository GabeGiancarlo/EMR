CREATE TABLE telehealth_rooms (
 id uuid PRIMARY KEY,
 organization_id uuid NOT NULL REFERENCES organizations(id),
 session_id uuid,
 group_id uuid,
 created_by uuid NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(),
 expires_at timestamptz NOT NULL,
 ended_at timestamptz,
 ended_by uuid,
 board jsonb NOT NULL DEFAULT '[]',
 CHECK ((session_id IS NOT NULL)::int + (group_id IS NOT NULL)::int = 1),
 CHECK (expires_at > created_at),
 UNIQUE(id,organization_id),
 FOREIGN KEY(session_id,organization_id) REFERENCES sessions(id,organization_id),
 FOREIGN KEY(group_id,organization_id) REFERENCES groups(id,organization_id),
 FOREIGN KEY(created_by,organization_id) REFERENCES staff(id,organization_id),
 FOREIGN KEY(ended_by,organization_id) REFERENCES staff(id,organization_id)
);
CREATE UNIQUE INDEX telehealth_active_session ON telehealth_rooms(session_id) WHERE ended_at IS NULL;
CREATE UNIQUE INDEX telehealth_active_group ON telehealth_rooms(group_id) WHERE ended_at IS NULL;
CREATE TABLE telehealth_participants (
 id uuid PRIMARY KEY,
 organization_id uuid NOT NULL,
 room_id uuid NOT NULL,
 staff_id uuid,
 client_id uuid,
 label text NOT NULL CHECK(length(label) BETWEEN 1 AND 100),
 token_hash text NOT NULL UNIQUE,
 token_expires_at timestamptz NOT NULL,
 state text NOT NULL DEFAULT 'invited' CHECK(state IN ('invited','waiting','admitted','left','removed')),
 connection_id uuid,
 last_seen_at timestamptz,
 created_at timestamptz NOT NULL DEFAULT now(),
 CHECK ((staff_id IS NOT NULL)::int + (client_id IS NOT NULL)::int = 1),
 UNIQUE(id,room_id), UNIQUE(room_id,client_id), UNIQUE(room_id,staff_id),
 FOREIGN KEY(room_id,organization_id) REFERENCES telehealth_rooms(id,organization_id),
 FOREIGN KEY(staff_id,organization_id) REFERENCES staff(id,organization_id),
 FOREIGN KEY(client_id,organization_id) REFERENCES clients(id,organization_id)
);
CREATE TABLE telehealth_events (
 id uuid PRIMARY KEY,
 organization_id uuid NOT NULL,
 room_id uuid NOT NULL,
 participant_id uuid,
 actor_id uuid,
 action text NOT NULL,
 occurred_at timestamptz NOT NULL DEFAULT now(),
 FOREIGN KEY(room_id,organization_id) REFERENCES telehealth_rooms(id,organization_id),
 FOREIGN KEY(participant_id,room_id) REFERENCES telehealth_participants(id,room_id),
 FOREIGN KEY(actor_id,organization_id) REFERENCES staff(id,organization_id)
);
CREATE TRIGGER immutable_telehealth_events BEFORE UPDATE OR DELETE ON telehealth_events FOR EACH ROW EXECUTE FUNCTION protect_append_only();
-- Transport metadata only; expires quickly and is never part of chart/audit exports.
CREATE TABLE telehealth_signals (
 id bigserial PRIMARY KEY,
 room_id uuid NOT NULL REFERENCES telehealth_rooms(id),
 from_id uuid NOT NULL,
 to_id uuid NOT NULL,
 from_connection uuid NOT NULL,
 to_connection uuid NOT NULL,
 payload jsonb NOT NULL,
 expires_at timestamptz NOT NULL DEFAULT now()+interval '2 minutes',
 FOREIGN KEY(from_id,room_id) REFERENCES telehealth_participants(id,room_id),
 FOREIGN KEY(to_id,room_id) REFERENCES telehealth_participants(id,room_id),
 CHECK(octet_length(payload::text)<=100000)
);
CREATE INDEX telehealth_signal_delivery ON telehealth_signals(room_id,to_id,to_connection,id);
CREATE INDEX telehealth_signal_expiry ON telehealth_signals(expires_at);
