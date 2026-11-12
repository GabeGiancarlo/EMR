-- Visit routes refuse a participant who has caused too many events in ten minutes
-- (telehealth::event_budget). The check counts one participant's recent events on
-- every join, so it needs an index; the table had none beyond its primary key.
CREATE INDEX telehealth_events_participant_recent ON telehealth_events (participant_id, occurred_at);
