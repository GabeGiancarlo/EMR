-- One originating note per signed form; amendments retain the same form provenance.
DROP INDEX one_note_per_signed_response;
CREATE UNIQUE INDEX one_note_per_signed_response ON notes(form_response_id)
 WHERE form_response_id IS NOT NULL AND amends_note_id IS NULL;
