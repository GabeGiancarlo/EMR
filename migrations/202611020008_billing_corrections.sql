ALTER TABLE claims DROP CONSTRAINT claims_status_check;
ALTER TABLE claims ADD CONSTRAINT claims_status_check CHECK(status IN ('draft','ready','submitted','denied','paid','voided'));
ALTER TABLE claims ADD COLUMN void_reason text NOT NULL DEFAULT '';
ALTER TABLE claims DROP CONSTRAINT claims_organization_id_session_id_service_code_key;
CREATE UNIQUE INDEX one_live_service_claim ON claims(organization_id,session_id,service_code) WHERE status<>'voided';
ALTER TABLE payments ADD CONSTRAINT payments_claim_org_identity UNIQUE(id,claim_id,organization_id);
CREATE TABLE payment_reversals (
 id uuid PRIMARY KEY, organization_id uuid NOT NULL REFERENCES organizations(id), payment_id uuid NOT NULL UNIQUE, claim_id uuid NOT NULL,
 amount_cents bigint NOT NULL CHECK(amount_cents>0), reversed_on date NOT NULL, reason text NOT NULL CHECK(length(trim(reason))>=10), actor_id uuid NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(), UNIQUE(id,organization_id),
 FOREIGN KEY(payment_id,claim_id,organization_id) REFERENCES payments(id,claim_id,organization_id), FOREIGN KEY(actor_id,organization_id) REFERENCES staff(id,organization_id)
);
CREATE TRIGGER immutable_payment_reversals BEFORE UPDATE OR DELETE ON payment_reversals FOR EACH ROW EXECUTE FUNCTION protect_append_only();
CREATE INDEX reversals_claim ON payment_reversals(organization_id,claim_id);
CREATE INDEX reversals_booked ON payment_reversals(organization_id,reversed_on);
