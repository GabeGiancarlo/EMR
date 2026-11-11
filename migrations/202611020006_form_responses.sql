CREATE TABLE form_responses (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL REFERENCES organizations(id), client_id uuid NOT NULL, episode_id uuid, session_id uuid, template_id uuid NOT NULL, author_id uuid NOT NULL,
 template_snapshot jsonb NOT NULL CHECK(jsonb_typeof(template_snapshot)='object'), answers jsonb NOT NULL DEFAULT '{}' CHECK(jsonb_typeof(answers)='object'),
 source_type text NOT NULL DEFAULT 'manual' CHECK(source_type IN ('manual','transcript')), source_excerpt text NOT NULL DEFAULT '', field_provenance jsonb NOT NULL DEFAULT '{}' CHECK(jsonb_typeof(field_provenance)='object'),
 status text NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','reviewed','signed')), reviewer_id uuid, reviewed_at timestamptz, signed_by uuid, signed_at timestamptz, amends_response_id uuid, amendment_reason text NOT NULL DEFAULT '',
 created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(), UNIQUE(id,organization_id), UNIQUE(id,client_id,organization_id),
 FOREIGN KEY(client_id,organization_id) REFERENCES clients(id,organization_id), FOREIGN KEY(episode_id,client_id,organization_id) REFERENCES episodes(id,client_id,organization_id),
 FOREIGN KEY(session_id,client_id,organization_id) REFERENCES sessions(id,client_id,organization_id), FOREIGN KEY(template_id,organization_id) REFERENCES templates(id,organization_id),
 FOREIGN KEY(author_id,organization_id) REFERENCES staff(id,organization_id), FOREIGN KEY(reviewer_id,organization_id) REFERENCES staff(id,organization_id), FOREIGN KEY(signed_by,organization_id) REFERENCES staff(id,organization_id), FOREIGN KEY(amends_response_id,organization_id) REFERENCES form_responses(id,organization_id),
 CHECK(status<>'signed' OR (signed_at IS NOT NULL AND signed_by IS NOT NULL AND reviewed_at IS NOT NULL AND reviewer_id IS NOT NULL)), CHECK(status<>'reviewed' OR (reviewed_at IS NOT NULL AND reviewer_id IS NOT NULL))
);
CREATE TRIGGER immutable_signed_response BEFORE UPDATE OR DELETE ON form_responses FOR EACH ROW EXECUTE FUNCTION protect_signed_note();
ALTER TABLE notes ADD COLUMN form_response_id uuid;
ALTER TABLE notes ADD CONSTRAINT note_response_org_client FOREIGN KEY(form_response_id,client_id,organization_id) REFERENCES form_responses(id,client_id,organization_id);
CREATE UNIQUE INDEX one_note_per_signed_response ON notes(form_response_id) WHERE form_response_id IS NOT NULL;
ALTER TABLE exceptions DROP CONSTRAINT exceptions_resource_check;
ALTER TABLE exceptions ADD CONSTRAINT exceptions_resource_check CHECK(resource IN ('clients','episodes','sessions','notes','tasks','policies','authorizations','claims','payments','templates','consents','treatment_plans','goals','plan_reviews','groups','group_attendance','form_responses'));
CREATE INDEX responses_client ON form_responses(organization_id,client_id,status);
