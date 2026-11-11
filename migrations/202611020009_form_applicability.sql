-- Live organization-authored applicability. Not a legal or clinical requirement.
-- Do not copy these columns into existing form_responses.template_snapshot rows.
ALTER TABLE templates
  ADD COLUMN applies_to_stage text NOT NULL DEFAULT 'any',
  ADD COLUMN applies_to_care_level text NOT NULL DEFAULT 'any',
  ADD COLUMN required_for_stage boolean NOT NULL DEFAULT false;

ALTER TABLE templates
  ADD CONSTRAINT templates_applies_to_stage_check
  CHECK (applies_to_stage IN ('any', 'intake', 'active', 'discharged'));

ALTER TABLE templates
  ADD CONSTRAINT templates_applies_to_care_level_check
  CHECK (applies_to_care_level IN ('any', 'outpatient', 'iop', 'php'));

COMMENT ON COLUMN templates.applies_to_stage IS 'Organization-authored stage match: any, intake, active, or discharged. Not a legal or clinical rule.';
COMMENT ON COLUMN templates.applies_to_care_level IS 'Organization-authored care-level match: any, outpatient, iop, or php. Not a legal or clinical rule.';
COMMENT ON COLUMN templates.required_for_stage IS 'When true, a matching active template is organization-required. Otherwise it is suggested. Not a legal or clinical determination.';
