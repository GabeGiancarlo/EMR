CREATE TABLE treatment_plans (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL REFERENCES organizations(id), client_id uuid NOT NULL, episode_id uuid NOT NULL, author_id uuid NOT NULL,
 title text NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 200), presenting_needs text NOT NULL CHECK(length(trim(presenting_needs))>0), strengths text NOT NULL DEFAULT '', next_review_date date,
 status text NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','active','completed','superseded')), signed_at timestamptz, signed_by uuid, supersedes_plan_id uuid, revision_reason text NOT NULL DEFAULT '', completion_reason text NOT NULL DEFAULT '',
 created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(), UNIQUE(id,organization_id), UNIQUE(id,client_id,organization_id),
 FOREIGN KEY(client_id,organization_id) REFERENCES clients(id,organization_id), FOREIGN KEY(episode_id,client_id,organization_id) REFERENCES episodes(id,client_id,organization_id),
 FOREIGN KEY(author_id,organization_id) REFERENCES staff(id,organization_id), FOREIGN KEY(signed_by,organization_id) REFERENCES staff(id,organization_id), FOREIGN KEY(supersedes_plan_id,organization_id) REFERENCES treatment_plans(id,organization_id)
);
CREATE UNIQUE INDEX one_active_plan_per_episode ON treatment_plans(episode_id) WHERE status='active';
CREATE TABLE goals (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL REFERENCES organizations(id), client_id uuid NOT NULL, plan_id uuid NOT NULL,
 description text NOT NULL CHECK(length(trim(description))>0), measure text NOT NULL CHECK(length(trim(measure))>0), baseline text NOT NULL DEFAULT '', target text NOT NULL CHECK(length(trim(target))>0), target_date date,
 created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(), UNIQUE(id,organization_id), FOREIGN KEY(plan_id,client_id,organization_id) REFERENCES treatment_plans(id,client_id,organization_id)
);
CREATE TABLE plan_reviews (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL REFERENCES organizations(id), client_id uuid NOT NULL, plan_id uuid NOT NULL, reviewer_id uuid NOT NULL,
 summary text NOT NULL CHECK(length(trim(summary))>0), goal_progress jsonb NOT NULL CHECK(jsonb_typeof(goal_progress)='array'), goal_snapshot jsonb NOT NULL CHECK(jsonb_typeof(goal_snapshot)='array'), next_review_date date,
 created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(), UNIQUE(id,organization_id), FOREIGN KEY(plan_id,client_id,organization_id) REFERENCES treatment_plans(id,client_id,organization_id), FOREIGN KEY(reviewer_id,organization_id) REFERENCES staff(id,organization_id)
);
CREATE TRIGGER immutable_plan_reviews BEFORE UPDATE OR DELETE ON plan_reviews FOR EACH ROW EXECUTE FUNCTION protect_append_only();
CREATE TABLE groups (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL REFERENCES organizations(id), facility_id uuid NOT NULL, clinician_id uuid NOT NULL,
 title text NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 200), kind text NOT NULL CHECK(kind IN ('process','psychoeducation','skills','family','support')),
 starts_at timestamptz NOT NULL, ends_at timestamptz NOT NULL CHECK(ends_at>starts_at), capacity integer NOT NULL DEFAULT 12 CHECK(capacity BETWEEN 1 AND 100), location text NOT NULL DEFAULT '', status text NOT NULL DEFAULT 'scheduled' CHECK(status IN ('scheduled','completed','cancelled')), cancellation_reason text NOT NULL DEFAULT '',
 created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(), UNIQUE(id,organization_id), FOREIGN KEY(facility_id,organization_id) REFERENCES facilities(id,organization_id), FOREIGN KEY(clinician_id,organization_id) REFERENCES staff(id,organization_id)
);
ALTER TABLE sessions ADD COLUMN group_id uuid;
ALTER TABLE sessions ADD CONSTRAINT session_group_org FOREIGN KEY(group_id,organization_id) REFERENCES groups(id,organization_id);
CREATE TABLE group_attendance (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL REFERENCES organizations(id), group_id uuid NOT NULL, client_id uuid NOT NULL, episode_id uuid NOT NULL, session_id uuid NOT NULL,
 status text NOT NULL DEFAULT 'scheduled' CHECK(status IN ('scheduled','attended','no_show','excused')),
 created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(), UNIQUE(id,organization_id), UNIQUE(group_id,client_id), UNIQUE(session_id),
 FOREIGN KEY(group_id,organization_id) REFERENCES groups(id,organization_id), FOREIGN KEY(client_id,organization_id) REFERENCES clients(id,organization_id), FOREIGN KEY(episode_id,client_id,organization_id) REFERENCES episodes(id,client_id,organization_id), FOREIGN KEY(session_id,client_id,organization_id) REFERENCES sessions(id,client_id,organization_id)
);
ALTER TABLE exceptions DROP CONSTRAINT exceptions_resource_check;
ALTER TABLE exceptions ADD CONSTRAINT exceptions_resource_check CHECK(resource IN ('clients','episodes','sessions','notes','tasks','policies','authorizations','claims','payments','templates','consents','treatment_plans','goals','plan_reviews','groups','group_attendance'));
CREATE INDEX plans_client ON treatment_plans(organization_id,client_id,status);
CREATE INDEX goals_plan ON goals(organization_id,plan_id);
CREATE INDEX reviews_plan ON plan_reviews(organization_id,plan_id,created_at DESC);
CREATE INDEX groups_calendar ON groups(organization_id,starts_at);
CREATE INDEX attendance_group ON group_attendance(organization_id,group_id);
