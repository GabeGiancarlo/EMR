CREATE SEQUENCE patient_mrn_sequence;
ALTER TABLE clients ADD COLUMN mrn text NOT NULL DEFAULT ('NX' || lpad(nextval('patient_mrn_sequence')::text,10,'0'));
ALTER TABLE clients ADD CONSTRAINT client_mrn_unique UNIQUE(organization_id,mrn);
ALTER TABLE clients
 ADD COLUMN pronouns text NOT NULL DEFAULT '' CHECK(length(pronouns)<=80),
 ADD COLUMN preferred_language text NOT NULL DEFAULT '' CHECK(length(preferred_language)<=80),
 ADD COLUMN address_line1 text NOT NULL DEFAULT '' CHECK(length(address_line1)<=200),
 ADD COLUMN address_line2 text NOT NULL DEFAULT '' CHECK(length(address_line2)<=200),
 ADD COLUMN city text NOT NULL DEFAULT '' CHECK(length(city)<=100),
 ADD COLUMN region text NOT NULL DEFAULT '' CHECK(length(region)<=100),
 ADD COLUMN postal_code text NOT NULL DEFAULT '' CHECK(length(postal_code)<=30),
 ADD COLUMN country text NOT NULL DEFAULT '' CHECK(length(country)<=80);
CREATE FUNCTION preserve_patient_mrn() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.mrn IS DISTINCT FROM OLD.mrn THEN RAISE EXCEPTION 'Medical record numbers are immutable'; END IF;
 RETURN NEW;
END; $$;
CREATE TRIGGER immutable_patient_mrn BEFORE UPDATE ON clients FOR EACH ROW EXECUTE FUNCTION preserve_patient_mrn();
CREATE INDEX patient_duplicate_birthdate ON clients(organization_id,birth_date);
CREATE TABLE patient_duplicate_reviews (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL, client_id uuid NOT NULL,
 actor_id uuid NOT NULL, candidate_ids jsonb NOT NULL CHECK(jsonb_typeof(candidate_ids)='array' AND jsonb_array_length(candidate_ids) BETWEEN 1 AND 1000),
 reason text NOT NULL CHECK(length(trim(reason)) BETWEEN 10 AND 2000), created_at timestamptz NOT NULL DEFAULT now(),
 FOREIGN KEY(client_id,organization_id) REFERENCES clients(id,organization_id),
 FOREIGN KEY(actor_id,organization_id) REFERENCES staff(id,organization_id)
);
CREATE TRIGGER immutable_duplicate_review BEFORE UPDATE OR DELETE ON patient_duplicate_reviews FOR EACH ROW EXECUTE FUNCTION protect_append_only();
