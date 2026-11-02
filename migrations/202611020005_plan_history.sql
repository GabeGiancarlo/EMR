CREATE FUNCTION protect_attested_plan() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF OLD.signed_at IS NOT NULL THEN
  IF TG_OP='DELETE' THEN RAISE EXCEPTION 'Attested treatment plans cannot be deleted'; END IF;
  IF (to_jsonb(NEW)-'status'-'updated_at'-'completion_reason') IS DISTINCT FROM (to_jsonb(OLD)-'status'-'updated_at'-'completion_reason') THEN RAISE EXCEPTION 'Attested plan definitions are immutable'; END IF;
  IF NOT(OLD.status='active' AND NEW.status IN ('completed','superseded')) THEN RAISE EXCEPTION 'Invalid attested plan transition'; END IF;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER immutable_attested_plan BEFORE UPDATE OR DELETE ON treatment_plans FOR EACH ROW EXECUTE FUNCTION protect_attested_plan();
CREATE FUNCTION protect_goal_definition() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE plan_status text;
BEGIN
 SELECT status INTO plan_status FROM treatment_plans WHERE id=COALESCE(NEW.plan_id,OLD.plan_id) FOR UPDATE;
 IF plan_status<>'draft' THEN RAISE EXCEPTION 'Goals on signed plans are immutable'; END IF;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER immutable_signed_goal BEFORE INSERT OR UPDATE OR DELETE ON goals FOR EACH ROW EXECUTE FUNCTION protect_goal_definition();
