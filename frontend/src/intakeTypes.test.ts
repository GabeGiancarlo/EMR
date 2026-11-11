import test from "node:test";
import assert from "node:assert/strict";
import { allowedIntakeTransitions, parseIntake, transitionBlockers } from "./intakeTypes.ts";

const caseRow = { id: "case", client_id: "patient", episode_id: "episode", facility_id: "facility", level_of_care: "iop", starts_on: "2026-11-02", owner_id: "owner", referral_type: "self", referrer_name: "", referrer_contact: "", referral_reason: "Requested care", received_on: "2026-11-02", status: "screening", disposition_reason: "Reviewed referral", created_at: "2026-11-02T12:00:00Z", updated_at: "2026-11-02T12:00:00Z", checklist: [{ code: "identity_confirmation", title: "Identity", required: true, passed: false, detail: "Review identity" }, { code: "admission_start_date", title: "Start date", required: false, passed: false, detail: "Future date" }, { code: "open_tasks", title: "Tasks", required: false, passed: false, detail: "Informational" }], events: [] };
const fixture = () => ({ client_id: "patient", cases: [structuredClone(caseRow)], identity_checks: [] });
test("intake parsing refuses a different patient, malformed readiness, and unknown state", () => {
  assert.throws(() => parseIntake(fixture(), "another"), /different patient/);
  const wrong = fixture(); wrong.cases[0].client_id = "another"; assert.throws(() => parseIntake(wrong, "patient"), /different patient/);
  const checks = fixture(); (checks.cases[0].checklist[0] as Record<string, unknown>).passed = "false"; assert.throws(() => parseIntake(checks, "patient"), /checks could not be read/);
  const state = fixture(); state.cases[0].status = "complete"; assert.throws(() => parseIntake(state, "patient"), /status is not recognized/);
  const duplicate = fixture(); duplicate.cases.push(structuredClone(caseRow)); assert.throws(() => parseIntake(duplicate, "patient"), /repeated cases/);
});
test("ready and admission use different requirements while informational items never block", () => {
  const row = parseIntake(fixture(), "patient").cases[0];
  assert.deepEqual(transitionBlockers(row, "ready").map(i => i.code), ["identity_confirmation"]);
  assert.deepEqual(transitionBlockers(row, "admitted").map(i => i.code), ["identity_confirmation", "admission_start_date"]);
  assert.deepEqual(transitionBlockers(row, "withdrawn"), []);
  row.checklist[0].passed = true;
  assert.equal(transitionBlockers(row, "ready").length, 0);
  assert.equal(transitionBlockers(row, "admitted").length, 1);
});
test("support cannot admit, terminal cases cannot transition, and billing has no intake actions", () => {
  assert.ok(!allowedIntakeTransitions("ready", "support").includes("admitted"));
  assert.ok(allowedIntakeTransitions("ready", "clinician").includes("admitted"));
  for (const state of ["admitted", "declined", "withdrawn"] as const) assert.deepEqual(allowedIntakeTransitions(state, "admin"), []);
  assert.deepEqual(allowedIntakeTransitions("screening", "billing"), []);
  assert.ok(!allowedIntakeTransitions("open", "admin").includes("admitted"));
});
test("saved decisions retain historical checks and reject another episode's evidence",()=>{
  const evidence={id:"decision",event_id:"event",client_id:"patient",intake_case_id:"case",episode_id:"episode",program_version:1,program_snapshot:{name:"Prior program rules",capacity:1},created_at:"2026-11-02T12:00:00Z",checklist:[{code:"program_active",title:"Program accepting",required:true,passed:true,detail:"Accepted at decision time"}]};
  const payload={...fixture(),cases:[{...caseRow,decision_evidence:[evidence]}]};
  const parsed=parseIntake(payload,"patient").cases[0];
  assert.equal(parsed.decision_evidence?.[0].checklist[0].passed,true);
  assert.equal(parsed.checklist[0].passed,false);
  assert.throws(()=>parseIntake({...payload,cases:[{...caseRow,decision_evidence:[{...evidence,episode_id:"other"}]}]},"patient"),/context could not be verified/);
});
