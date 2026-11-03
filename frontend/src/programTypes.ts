export const bounds = {
  minAge: 18,
  maxMinAge: 120,
  maxAge: 130,
  minCapacity: 1,
  maxCapacity: 10000,
  maxMinutes: 480,
  maxChargeCents: 100000000,
  reasonMin: 10,
  reasonMax: 2000,
  codeMax: 40,
  nameMax: 160,
  programLimit: 1000,
  serviceLimit: 5000,
  revisionLimit: 200,
} as const;

export const levelsOfCare = ["outpatient", "iop", "php"] as const;
export const codeSystems = ["internal", "cpt", "hcpcs"] as const;
export const sessionKinds = ["individual", "group", "assessment", "family", "case_management"] as const;
export const censusDefinition = "Active episodes only. Planned intakes do not reserve a place.";
export const catalogPath = "/api/program-catalog";

export type LevelOfCare = (typeof levelsOfCare)[number];
export type CodeSystem = (typeof codeSystems)[number];
export type SessionKind = (typeof sessionKinds)[number];

export type CareProgram = {
  id: string;
  organization_id: string;
  facility_id: string;
  code: string;
  name: string;
  level_of_care: LevelOfCare;
  min_age: number;
  max_age: number | null;
  capacity: number;
  active: boolean;
  version: number;
  created_at: string;
  updated_at: string;
  required_intake_template_ids: string[];
  required_discharge_template_ids: string[];
  active_census: number;
  available_places: number;
};

export type ProgramService = {
  id: string;
  organization_id: string;
  program_id: string;
  code_system: CodeSystem;
  service_code: string;
  name: string;
  session_kind: SessionKind;
  min_minutes: number;
  default_minutes: number;
  max_minutes: number;
  default_charge_cents: number | null;
  note_template_id: string | null;
  active: boolean;
  version: number;
  created_at: string;
  updated_at: string;
};

export type ProgramCatalog = { programs: CareProgram[]; services: ProgramService[]; can_manage: boolean };

export type Revision = {
  id: string;
  organization_id: string;
  program_id: string | null;
  program_service_id: string | null;
  version: number;
  snapshot: Record<string, unknown>;
  reason: string;
  actor_id: string;
  created_at: string;
};

export type TemplateRef = { id: string; name: string; active: boolean; organizationId?: string };
export type FacilityRef = { id: string; name: string; active: boolean };
export type ProgramDraft = {
  facility_id: string;
  code: string;
  name: string;
  level_of_care: string;
  min_age: string;
  max_age: string;
  capacity: string;
  active: boolean;
  intake_ids: string[];
  discharge_ids: string[];
  change_reason: string;
};
export type ServiceDraft = {
  program_id: string;
  code_system: string;
  service_code: string;
  name: string;
  session_kind: string;
  min_minutes: string;
  default_minutes: string;
  max_minutes: string;
  default_charge_cents: string;
  note_template_id: string;
  active: boolean;
  change_reason: string;
};
export type Check = { ok: true; body: Record<string, unknown> } | { ok: false; error: string };
export type WriteContext = {
  organizationId: string;
  templates: readonly TemplateRef[];
  facilities: readonly FacilityRef[];
  programs: readonly CareProgram[];
  services: readonly ProgramService[];
};

const hiddenSnapshotKeys = new Set(["narrative", "clinical_narrative", "diagnosis", "presenting_problem"]);
const idSnapshotKeys = new Set(["facility_id", "note_template_id", "program_id", "required_intake_template_ids", "required_discharge_template_ids"]);

function fail(detail: string): never {
  throw new Error(`Program configuration could not be read (${detail}). Refresh before continuing.`);
}
function otherOrg(): never {
  throw new Error("Program configuration includes another organization. Refresh before continuing.");
}
function object(input: unknown, detail: string): Record<string, unknown> {
  if (!input || typeof input !== "object" || Array.isArray(input)) fail(detail);
  return input as Record<string, unknown>;
}
function text(row: Record<string, unknown>, key: string): string {
  if (typeof row[key] !== "string" || row[key].length === 0) fail(key);
  return row[key];
}
function bool(row: Record<string, unknown>, key: string): boolean {
  if (typeof row[key] !== "boolean") fail(key);
  return row[key];
}
function int(row: Record<string, unknown>, key: string, min: number, max: number): number {
  const value = row[key];
  if (typeof value !== "number" || !Number.isInteger(value) || value < min || value > max) fail(key);
  return value;
}
function intOrNull(row: Record<string, unknown>, key: string, min: number, max: number): number | null {
  if (row[key] === null) return null;
  return int(row, key, min, max);
}
function idList(row: Record<string, unknown>, key: string): string[] {
  const value = row[key];
  if (!Array.isArray(value) || value.length > 500) fail(key);
  const ids = value.map(item => {
    if (typeof item !== "string" || item.length === 0) fail(key);
    return item;
  });
  if (new Set(ids).size !== ids.length) fail(`${key} duplicates`);
  return ids;
}
function hasControl(value: string): boolean {
  for (let i = 0; i < value.length; i += 1) if (value.charCodeAt(i) < 32) return true;
  return false;
}
function isCanonicalInt(value: string): boolean {
  if (value === "0") return true;
  if (!value || value[0] === "0") return false;
  for (const ch of value) if (ch < "0" || ch > "9") return false;
  return true;
}
function invalid(error: string): Check {
  return { ok: false, error };
}
function valid(body: Record<string, unknown>): Check {
  return { ok: true, body };
}
function sameIds(left: readonly string[], right: readonly string[]): boolean {
  if (left.length !== right.length) return false;
  const a = [...left].sort();
  const b = [...right].sort();
  return a.every((id, index) => id === b[index]);
}
function draftInt(input: string, min: number, max: number): number | null {
  const textValue = input.trim();
  if (!isCanonicalInt(textValue)) return null;
  const value = Number(textValue);
  if (!Number.isInteger(value) || value < min || value > max) return null;
  return value;
}
function normalizedCode(input: string): string | null {
  const code = input.trim().toUpperCase();
  if (code.length < 1 || code.length > bounds.codeMax || hasControl(code)) return null;
  return code;
}
function normalizedName(input: string): string | null {
  const name = input.trim();
  if (name.length < 1 || name.length > bounds.nameMax || hasControl(name)) return null;
  return name;
}
function normalizedReason(input: string): string | null {
  const reason = input.trim();
  if (reason.length < bounds.reasonMin || reason.length > bounds.reasonMax) return null;
  return reason;
}
function templateError(ids: readonly string[], ctx: WriteContext, original: readonly string[]): string | null {
  if (new Set(ids).size !== ids.length) return "Duplicate template selections are not allowed.";
  for (const id of ids) {
    const template = ctx.templates.find(item => item.id === id);
    if (!template || (template.organizationId && template.organizationId !== ctx.organizationId)) {
      if (!original.includes(id)) return "A selected template is not in this organization.";
      continue;
    }
    if (!template.active && !original.includes(id)) return "Inactive templates cannot be newly required.";
  }
  return null;
}
function named(id: string, nameOf: (id: string) => string): string {
  const name = nameOf(id);
  return name && name !== id ? `${name} (${id})` : id;
}

export function programPath(id?: string): string {
  return id ? `/api/care-programs/${encodeURIComponent(id)}` : "/api/care-programs";
}
export function servicePath(id?: string): string {
  return id ? `/api/program-services/${encodeURIComponent(id)}` : "/api/program-services";
}
export function programHistoryPath(id: string): string {
  return `${programPath(id)}/history`;
}
export function serviceHistoryPath(id: string): string {
  return `${servicePath(id)}/history`;
}
export function isProgramDraft(draft: ProgramDraft | ServiceDraft): draft is ProgramDraft {
  return "level_of_care" in draft;
}
export function toggleId(ids: readonly string[], id: string): string[] {
  return ids.includes(id) ? ids.filter(item => item !== id) : [...ids, id];
}
export function censusMismatch(program: CareProgram): boolean {
  return program.available_places !== Math.max(0, program.capacity - program.active_census);
}
export function filterPrograms(programs: readonly CareProgram[], facilityId: string, status: "all" | "active" | "inactive"): CareProgram[] {
  return programs.filter(program => {
    if (facilityId && facilityId !== "all" && program.facility_id !== facilityId) return false;
    if (status === "active" && !program.active) return false;
    if (status === "inactive" && program.active) return false;
    return true;
  });
}
export function servicesForProgram(services: readonly ProgramService[], programId: string): ProgramService[] {
  return services.filter(service => service.program_id === programId);
}
export function readTemplates(rows: readonly { id: string; [key: string]: unknown }[]): TemplateRef[] {
  const seen = new Set<string>();
  return rows.map(row => {
    if (!row || typeof row.id !== "string" || !row.id) fail("template");
    if (seen.has(row.id)) fail("duplicate template");
    seen.add(row.id);
    const name = typeof row.name === "string" && row.name.trim() ? row.name.trim() : typeof row.title === "string" && row.title.trim() ? row.title.trim() : "Untitled template";
    return { id: row.id, name, active: row.active === true, organizationId: typeof row.organization_id === "string" ? row.organization_id : undefined };
  });
}
export function formatChargeInput(cents: number | null): string {
  if (cents == null) return "";
  if (!Number.isInteger(cents) || cents < 0 || cents > bounds.maxChargeCents) return "";
  return `${Math.trunc(cents / 100)}.${String(cents % 100).padStart(2, "0")}`;
}
export function formatChargeDisplay(cents: number | null): string {
  const input = formatChargeInput(cents);
  return input ? `$${input}` : "None";
}
export function parseChargeInput(input: string): number | null | string {
  const trimmed = input.trim();
  if (!trimmed) return null;
  const body = trimmed[0] === "$" ? trimmed.slice(1).trim() : trimmed;
  if (!body) return "Default charge must be blank or a dollar amount with exact cents.";
  if (body.includes(",") && !/^(?:0|[1-9]\d{0,2})(?:,\d{3})+(?:\.\d{1,2})?$/.test(body)) return "Default charge must be blank or a dollar amount with exact cents.";
  const plain = body.replace(/,/g, "");
  const text = plain.startsWith(".") ? `0${plain}` : plain;
  if (!/^(?:0|[1-9]\d*)(?:\.\d{1,2})?$/.test(text)) return "Default charge must be blank or a dollar amount with exact cents.";
  const [whole, frac = ""] = text.split(".");
  const cents = Number(whole) * 100 + Number((frac + "00").slice(0, 2));
  if (!Number.isSafeInteger(cents) || cents > bounds.maxChargeCents) return "Default charge must be blank or a dollar amount with exact cents.";
  return cents;
}
export function canMutatePrograms(canManage: boolean, role: string): boolean {
  return canManage === true && role === "admin";
}
export function templateAccess(permissions: unknown): "allowed" | "denied" | "unknown" {
  if (permissions == null || typeof permissions !== "object" || Array.isArray(permissions) || !("templates" in permissions)) return "unknown";
  const value = (permissions as { templates?: unknown }).templates;
  if (value === false) return "denied";
  if (value === true) return "allowed";
  if (!value || typeof value !== "object" || Array.isArray(value)) return "unknown";
  const read = (value as { read?: unknown }).read;
  if (read === false) return "denied";
  if (read === true) return "allowed";
  return "unknown";
}
export function readTemplatesIfAvailable(rows: unknown, permissions: unknown): TemplateRef[] {
  if (templateAccess(permissions) === "denied") return [];
  if (!Array.isArray(rows)) {
    if (templateAccess(permissions) === "allowed") fail("template");
    return [];
  }
  return readTemplates(rows);
}
export function absentTemplateIds(ids: readonly string[], templates: readonly TemplateRef[]): string[] {
  const known = new Set(templates.map(item => item.id));
  return [...new Set(ids)].filter(id => id.length > 0 && !known.has(id));
}
export function visibleTemplates(templates: readonly TemplateRef[], organizationId: string, selected: readonly string[], original: readonly string[]): TemplateRef[] {
  const allowed = templates.filter(template => {
    const retained = selected.includes(template.id) || original.includes(template.id);
    if (template.organizationId && template.organizationId !== organizationId && !retained) return false;
    return template.active || retained;
  });
  const known = new Set(allowed.map(item => item.id));
  return [...allowed, ...[...new Set([...selected, ...original])].filter(id => id.length > 0 && !known.has(id)).map(id => ({ id, name: `Template ${id} (record unavailable)`, active: true }))];
}
export function historyInScope(revisions: readonly { organization_id: string; program_id: string | null; program_service_id: string | null }[], kind: "program" | "service", id: string, organizationId: string): boolean {
  if (!id || !organizationId) return false;
  return revisions.every(revision => revision.organization_id === organizationId && (kind === "program" ? revision.program_id === id : revision.program_service_id === id));
}
export function staleScope(loadTicket: number, currentTicket: number, loadActor: string, actor: string, loadOrg: string, organization: string): boolean {
  return loadTicket !== currentTicket || !loadActor || loadActor !== actor || !loadOrg || loadOrg !== organization;
}
export function configurationBlockers(ids: readonly string[], templates: readonly TemplateRef[], organizationId: string): string[] {
  const messages: string[] = [];
  const seen = new Set<string>();
  for (const id of ids) {
    if (seen.has(id)) {
      messages.push(`Template ${id} is listed more than once.`);
      continue;
    }
    seen.add(id);
    const template = templates.find(item => item.id === id);
    if (!template) continue;
    if (template.organizationId && template.organizationId !== organizationId) messages.push(`Template ${id} is not in this organization.`);
    else if (!template.active) messages.push(`${template.name} is inactive and still required.`);
  }
  return messages;
}
export function programBlockers(program: CareProgram, templates: readonly TemplateRef[], organizationId: string): string[] {
  return [
    ...(censusMismatch(program) ? ["Free places do not match capacity minus active census."] : []),
    ...configurationBlockers(program.required_intake_template_ids, templates, organizationId).map(item => `Intake: ${item}`),
    ...configurationBlockers(program.required_discharge_template_ids, templates, organizationId).map(item => `Discharge: ${item}`),
  ];
}
export function emptyProgramDraft(facilityId = ""): ProgramDraft {
  return { facility_id: facilityId, code: "", name: "", level_of_care: "", min_age: "", max_age: "", capacity: "", active: true, intake_ids: [], discharge_ids: [], change_reason: "" };
}
export function programDraftFrom(program: CareProgram): ProgramDraft {
  return {
    facility_id: program.facility_id,
    code: program.code,
    name: program.name,
    level_of_care: program.level_of_care,
    min_age: String(program.min_age),
    max_age: program.max_age == null ? "" : String(program.max_age),
    capacity: String(program.capacity),
    active: program.active,
    intake_ids: [...program.required_intake_template_ids],
    discharge_ids: [...program.required_discharge_template_ids],
    change_reason: "",
  };
}
export function emptyServiceDraft(programId: string): ServiceDraft {
  return { program_id: programId, code_system: "", service_code: "", name: "", session_kind: "", min_minutes: "", default_minutes: "", max_minutes: "", default_charge_cents: "", note_template_id: "", active: true, change_reason: "" };
}
export function serviceDraftFrom(service: ProgramService): ServiceDraft {
  return {
    program_id: service.program_id,
    code_system: service.code_system,
    service_code: service.service_code,
    name: service.name,
    session_kind: service.session_kind,
    min_minutes: String(service.min_minutes),
    default_minutes: String(service.default_minutes),
    max_minutes: String(service.max_minutes),
    default_charge_cents: formatChargeInput(service.default_charge_cents),
    note_template_id: service.note_template_id ?? "",
    active: service.active,
    change_reason: "",
  };
}

export function parseProgram(input: unknown, organizationId: string): CareProgram {
  const row = object(input, "program");
  if (!organizationId) fail("organization");
  const organization_id = text(row, "organization_id");
  if (organization_id !== organizationId) otherOrg();
  const level = text(row, "level_of_care");
  if (!levelsOfCare.includes(level as LevelOfCare)) fail("level_of_care");
  const min_age = int(row, "min_age", bounds.minAge, bounds.maxMinAge);
  const codeSystem = text(row, "code");
  return {
    id: text(row, "id"),
    organization_id,
    facility_id: text(row, "facility_id"),
    code: codeSystem,
    name: text(row, "name"),
    level_of_care: level as LevelOfCare,
    min_age,
    max_age: intOrNull(row, "max_age", min_age, bounds.maxAge),
    capacity: int(row, "capacity", bounds.minCapacity, bounds.maxCapacity),
    active: bool(row, "active"),
    version: int(row, "version", 1, 1000000000),
    created_at: text(row, "created_at"),
    updated_at: text(row, "updated_at"),
    required_intake_template_ids: idList(row, "required_intake_template_ids"),
    required_discharge_template_ids: idList(row, "required_discharge_template_ids"),
    active_census: int(row, "active_census", 0, 1000000000),
    available_places: int(row, "available_places", 0, 1000000000),
  };
}
export function parseService(input: unknown, organizationId: string): ProgramService {
  const row = object(input, "service");
  if (!organizationId) fail("organization");
  const organization_id = text(row, "organization_id");
  if (organization_id !== organizationId) otherOrg();
  const code_system = text(row, "code_system");
  const session_kind = text(row, "session_kind");
  if (!codeSystems.includes(code_system as CodeSystem)) fail("code_system");
  if (!sessionKinds.includes(session_kind as SessionKind)) fail("session_kind");
  const min_minutes = int(row, "min_minutes", 1, bounds.maxMinutes);
  const default_minutes = int(row, "default_minutes", 1, bounds.maxMinutes);
  const max_minutes = int(row, "max_minutes", 1, bounds.maxMinutes);
  if (min_minutes > default_minutes || default_minutes > max_minutes) fail("minutes");
  const note = row.note_template_id;
  if (note !== null && (typeof note !== "string" || note.length === 0)) fail("note_template_id");
  return {
    id: text(row, "id"),
    organization_id,
    program_id: text(row, "program_id"),
    code_system: code_system as CodeSystem,
    service_code: text(row, "service_code"),
    name: text(row, "name"),
    session_kind: session_kind as SessionKind,
    min_minutes,
    default_minutes,
    max_minutes,
    default_charge_cents: intOrNull(row, "default_charge_cents", 0, bounds.maxChargeCents),
    note_template_id: note === null ? null : note,
    active: bool(row, "active"),
    version: int(row, "version", 1, 1000000000),
    created_at: text(row, "created_at"),
    updated_at: text(row, "updated_at"),
  };
}
export function parseProgramCatalog(input: unknown, organizationId: string): ProgramCatalog {
  const root = object(input, "catalog");
  if (!organizationId) fail("organization");
  if (typeof root.can_manage !== "boolean") fail("can_manage");
  if (!Array.isArray(root.programs) || !Array.isArray(root.services)) fail("catalog lists");
  if (root.programs.length > bounds.programLimit) throw new Error("Program catalog exceeds 1000 programs and was not truncated.");
  if (root.services.length > bounds.serviceLimit) throw new Error("Program catalog exceeds 5000 services and was not truncated.");
  const programs = root.programs.map(item => parseProgram(item, organizationId));
  const services = root.services.map(item => parseService(item, organizationId));
  if (new Set(programs.map(item => item.id)).size !== programs.length) fail("duplicate program");
  if (new Set(services.map(item => item.id)).size !== services.length) fail("duplicate service");
  if(services.some(service=>!programs.some(p=>p.id===service.program_id)))fail("service parent program");
  return { programs, services, can_manage: root.can_manage };
}
function parseRevision(input: unknown, kind: "program" | "service", organizationId: string): Revision {
  const row = object(input, "revision");
  const organization_id = text(row, "organization_id");
  if (organization_id !== organizationId) otherOrg();
  const program_id = typeof row.program_id === "string" && row.program_id ? row.program_id : null;
  const program_service_id = typeof row.program_service_id === "string" && row.program_service_id ? row.program_service_id : typeof row.service_id === "string" && row.service_id ? row.service_id : null;
  if (kind === "program" && !program_id) fail("program_id");
  if (kind === "service" && !program_service_id) fail("program_service_id");
  if (!row.snapshot || typeof row.snapshot !== "object" || Array.isArray(row.snapshot)) fail("snapshot");
  return {
    id: text(row, "id"),
    organization_id,
    program_id,
    program_service_id,
    version: int(row, "version", 1, 1000000000),
    snapshot: row.snapshot as Record<string, unknown>,
    reason: text(row, "reason"),
    actor_id: text(row, "actor_id"),
    created_at: text(row, "created_at"),
  };
}
export function parseHistory(input: unknown, kind: "program" | "service", organizationId: string): Revision[] {
  const root = object(input, "history");
  if (!organizationId) fail("organization");
  if (!Array.isArray(root.revisions)) fail("revisions");
  if (root.revisions.length > bounds.revisionLimit) throw new Error("Revision history exceeds 200 records and was not truncated.");
  const revisions = root.revisions.map(item => parseRevision(item, kind, organizationId));
  if (new Set(revisions.map(item => item.id)).size !== revisions.length) fail("duplicate revision");
  return revisions;
}

export function displaySnapshot(snapshot: Record<string, unknown>, nameOf: (id: string) => string = () => ""): { label: string; value: string }[] {
  const labels: Record<string, string> = {
    code: "Code", name: "Name", facility_id: "Facility", level_of_care: "Level of care", min_age: "Minimum age", max_age: "Maximum age", capacity: "Capacity", active: "Status", required_intake_template_ids: "Intake forms", required_discharge_template_ids: "Discharge forms", program_id: "Program", code_system: "Code system", service_code: "Service code", session_kind: "Session kind", min_minutes: "Minimum minutes", default_minutes: "Default minutes", max_minutes: "Maximum minutes", default_charge_cents: "Default charge", note_template_id: "Note template", version: "Version",
  };
  const preferred = Object.keys(labels);
  const keys = [...preferred.filter(key => key in snapshot), ...Object.keys(snapshot).filter(key => !preferred.includes(key))];
  return keys.filter(key => !hiddenSnapshotKeys.has(key)).map(key => {
    const value = snapshot[key];
    let printed = "None";
    if (Array.isArray(value)) printed = value.length ? value.map(item => typeof item === "string" && idSnapshotKeys.has(key) ? named(item, nameOf) : String(item)).join(", ") : "None";
    else if (typeof value === "boolean") printed = key === "active" ? (value ? "Active" : "Inactive") : value ? "Yes" : "No";
    else if (typeof value === "number") printed = key === "default_charge_cents" ? (formatChargeInput(value) ? formatChargeDisplay(value) : String(value)) : String(value);
    else if (typeof value === "string") printed = idSnapshotKeys.has(key) ? named(value, nameOf) : value;
    else if (value && typeof value === "object") printed = "Nested value omitted";
    return { label: labels[key] || key, value: printed.length > 500 ? `${printed.slice(0, 500)}…` : printed };
  });
}

function ages(draft: ProgramDraft): { min: number; max: number | null } | string {
  const min = draftInt(draft.min_age, bounds.minAge, bounds.maxMinAge);
  if (min == null) return "Minimum age must be a whole number from 18 to 120.";
  if (!draft.max_age.trim()) return { min, max: null };
  const max = draftInt(draft.max_age, min, bounds.maxAge);
  if (max == null) return "Maximum age must be empty or a whole number from the minimum age through 130.";
  return { min, max };
}
function capacityOf(draft: ProgramDraft, census?: number): number | string {
  const capacity = draftInt(draft.capacity, bounds.minCapacity, bounds.maxCapacity);
  if (capacity == null) return "Capacity must be a whole number from 1 to 10000.";
  if (census != null && capacity < census) return "Capacity cannot be reduced below the active census.";
  return capacity;
}
function minutesOf(draft: ServiceDraft): { min: number; def: number; max: number } | string {
  const min = draftInt(draft.min_minutes, 1, bounds.maxMinutes);
  const def = draftInt(draft.default_minutes, 1, bounds.maxMinutes);
  const max = draftInt(draft.max_minutes, 1, bounds.maxMinutes);
  if (min == null || def == null || max == null || min > def || def > max) return "Minutes must be whole numbers from 1 to 480, with minimum, then default, then maximum.";
  return { min, def, max };
}
function chargeOf(input: string): number | null | string {
  const charge = parseChargeInput(input);
  return typeof charge === "string" ? "Default charge must be blank or dollars from 0.00 through 1000000.00. Blank is no default; 0.00 is zero." : charge;
}
function programActiveContext(active: boolean, facilityId: string, ctx: WriteContext): string | null {
  if (!active) return null;
  const facility = ctx.facilities.find(item => item.id === facilityId);
  if (!facility || !facility.active) return "An active facility is required.";
  return null;
}

export function buildProgramCreate(draft: ProgramDraft, ctx: WriteContext): Check {
  const reason = normalizedReason(draft.change_reason);
  if (!reason) return invalid("Enter a change reason of 10 to 2000 characters.");
  const code = normalizedCode(draft.code);
  const name = normalizedName(draft.name);
  if (!code) return invalid("Code must be 1 to 40 characters.");
  if (!name) return invalid("Name must be 1 to 160 characters.");
  if (!levelsOfCare.includes(draft.level_of_care as LevelOfCare)) return invalid("Choose a level of care.");
  const age = ages(draft);
  if (typeof age === "string") return invalid(age);
  const capacity = capacityOf(draft);
  if (typeof capacity === "string") return invalid(capacity);
  if (programActiveContext(true, draft.facility_id, ctx)) return invalid("An active facility is required.");
  if (ctx.programs.some(program => program.code.toUpperCase() === code)) return invalid("A program with this code already exists.");
  const intake = templateError(draft.intake_ids, ctx, []);
  if (intake) return invalid(intake);
  const discharge = templateError(draft.discharge_ids, ctx, []);
  if (discharge) return invalid(discharge);
  return valid({
    facility_id: draft.facility_id,
    code,
    name,
    level_of_care: draft.level_of_care,
    min_age: age.min,
    max_age: age.max,
    capacity,
    active: draft.active,
    required_intake_template_ids: [...draft.intake_ids],
    required_discharge_template_ids: [...draft.discharge_ids],
    change_reason: reason,
  });
}
export function buildProgramPatch(program: CareProgram, draft: ProgramDraft, ctx: WriteContext): Check {
  const reason = normalizedReason(draft.change_reason);
  if (!reason) return invalid("Enter a change reason of 10 to 2000 characters.");
  if (!program.updated_at) return invalid("The loaded timestamp is missing. Refresh before continuing.");
  const name = normalizedName(draft.name);
  if (!name) return invalid("Name must be 1 to 160 characters.");
  const age = ages(draft);
  if (typeof age === "string") return invalid(age);
  const capacity = capacityOf(draft, program.active_census);
  if (typeof capacity === "string") return invalid(capacity);
  const facilityError = programActiveContext(draft.active && !program.active, program.facility_id, ctx);
  if (facilityError) return invalid(facilityError);
  const intake = templateError(draft.intake_ids, ctx, sameIds(draft.intake_ids,program.required_intake_template_ids) ? program.required_intake_template_ids : []);
  if (intake) return invalid(intake);
  const discharge = templateError(draft.discharge_ids, ctx, sameIds(draft.discharge_ids,program.required_discharge_template_ids) ? program.required_discharge_template_ids : []);
  if (discharge) return invalid(discharge);
  const body: Record<string, unknown> = { _expected_updated_at: program.updated_at, change_reason: reason };
  if (name !== program.name) body.name = name;
  if (age.min !== program.min_age) body.min_age = age.min;
  if (age.max !== program.max_age) body.max_age = age.max;
  if (capacity !== program.capacity) body.capacity = capacity;
  if (draft.active !== program.active) body.active = draft.active;
  if (!sameIds(draft.intake_ids, program.required_intake_template_ids)) body.required_intake_template_ids = [...draft.intake_ids];
  if (!sameIds(draft.discharge_ids, program.required_discharge_template_ids)) body.required_discharge_template_ids = [...draft.discharge_ids];
  if (Object.keys(body).length === 2) return invalid("No configuration changes to save.");
  return valid(body);
}
function serviceParent(programId: string, ctx: WriteContext, mustBeActive: boolean): string | CareProgram {
  const program = ctx.programs.find(item => item.id === programId && item.organization_id === ctx.organizationId);
  if (!program) return "Program record is unavailable. Refresh before continuing.";
  if (mustBeActive && !program.active) return "The program must be active.";
  const facility = ctx.facilities.find(item => item.id === program.facility_id);
  if (mustBeActive && (!facility || !facility.active)) return "The program facility must be active.";
  return program;
}
export function buildServiceCreate(draft: ServiceDraft, ctx: WriteContext): Check {
  const reason = normalizedReason(draft.change_reason);
  if (!reason) return invalid("Enter a change reason of 10 to 2000 characters.");
  const parent = serviceParent(draft.program_id, ctx, true);
  if (typeof parent === "string") return invalid(parent);
  if (!codeSystems.includes(draft.code_system as CodeSystem)) return invalid("Choose a code system.");
  if (!sessionKinds.includes(draft.session_kind as SessionKind)) return invalid("Choose a session kind.");
  const code = normalizedCode(draft.service_code);
  const name = normalizedName(draft.name);
  if (!code) return invalid("Code must be 1 to 40 characters.");
  if (!name) return invalid("Name must be 1 to 160 characters.");
  const minutes = minutesOf(draft);
  if (typeof minutes === "string") return invalid(minutes);
  const charge = chargeOf(draft.default_charge_cents);
  if (typeof charge === "string") return invalid(charge);
  const templateIds = draft.note_template_id ? [draft.note_template_id] : [];
  const template = templateError(templateIds, ctx, []);
  if (template) return invalid(template);
  if (draft.active && programActiveContext(true, parent.facility_id, ctx)) return invalid("The program facility must be active.");
  if (ctx.services.some(service => service.program_id === draft.program_id && service.code_system === draft.code_system && service.service_code.toUpperCase() === code)) return invalid("This service code already exists on the program.");
  return valid({
    program_id: draft.program_id,
    code_system: draft.code_system,
    service_code: code,
    name,
    session_kind: draft.session_kind,
    min_minutes: minutes.min,
    default_minutes: minutes.def,
    max_minutes: minutes.max,
    default_charge_cents: charge,
    note_template_id: draft.note_template_id || null,
    active: draft.active,
    change_reason: reason,
  });
}
export function buildServicePatch(service: ProgramService, draft: ServiceDraft, ctx: WriteContext): Check {
  const reason = normalizedReason(draft.change_reason);
  if (!reason) return invalid("Enter a change reason of 10 to 2000 characters.");
  if (!service.updated_at) return invalid("The loaded timestamp is missing. Refresh before continuing.");
  const parent = serviceParent(service.program_id, ctx, draft.active && !service.active);
  if (typeof parent === "string") return invalid(parent);
  const name = normalizedName(draft.name);
  if (!name) return invalid("Name must be 1 to 160 characters.");
  const minutes = minutesOf(draft);
  if (typeof minutes === "string") return invalid(minutes);
  const charge = chargeOf(draft.default_charge_cents);
  if (typeof charge === "string") return invalid(charge);
  const template = templateError(draft.note_template_id ? [draft.note_template_id] : [], ctx, service.note_template_id ? [service.note_template_id] : []);
  if (template) return invalid(template);
  const body: Record<string, unknown> = { _expected_updated_at: service.updated_at, change_reason: reason };
  if (name !== service.name) body.name = name;
  if (minutes.min !== service.min_minutes) body.min_minutes = minutes.min;
  if (minutes.def !== service.default_minutes) body.default_minutes = minutes.def;
  if (minutes.max !== service.max_minutes) body.max_minutes = minutes.max;
  if (charge !== service.default_charge_cents) body.default_charge_cents = charge;
  const nextTemplate = draft.note_template_id || null;
  if (nextTemplate !== service.note_template_id) body.note_template_id = nextTemplate;
  if (draft.active !== service.active) body.active = draft.active;
  if (Object.keys(body).length === 2) return invalid("No configuration changes to save.");
  return valid(body);
}

function copyProgram(program: CareProgram): CareProgram {
  return { ...program, required_intake_template_ids: [...program.required_intake_template_ids], required_discharge_template_ids: [...program.required_discharge_template_ids] };
}
export function applyProgramUpdate(catalog: ProgramCatalog, id: string, patch: Record<string, unknown>): ProgramCatalog {
  if (!catalog.programs.some(program => program.id === id)) return { ...catalog, programs: catalog.programs.map(copyProgram), services: [...catalog.services] };
  return {
    ...catalog,
    programs: catalog.programs.map(program => {
      if (program.id !== id) return copyProgram(program);
      const next = copyProgram(program);
      if (typeof patch.name === "string") next.name = patch.name;
      if (typeof patch.min_age === "number") next.min_age = patch.min_age;
      if (patch.max_age === null || typeof patch.max_age === "number") next.max_age = patch.max_age as number | null;
      if (typeof patch.capacity === "number") {
        if (patch.capacity < program.active_census) throw new Error("Capacity cannot be reduced below the active census.");
        next.capacity = patch.capacity;
        next.available_places = patch.capacity - program.active_census;
      }
      if (typeof patch.active === "boolean") next.active = patch.active;
      if (Array.isArray(patch.required_intake_template_ids)) next.required_intake_template_ids = [...patch.required_intake_template_ids];
      if (Array.isArray(patch.required_discharge_template_ids)) next.required_discharge_template_ids = [...patch.required_discharge_template_ids];
      next.facility_id = program.facility_id;
      next.code = program.code;
      next.level_of_care = program.level_of_care;
      next.updated_at = program.updated_at;
      next.version = program.version;
      next.active_census = program.active_census;
      return next;
    }),
    services: [...catalog.services],
  };
}
export function applyServiceUpdate(catalog: ProgramCatalog, id: string, patch: Record<string, unknown>): ProgramCatalog {
  return {
    ...catalog,
    programs: catalog.programs.map(copyProgram),
    services: catalog.services.map(service => {
      if (service.id !== id) return { ...service };
      const next = { ...service };
      if (typeof patch.name === "string") next.name = patch.name;
      if (typeof patch.min_minutes === "number") next.min_minutes = patch.min_minutes;
      if (typeof patch.default_minutes === "number") next.default_minutes = patch.default_minutes;
      if (typeof patch.max_minutes === "number") next.max_minutes = patch.max_minutes;
      if (next.min_minutes > next.default_minutes || next.default_minutes > next.max_minutes) throw new Error("Minutes are outside the service bounds.");
      if (patch.default_charge_cents === null || typeof patch.default_charge_cents === "number") next.default_charge_cents = patch.default_charge_cents as number | null;
      if (patch.note_template_id === null || typeof patch.note_template_id === "string") next.note_template_id = patch.note_template_id as string | null;
      if (typeof patch.active === "boolean") next.active = patch.active;
      next.program_id = service.program_id;
      next.code_system = service.code_system;
      next.service_code = service.service_code;
      next.session_kind = service.session_kind;
      next.updated_at = service.updated_at;
      next.version = service.version;
      return next;
    }),
  };
}
export function replaceProgram(catalog: ProgramCatalog, program: CareProgram): ProgramCatalog {
  const exists = catalog.programs.some(item => item.id === program.id);
  return { ...catalog, programs: exists ? catalog.programs.map(item => item.id === program.id ? program : item) : [...catalog.programs, program] };
}
export function replaceService(catalog: ProgramCatalog, service: ProgramService): ProgramCatalog {
  const exists = catalog.services.some(item => item.id === service.id);
  return { ...catalog, services: exists ? catalog.services.map(item => item.id === service.id ? service : item) : [...catalog.services, service] };
}
