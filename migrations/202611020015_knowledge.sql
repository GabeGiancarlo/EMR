CREATE TABLE knowledge_documents (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL REFERENCES organizations(id), client_id uuid NOT NULL,
 source_kind text NOT NULL CHECK(source_kind IN ('note','form_response','treatment_plan','plan_review','transcript_excerpt')),
 source_id uuid NOT NULL, source_version uuid NOT NULL, source_hash text NOT NULL CHECK(source_hash ~ '^[a-f0-9]{64}$'),
 title text NOT NULL, body text NOT NULL CHECK(octet_length(body) BETWEEN 1 AND 200000),
 source_date timestamptz NOT NULL, episode_id uuid, session_id uuid,
 classification text NOT NULL DEFAULT 'clinical' CHECK(classification='clinical'),
 indexed_at timestamptz NOT NULL DEFAULT now(), UNIQUE(id,client_id,organization_id),
 FOREIGN KEY(client_id,organization_id) REFERENCES clients(id,organization_id),
 FOREIGN KEY(episode_id,client_id,organization_id) REFERENCES episodes(id,client_id,organization_id),
 FOREIGN KEY(session_id,client_id,organization_id) REFERENCES sessions(id,client_id,organization_id)
);
CREATE INDEX knowledge_patient_sources ON knowledge_documents(organization_id,client_id,source_date DESC);
CREATE TABLE knowledge_chunks (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL, client_id uuid NOT NULL, document_id uuid NOT NULL,
 ordinal integer NOT NULL CHECK(ordinal>=0), text text NOT NULL CHECK(octet_length(text) BETWEEN 1 AND 8000),
 text_search tsvector GENERATED ALWAYS AS (to_tsvector('english',text)) STORED,
 UNIQUE(document_id,ordinal),
 FOREIGN KEY(document_id,client_id,organization_id) REFERENCES knowledge_documents(id,client_id,organization_id) ON DELETE CASCADE
);
CREATE INDEX knowledge_patient_chunks ON knowledge_chunks(organization_id,client_id);
CREATE INDEX knowledge_english_search ON knowledge_chunks USING gin(text_search);
CREATE FUNCTION invalidate_patient_knowledge() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE record_value jsonb; patient uuid; org uuid;
BEGIN
 record_value := CASE WHEN TG_OP='DELETE' THEN to_jsonb(OLD) ELSE to_jsonb(NEW) END;
 org := (record_value->>'organization_id')::uuid;
 -- Group processing consent can affect excerpts for every participant. Transcript
 -- versions have the same shared boundary, so invalidate that organization's index.
 IF TG_TABLE_NAME IN ('consents','transcripts','transcript_versions') THEN patient := NULL;
 ELSIF TG_TABLE_NAME='clients' THEN patient := (record_value->>'id')::uuid;
 ELSE patient := (record_value->>'client_id')::uuid;
 END IF;
 DELETE FROM knowledge_documents WHERE organization_id=org AND (patient IS NULL OR client_id=patient);
 IF TG_OP='DELETE' THEN RETURN OLD; END IF; RETURN NEW;
END; $$;
DO $$ DECLARE source_table text; BEGIN
 FOREACH source_table IN ARRAY ARRAY['notes','form_responses','treatment_plans','goals','plan_reviews','transcripts','transcript_versions','transcript_excerpts','consents','sessions','episodes','clients'] LOOP
   EXECUTE format('CREATE TRIGGER invalidate_knowledge AFTER INSERT OR UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION invalidate_patient_knowledge()',source_table);
 END LOOP;
END $$;
