-- Applied transcript migrations remain immutable; strengthen the pointer rule separately.
CREATE OR REPLACE FUNCTION protect_transcript_source() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'Transcript sources cannot be deleted'; END IF;
 IF (to_jsonb(NEW)-'current_version_id') IS DISTINCT FROM (to_jsonb(OLD)-'current_version_id') THEN
   RAISE EXCEPTION 'Recognized source is immutable';
 END IF;
 IF NEW.current_version_id IS DISTINCT FROM OLD.current_version_id AND NOT EXISTS (
   SELECT 1 FROM transcript_versions newer JOIN transcript_versions previous
     ON previous.id=OLD.current_version_id AND previous.transcript_id=OLD.id
   WHERE newer.id=NEW.current_version_id AND newer.transcript_id=OLD.id
     AND newer.parent_id=previous.id AND newer.number=previous.number+1
 ) THEN
   RAISE EXCEPTION 'Transcript history must advance to the next child version';
 END IF;
 RETURN NEW;
END; $$;
