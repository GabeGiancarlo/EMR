-- Financial identity v1: explicit boundary whitespace, ICU root full uppercase,
-- then byte-exact equality. Preserve original rows and clinical snapshots.
-- ICU support is required. Recheck compatibility and rebuild these indexes when
-- upgrading ICU; refreshing collation metadata alone does not rebuild indexes.
CREATE FUNCTION nexus_service_code_v1(input text) RETURNS text
LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE AS $$
 SELECT NULLIF(upper(btrim(input,
   U&'\0009\000A\000B\000C\000D\0020\0085\00A0\1680\2000\2001\2002\2003\2004\2005\2006\2007\2008\2009\200A\2028\2029\202F\205F\3000'
 ) COLLATE pg_catalog."unicode"), '') COLLATE "C"
$$;

DO $$
DECLARE ids text;
BEGIN
 SELECT string_agg(id::text, ', ') INTO ids FROM
   (SELECT id FROM claims WHERE status <> 'voided'
      AND nexus_service_code_v1(service_code) IS NULL ORDER BY id LIMIT 20) bad;
 IF ids IS NOT NULL THEN
   RAISE EXCEPTION 'Blank live claim service codes block migration 021. Correct these records through the application first: %', ids;
 END IF;
 SELECT string_agg(id::text, ', ') INTO ids FROM
   (SELECT id FROM authorizations WHERE nexus_service_code_v1(service_code) IS NULL ORDER BY id LIMIT 20) bad;
 IF ids IS NOT NULL THEN
   RAISE EXCEPTION 'Blank authorization service codes block migration 021. Correct these records first: %', ids;
 END IF;
 SELECT string_agg(id::text, ', ') INTO ids FROM
   (SELECT id FROM program_services WHERE nexus_service_code_v1(service_code) IS NULL ORDER BY id LIMIT 20) bad;
 IF ids IS NOT NULL THEN
   RAISE EXCEPTION 'Blank catalog service codes block migration 021. Review these records without rewriting historical snapshots: %', ids;
 END IF;
 SELECT string_agg(claim_ids, '; ') INTO ids FROM (
   SELECT string_agg(id::text, ', ' ORDER BY id) AS claim_ids
   FROM claims WHERE status <> 'voided'
   GROUP BY organization_id, session_id, (nexus_service_code_v1(service_code) COLLATE "C")
   HAVING count(*) > 1 LIMIT 20
 ) collisions;
 IF ids IS NOT NULL THEN
   RAISE EXCEPTION 'Canonical live claim collisions block migration 021; no records changed. Resolve the economic duplicates with audited corrections before retrying. Claim IDs: %', ids;
 END IF;
 SELECT string_agg(service_ids, '; ') INTO ids FROM (
   SELECT string_agg(id::text, ', ' ORDER BY id) AS service_ids
   FROM program_services
   GROUP BY program_id, code_system, (nexus_service_code_v1(service_code) COLLATE "C")
   HAVING count(*) > 1 LIMIT 20
 ) collisions;
 IF ids IS NOT NULL THEN
   RAISE EXCEPTION 'Canonical catalog collisions block migration 021; no records changed. Service IDs: %', ids;
 END IF;
 SELECT string_agg(id::text, ', ') INTO ids FROM (
   SELECT c.id FROM claims c JOIN sessions s
     ON s.id=c.session_id AND s.organization_id=c.organization_id
   WHERE c.status <> 'voided' AND s.program_service_id IS NOT NULL
     AND ((nexus_service_code_v1(s.service_snapshot->>'service_code') COLLATE "C") IS NULL
       OR (nexus_service_code_v1(c.service_code) COLLATE "C") IS DISTINCT FROM
          (nexus_service_code_v1(s.service_snapshot->>'service_code') COLLATE "C"))
   ORDER BY c.id LIMIT 20
 ) bad;
 IF ids IS NOT NULL THEN
   RAISE EXCEPTION 'Claim/snapshot code mismatches block migration 021; no evidence changed. Claim IDs: %', ids;
 END IF;
END $$;

-- Build stronger guards before removing their predecessors. SQLx applies this
-- forward-only migration transactionally; any preflight/index failure rolls back.
CREATE UNIQUE INDEX one_live_service_claim_v1 ON claims
 (organization_id, session_id, (nexus_service_code_v1(service_code) COLLATE "C"))
 WHERE status <> 'voided';
CREATE UNIQUE INDEX unique_program_service_code_v1 ON program_services
 (program_id, code_system, (nexus_service_code_v1(service_code) COLLATE "C"));
ALTER TABLE claims ADD CONSTRAINT live_claim_code_present
 CHECK(status='voided' OR nexus_service_code_v1(service_code) IS NOT NULL);
ALTER TABLE authorizations ADD CONSTRAINT authorization_code_present
 CHECK(nexus_service_code_v1(service_code) IS NOT NULL);
ALTER TABLE program_services ADD CONSTRAINT catalog_service_code_present
 CHECK(nexus_service_code_v1(service_code) IS NOT NULL);
DROP INDEX one_live_service_claim;
ALTER INDEX one_live_service_claim_v1 RENAME TO one_live_service_claim;
DROP INDEX unique_program_service_code;
ALTER INDEX unique_program_service_code_v1 RENAME TO unique_program_service_code;
