-- Existing appointments are preserved. Application checks govern new reservations.
CREATE TABLE staff_unavailability (
 id uuid PRIMARY KEY,
 organization_id uuid NOT NULL REFERENCES organizations(id),
 staff_id uuid NOT NULL,
 starts_at timestamptz NOT NULL,
 ends_at timestamptz NOT NULL CHECK(ends_at > starts_at),
 reason text NOT NULL CHECK(length(trim(reason)) BETWEEN 1 AND 200),
 active boolean NOT NULL DEFAULT true,
 created_by uuid NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(),
 updated_at timestamptz NOT NULL DEFAULT now(),
 UNIQUE(id,organization_id),
 FOREIGN KEY(staff_id,organization_id) REFERENCES staff(id,organization_id),
 FOREIGN KEY(created_by,organization_id) REFERENCES staff(id,organization_id)
);
CREATE INDEX staff_unavailability_reservations ON staff_unavailability(organization_id,staff_id,starts_at,ends_at) WHERE active;
CREATE INDEX individual_schedule_reservations ON sessions(organization_id,clinician_id,starts_at,ends_at) WHERE group_id IS NULL AND status IN ('scheduled','completed');
CREATE INDEX client_schedule_reservations ON sessions(organization_id,client_id,starts_at,ends_at) WHERE status IN ('scheduled','completed');
CREATE INDEX group_schedule_reservations ON groups(organization_id,clinician_id,starts_at,ends_at) WHERE status IN ('scheduled','completed');
