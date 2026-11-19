import test from "node:test";
import assert from "node:assert/strict";
import { mayEditResponse, savedFormResponse } from "./savedFormResponse.ts";
import type { Row } from "./api";

const actor = { id: "clinician", role: "clinician" };
const draft: Row = { id: "saved", client_id: "client", episode_id: "episode", template_id: "template",
  author_id: actor.id, status: "draft", updated_at: "2026-11-18T12:00:00Z" };
const select = (rows: Row[]) => savedFormResponse(rows, "client", "episode", "template", actor);

test("resume matches the patient, episode and template even when there are newer responses", () => {
  const other = { ...draft, id: "newer", updated_at: "2026-11-19T12:00:00Z" };
  assert.equal(select([draft, { ...other, client_id: "other" }, { ...other, episode_id: null },
    { ...other, template_id: "other" }, { ...other, episode_id: "other" }, { ...other, status: "signed" }]), draft);
  assert.equal(select([{ ...draft, status: "signed" }]), undefined);
});

test("choose the latest editable draft ahead of reviewed responses or another clinician's draft", () => {
  const newer = { ...draft, id: "updated", updated_at: "2026-11-18T12:00:01Z" };
  const latest = { ...newer, updated_at: "2026-11-19T12:00:00Z" };
  assert.equal(select([{ ...latest, author_id: "other" }, { ...latest, status: "reviewed" }, draft, newer]), newer);
  assert.equal(mayEditResponse(draft, { id: "other", role: "clinician" }), false);
  assert.equal(mayEditResponse(draft, { id: "lead", role: "clinical_lead" }), true);
  assert.equal(mayEditResponse({ ...draft, status: "signed" }, { id: "admin", role: "admin" }), false);
});
