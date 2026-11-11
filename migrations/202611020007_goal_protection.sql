CREATE OR REPLACE FUNCTION protect_goal_definition() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE plan_status text;
BEGIN
 IF TG_OP<>'INSERT' THEN
  SELECT status INTO plan_status FROM treatment_plans WHERE id=OLD.plan_id FOR UPDATE;
  IF plan_status<>'draft' THEN RAISE EXCEPTION 'Goals on signed plans are immutable'; END IF;
 END IF;
 IF TG_OP<>'DELETE' THEN
  SELECT status INTO plan_status FROM treatment_plans WHERE id=NEW.plan_id FOR UPDATE;
  IF plan_status<>'draft' THEN RAISE EXCEPTION 'New goals require a draft plan'; END IF;
  RETURN NEW;
 END IF;
 RETURN OLD;
END $$;
