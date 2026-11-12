-- Staff sign-in identity. Hosted sign-in resolves a verified email to a staff member
-- by lower(email) across every organization, so one address must name one active
-- staff member in the whole table. The older UNIQUE(organization_id,email) is kept:
-- it is per organization and case-sensitive, and does not guarantee that.
--
-- This migration changes no row. If active staff already share an address, it stops
-- and names them; SQLx applies it in a transaction, so nothing is left half done.
-- Inactive staff are outside the rule, so a former member's address can be reused.
DO $$
DECLARE ids text;
BEGIN
 SELECT string_agg(staff_ids, '; ') INTO ids FROM (
   SELECT string_agg(id::text, ', ' ORDER BY id) AS staff_ids
   FROM staff WHERE active
   GROUP BY lower(email)
   HAVING count(*) > 1
   ORDER BY min(id::text) LIMIT 20
 ) collisions;
 IF ids IS NOT NULL THEN
   RAISE EXCEPTION 'Active staff who share one email address (compared without regard to case) block migration 022; no records changed. Sign-in cannot tell them apart. Give each person a distinct address, or deactivate the duplicate, then start again. Staff IDs: %', ids;
 END IF;
END $$;

CREATE UNIQUE INDEX staff_active_email_identity ON staff (lower(email)) WHERE active;
