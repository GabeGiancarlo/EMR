export const intakeStatuses = ["open", "screening", "ready", "admitted", "declined", "withdrawn"] as const;
export type IntakeStatus = typeof intakeStatuses[number];
export type IntakeItem = { code: string; title: string; required: boolean; passed: boolean; detail: string };
export type IntakeEvent = { id: string; from_status: IntakeStatus; to_status: IntakeStatus; reason: string; actor_id: string; created_at: string };
export type IntakeDecision = { id:string; event_id:string; program_version:number; program_snapshot:Record<string,unknown>; checklist:IntakeItem[]; created_at:string };
export type IntakeCase = { program_id?:string|null; program_name?:string; program_version?:number; episode_updated_at?:string; decision_evidence?:IntakeDecision[]; id: string; client_id: string; episode_id: string; facility_id: string; level_of_care: string; starts_on: string; owner_id: string; referral_type: string; referrer_name: string; referrer_contact: string; referral_reason: string; received_on: string; status: IntakeStatus; disposition_reason: string; created_at: string; updated_at: string; checklist: IntakeItem[]; events: IntakeEvent[] };
export type IdentityCheck = { id: string; actor_id: string; method: string; outcome: string; reason: string; current: boolean; created_at: string };
export type IntakeData = { client_id: string; cases: IntakeCase[]; identity_checks: IdentityCheck[] };

function object(input: unknown): Record<string, unknown> {
  if (!input || typeof input !== "object" || Array.isArray(input)) throw Error("Intake data could not be read. Refresh to try again.");
  return input as Record<string, unknown>;
}
function text(row: Record<string, unknown>, key: string): string {
  if (typeof row[key] !== "string") throw Error("Intake data is incomplete. Refresh to try again.");
  return row[key] as string;
}
function bool(row: Record<string, unknown>, key: string): boolean {
  if (typeof row[key] !== "boolean") throw Error("Intake checks could not be read. Refresh to try again.");
  return row[key] as boolean;
}
function array(input: unknown, limit = 10000): unknown[] {
  if (!Array.isArray(input) || input.length > limit) throw Error("Intake history could not be read in full.");
  return input;
}
function status(input: string): IntakeStatus {
  if (!intakeStatuses.includes(input as IntakeStatus)) throw Error("Intake status is not recognized. Refresh before continuing.");
  return input as IntakeStatus;
}
export function parseIntake(input: unknown, clientId: string): IntakeData {
  const root = object(input);
  if (text(root, "client_id") !== clientId) throw Error("The intake response belongs to a different patient. Refresh before continuing.");
  const cases = array(root.cases, 100).map(raw => {
    const row = object(raw);
    if (text(row, "client_id") !== clientId) throw Error("The intake case belongs to a different patient.");
    const fields = Object.fromEntries(["id", "client_id", "episode_id", "facility_id", "level_of_care", "starts_on", "owner_id", "referral_type", "referrer_name", "referrer_contact", "referral_reason", "received_on", "disposition_reason", "created_at", "updated_at"].map(k => [k, text(row, k)]));
    const checklist = array(row.checklist).map(raw => { const item = object(raw); return { code: text(item, "code"), title: text(item, "title"), detail: text(item, "detail"), required: bool(item, "required"), passed: bool(item, "passed") }; });
    const events = array(row.events).map(raw => { const e = object(raw); return { id: text(e, "id"), from_status: status(text(e, "from_status")), to_status: status(text(e, "to_status")), reason: text(e, "reason"), actor_id: text(e, "actor_id"), created_at: text(e, "created_at") }; });
    const decision_evidence=array(row.decision_evidence ?? []).map(raw=>{
      const d=object(raw);if(d.client_id!==clientId || d.episode_id!==fields.episode_id || d.intake_case_id!==fields.id || !Number.isInteger(d.program_version) || Number(d.program_version)<1) throw Error("Admission decision context could not be verified.");
      return {id:text(d,"id"),event_id:text(d,"event_id"),program_version:Number(d.program_version),program_snapshot:object(d.program_snapshot),created_at:text(d,"created_at"),checklist:array(d.checklist).map(raw=>{const i=object(raw);return {code:text(i,"code"),title:text(i,"title"),detail:text(i,"detail"),required:bool(i,"required"),passed:bool(i,"passed")};})};
    });
    return { ...fields, program_id:typeof row.program_id==="string"?row.program_id:null,program_name:typeof row.program_name==="string"?row.program_name:undefined,program_version:typeof row.program_version==="number"?row.program_version:undefined,episode_updated_at:typeof row.episode_updated_at==="string"?row.episode_updated_at:undefined,decision_evidence,status: status(text(row, "status")), checklist, events } as IntakeCase;
  });
  if (new Set(cases.map(c => c.id)).size !== cases.length) throw Error("Intake history contains repeated cases. Refresh before continuing.");
  const identity_checks = array(root.identity_checks, 100).map(raw => { const r = object(raw); return { id: text(r, "id"), actor_id: text(r, "actor_id"), method: text(r, "method"), outcome: text(r, "outcome"), reason: text(r, "reason"), current: bool(r, "current"), created_at: text(r, "created_at") }; });
  return { client_id: clientId, cases, identity_checks };
}
export const isOpenIntake = (s: IntakeStatus) => s === "open" || s === "screening" || s === "ready";
export function allowedIntakeTransitions(s: IntakeStatus, role: string): IntakeStatus[] {
  if (!["admin", "clinical_lead", "clinician", "support"].includes(role)) return [];
  const clinical = role !== "support";
  if (s === "open") return ["screening", "declined", "withdrawn"];
  if (s === "screening") return ["ready", "declined", "withdrawn"];
  if (s === "ready") return [...(clinical ? ["admitted" as const] : []), "screening", "declined", "withdrawn"];
  return [];
}
export function transitionBlockers(c: IntakeCase, to: IntakeStatus): IntakeItem[] {
  return to === "ready" || to === "admitted" ? c.checklist.filter(i => !i.passed && (i.required || (to === "admitted" && i.code.startsWith("admission_")))) : [];
}
