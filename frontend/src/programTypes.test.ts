import assert from "node:assert/strict";
import test from "node:test";
import {
  applyProgramUpdate,
  applyServiceUpdate,
  buildProgramCreate,
  buildProgramPatch,
  buildServiceCreate,
  buildServicePatch,
  catalogPath,
  censusDefinition,
  censusMismatch,
  configurationBlockers,
  displaySnapshot,
  filterPrograms,
  parseHistory,
  parseProgram,
  parseProgramCatalog,
  parseService,
  programBlockers,
  programDraftFrom,
  programHistoryPath,
  programPath,
  readTemplates,
  absentTemplateIds,
  canMutatePrograms,
  emptyServiceDraft,
  formatChargeDisplay,
  formatChargeInput,
  historyInScope,
  parseChargeInput,
  readTemplatesIfAvailable,
  replaceProgram,
  serviceDraftFrom,
  staleScope,
  templateAccess,
  visibleTemplates,
  serviceHistoryPath,
  servicePath,
  servicesForProgram,
  toggleId,
} from "./programTypes.ts";
import type { WriteContext } from "./programTypes.ts";

const stamp = "2026-11-02T01:02:03.456+00:00";
function rawProgram(over: Record<string, unknown> = {}) {
  return { id: "p1", organization_id: "org", facility_id: "f1", code: "OP", name: "Outpatient", level_of_care: "outpatient", min_age: 18, max_age: null, capacity: 10, active: true, version: 1, created_at: stamp, updated_at: stamp, required_intake_template_ids: [], required_discharge_template_ids: [], active_census: 2, available_places: 8, extra: "ignored", ...over };
}
function rawService(over: Record<string, unknown> = {}) {
  return { id: "s1", organization_id: "org", program_id: "p1", code_system: "cpt", service_code: "90791", name: "Assessment", session_kind: "assessment", min_minutes: 30, default_minutes: 60, max_minutes: 90, default_charge_cents: null, note_template_id: null, active: true, version: 1, created_at: stamp, updated_at: stamp, ...over };
}
function ctx(over: Partial<WriteContext> = {}): WriteContext {
  return { organizationId: "org", templates: [{ id: "t1", name: "Intake packet", active: true, organizationId: "org" }, { id: "old", name: "Old packet", active: false, organizationId: "org" }], facilities: [{ id: "f1", name: "North", active: true }], programs: [], services: [], ...over };
}

test("catalog parser accepts contract records and rejects unsafe catalogs", () => {
  const parsed = parseProgramCatalog({ programs: [rawProgram({ required_intake_template_ids: ["old"] })], services: [rawService()], can_manage: false }, "org");
  assert.equal(parsed.programs[0].max_age, null);
  assert.deepEqual(parsed.programs[0].required_intake_template_ids, ["old"]);
  assert.equal(parsed.services[0].default_charge_cents, null);
  assert.equal(parsed.can_manage, false);
  assert.equal("extra" in parsed.programs[0], false);
  assert.equal(parseProgramCatalog({ programs: [], services: [], can_manage: true }, "org").programs.length, 0);
  assert.throws(() => parseProgramCatalog({ programs: Array.from({ length: 1001 }, (_, i) => rawProgram({ id: `p${i}` })), services: [], can_manage: true }, "org"), /not truncated/);
  assert.throws(() => parseProgramCatalog({ programs: [], services: Array.from({ length: 5001 }, (_, i) => rawService({ id: `s${i}` })), can_manage: true }, "org"), /not truncated/);
  assert.throws(() => parseProgramCatalog({ programs: [rawProgram({ organization_id: "other" })], services: [], can_manage: true }, "org"), /another organization/);
  assert.throws(() => parseProgramCatalog({ programs: [], services: [rawService({ organization_id: "other" })], can_manage: true }, "org"), /another organization/);
  assert.throws(() => parseProgramCatalog({ programs: [rawProgram(), rawProgram()], services: [], can_manage: true }, "org"), /duplicate program/);
  assert.throws(() => parseProgramCatalog({ programs: [], services: [rawService(), rawService()], can_manage: true }, "org"), /duplicate service/);
  assert.throws(() => parseProgram(rawProgram({ level_of_care: "residential" }), "org"), /level_of_care/);
  assert.throws(() => parseProgram(rawProgram({ min_age: 18.5 }), "org"), /min_age/);
  assert.throws(() => parseProgram(rawProgram({ available_places: -1 }), "org"), /available_places/);
  assert.throws(() => parseProgram(rawProgram({ active_census: undefined }), "org"), /active_census/);
  assert.throws(() => parseProgramCatalog({ programs: [], services: [], can_manage: "true" }, "org"), /can_manage/);
  assert.equal(censusMismatch(parseProgram(rawProgram({ available_places: 7 }), "org")), true);
  assert.equal(censusMismatch(parseProgram(rawProgram(), "org")), false);
  assert.equal(censusDefinition.includes("Planned intakes"), true);
});

test("history preserves order, bounds, organization, and service link aliases", () => {
  const revisions = [{ id: "r2", organization_id: "org", program_id: "p1", version: 2, snapshot: { name: "Newer" }, reason: "Second saved change", actor_id: "staff", created_at: stamp }, { id: "r1", organization_id: "org", program_id: "p1", version: 1, snapshot: {}, reason: "Created the program", actor_id: "staff", created_at: stamp }];
  const parsed = parseHistory({ revisions }, "program", "org");
  assert.deepEqual(parsed.map(item => item.version), [2, 1]);
  assert.equal(parseHistory({ revisions: [{ id: "sr", organization_id: "org", service_id: "s1", version: 1, snapshot: { service_code: "90791" }, reason: "Created service", actor_id: "staff", created_at: stamp }] }, "service", "org")[0].program_service_id, "s1");
  assert.throws(() => parseHistory({ revisions: Array.from({ length: 201 }, (_, i) => ({ id: `r${i}`, organization_id: "org", program_id: "p1", version: i + 1, snapshot: {}, reason: "Saved configuration", actor_id: "staff", created_at: stamp })) }, "program", "org"), /not truncated/);
  assert.throws(() => parseHistory({ revisions: [{ id: "r", organization_id: "other", program_id: "p1", version: 1, snapshot: {}, reason: "Created", actor_id: "staff", created_at: stamp }] }, "program", "org"), /another organization/);
  assert.throws(() => parseHistory({ revisions: [{ id: "r", organization_id: "org", program_id: "p1", version: 1, reason: "Created", actor_id: "staff", created_at: stamp }] }, "program", "org"), /snapshot/);
  assert.throws(() => parseHistory({ revisions: [{ id: "r", organization_id: "org", version: 1, snapshot: {}, reason: "Created", actor_id: "staff", created_at: stamp }] }, "service", "org"), /program_service_id/);
});

test("snapshot display and blockers keep inactive requirements visible", () => {
  const lines = displaySnapshot({ name: "IOP", required_intake_template_ids: ["t1"], diagnosis: "hidden", max_age: null, active: false }, id => id === "t1" ? "Intake packet" : "");
  assert.equal(lines.some(line => line.value.includes("Intake packet (t1)")), true);
  assert.equal(lines.some(line => line.label === "diagnosis" || line.value === "hidden"), false);
  assert.equal(lines.find(line => line.label === "Maximum age")?.value, "None");
  assert.equal(lines.find(line => line.label === "Status")?.value, "Inactive");
  const ids = ["old"];
  const blockers = configurationBlockers(ids, ctx().templates, "org");
  assert.deepEqual(ids, ["old"]);
  assert.equal(blockers[0].includes("inactive and still required"), true);
  const program = parseProgram(rawProgram({ required_discharge_template_ids: ["missing"], available_places: 3 }), "org");
  assert.equal(programBlockers(program, ctx().templates, "org").some(item => item.includes("Free places")), true);
  assert.equal(programBlockers(program, ctx().templates, "org").some(item => item.includes("missing")), false);
});

test("program writes use opened timestamp, bounds, and optimistic immutable fields", () => {
  const program = parseProgram(rawProgram({ required_intake_template_ids: ["old"] }), "org");
  const base = ctx({ programs: [program] });
  const created = buildProgramCreate({ facility_id: "f1", code: "op-1", name: "Adult IOP", level_of_care: "iop", min_age: "18", max_age: "", capacity: "12", active: false, intake_ids: [], discharge_ids: [], change_reason: "  Open the IOP program  " }, base);
  assert.equal(created.ok, true);
  if (created.ok) {
    assert.equal(created.body.code, "OP-1");
    assert.equal(created.body.max_age, null);
    assert.equal(created.body.change_reason, "Open the IOP program");
    assert.deepEqual(Object.keys(created.body).sort(), ["active", "capacity", "change_reason", "code", "facility_id", "level_of_care", "max_age", "min_age", "name", "required_discharge_template_ids", "required_intake_template_ids"]);
  }
  assert.equal(buildProgramCreate({ facility_id: "f1", code: "op", name: "X", level_of_care: "iop", min_age: "17", max_age: "", capacity: "1", active: true, intake_ids: [], discharge_ids: [], change_reason: "Reason long enough" }, base).ok, false);
  assert.equal(buildProgramCreate({ facility_id: "f1", code: "OP", name: "Copy", level_of_care: "outpatient", min_age: "18", max_age: "", capacity: "1", active: true, intake_ids: [], discharge_ids: [], change_reason: "Reason long enough" }, base).ok, false);
  assert.equal(buildProgramCreate({ facility_id: "f1", code: "NEW", name: "Copy", level_of_care: "outpatient", min_age: "18", max_age: "", capacity: "1", active: true, intake_ids: ["t1", "t1"], discharge_ids: [], change_reason: "Reason long enough" }, base).ok, false);
  assert.equal(buildProgramPatch(program, programDraftFrom(program), base).ok, false);
  const cleared = buildProgramPatch(parseProgram(rawProgram({ max_age: 65 }), "org"), { ...programDraftFrom(parseProgram(rawProgram({ max_age: 65 }), "org")), max_age: "", change_reason: "Remove the maximum age" }, base);
  assert.equal(cleared.ok && cleared.body.max_age, null);
  const kept = buildProgramPatch(program, { ...programDraftFrom(program), intake_ids: ["old", "t1"], change_reason: "Add intake form requirement" }, base);
  assert.equal(kept.ok, false);
  if (kept.ok) assert.deepEqual(kept.body.required_intake_template_ids, ["old", "t1"]);
  assert.equal(buildProgramPatch(program, { ...programDraftFrom(program), intake_ids: ["old", "missing"], change_reason: "Add a missing template" }, base).ok, false);
  const renamed = buildProgramPatch(program, { ...programDraftFrom(program), name: "Renamed program", change_reason: "Rename the outpatient program" }, base);
  assert.equal(renamed.ok, true);
  if (renamed.ok) {
    assert.deepEqual(Object.keys(renamed.body).sort(), ["_expected_updated_at", "change_reason", "name"]);
    assert.equal(renamed.body._expected_updated_at, stamp);
    assert.equal(renamed.body.facility_id, undefined);
    assert.equal(renamed.body.code, undefined);
    assert.equal(renamed.body.level_of_care, undefined);
  }
  const retired = buildProgramPatch(program, { ...programDraftFrom(program), active: false, change_reason: "Retire outpatient program" }, base);
  assert.equal(retired.ok, true);
  if (retired.ok) {
    const source = parseProgramCatalog({ programs: [rawProgram({ required_intake_template_ids: ["old"] })], services: [], can_manage: true }, "org");
    const next = applyProgramUpdate(source, "p1", retired.body);
    assert.equal(next.programs[0].active, false);
    assert.equal(next.programs[0].updated_at, stamp);
    assert.equal(next.programs[0].code, "OP");
    assert.equal(next.programs[0].version, 1);
    assert.equal(next.programs[0].active_census, 2);
    assert.equal(source.programs[0].active, true);
    next.programs[0].required_intake_template_ids.push("x");
    assert.deepEqual(source.programs[0].required_intake_template_ids, ["old"]);
  }
});

test("service writes preserve context and exact cents while distinguishing zero and unset",()=>{
  const program=parseProgram(rawProgram(),"org"),service=parseService(rawService(),"org");
  const context=ctx({programs:[program],services:[service]});
  const draft={...serviceDraftFrom(service),service_code:"ORG-NEW",default_charge_cents:"0",change_reason:"Add a configured assessment service"};
  const created=buildServiceCreate(draft,context);assert.equal(created.ok,true);if(created.ok){assert.equal(created.body.default_charge_cents,0);assert.equal(created.body.note_template_id,null);}
  assert.equal(buildServiceCreate({...draft,min_minutes:"60",default_minutes:"30"},context).ok,false);
  assert.equal(buildServiceCreate({...draft,default_charge_cents:"1.555"},context).ok,false);
  const edited=buildServicePatch(service,{...serviceDraftFrom(service),name:"Reviewed assessment",default_charge_cents:"0",change_reason:"Update service name and default charge"},context);
  assert.equal(edited.ok,true);if(edited.ok){assert.equal(edited.body._expected_updated_at,stamp);assert.equal(edited.body.default_charge_cents,0);assert.equal(edited.body.service_code,undefined);assert.equal(edited.body.program_id,undefined);}
  assert.equal(buildServicePatch(service,{...serviceDraftFrom(service),name:"Retained service",change_reason:"Keep the existing service history"},{...context,programs:[{...program,active:false}]}).ok,true);
  assert.equal(buildServicePatch({...service,active:false},{...serviceDraftFrom(service),active:true,change_reason:"Try to reactivate under inactive program"},{...context,programs:[{...program,active:false}]}).ok,false);
});

test("money, template permission, history scope, and stale loads", () => {
  assert.equal(parseChargeInput(""), null);
  assert.equal(parseChargeInput("   "), null);
  assert.equal(parseChargeInput("0"), 0);
  assert.equal(parseChargeInput("0.00"), 0);
  assert.equal(parseChargeInput("1.5"), 150);
  assert.equal(parseChargeInput("$1,250.50"), 125050);
  assert.equal(parseChargeInput("1000000.00"), 100000000);
  assert.equal(typeof parseChargeInput("1.555"), "string");
  assert.equal(typeof parseChargeInput("1000000.01"), "string");
  assert.equal(formatChargeInput(null), "");
  assert.equal(formatChargeInput(0), "0.00");
  assert.equal(formatChargeInput(1250), "12.50");
  assert.equal(formatChargeDisplay(null), "None");
  assert.equal(formatChargeDisplay(0), "$0.00");
  assert.equal(displaySnapshot({ default_charge_cents: 1250 }).find(line => line.label === "Default charge")?.value, "$12.50");
  assert.equal(displaySnapshot({ default_charge_cents: null }).find(line => line.label === "Default charge")?.value, "None");
  const service = parseService(rawService({ default_charge_cents: 1250 }), "org");
  assert.equal(serviceDraftFrom(service).default_charge_cents, "12.50");
  assert.equal(serviceDraftFrom(parseService(rawService(), "org")).default_charge_cents, "");
  assert.equal(emptyServiceDraft("p1").min_minutes, "");
  assert.equal(emptyServiceDraft("p1").default_charge_cents, "");
  const write = ctx({ programs: [parseProgram(rawProgram(), "org")], services: [parseService(rawService(), "org")] });
  const priced = buildServiceCreate({ ...serviceDraftFrom(parseService(rawService(), "org")), service_code: "ORG-FEE", default_charge_cents: "12.50", change_reason: "Set an exact dollar default" }, write);
  assert.equal(priced.ok && priced.body.default_charge_cents, 1250);
  const blank = buildServiceCreate({ ...serviceDraftFrom(parseService(rawService(), "org")), service_code: "ORG-BLANK", default_charge_cents: " ", change_reason: "Leave the default charge unset" }, write);
  assert.equal(blank.ok && blank.body.default_charge_cents, null);
  const zeroService = parseService(rawService({ default_charge_cents: 0 }), "org");
  const cleared = buildServicePatch(zeroService, { ...serviceDraftFrom(zeroService), default_charge_cents: "", change_reason: "Clear the suggested charge" }, write);
  assert.equal(cleared.ok && cleared.body.default_charge_cents, null);
  assert.equal(cleared.ok && cleared.body._expected_updated_at, stamp);
  assert.equal(buildServicePatch(zeroService, { ...serviceDraftFrom(zeroService), change_reason: "Keep zero charge exactly" }, write).ok, false);
  const source = parseProgramCatalog({ programs: [rawProgram()], services: [rawService({ default_charge_cents: 500 })], can_manage: true }, "org");
  const optimistic = applyServiceUpdate(source, "s1", { default_charge_cents: 0, updated_at: "2099-01-01T00:00:00.000Z", version: 9 });
  assert.equal(optimistic.services[0].default_charge_cents, 0);
  assert.equal(optimistic.services[0].updated_at, stamp);
  assert.equal(optimistic.services[0].version, 1);
  assert.equal(source.services[0].default_charge_cents, 500);
  const missingProgram = parseProgram(rawProgram({ required_intake_template_ids: ["not-loaded"] }), "org");
  const renamed = buildProgramPatch(missingProgram, { ...programDraftFrom(missingProgram), name: "Still valid", change_reason: "Rename without template records" }, ctx({ templates: [], programs: [missingProgram] }));
  assert.equal(renamed.ok, true);
  if (renamed.ok) assert.equal(renamed.body.required_intake_template_ids, undefined);
  assert.equal(configurationBlockers(["missing"], [], "org").length, 0);
  assert.equal(templateAccess({ templates: false }), "denied");
  assert.equal(templateAccess({ templates: { read: false } }), "denied");
  assert.equal(templateAccess(undefined), "unknown");
  assert.equal(readTemplatesIfAvailable(undefined, { templates: false }).length, 0);
  assert.throws(() => readTemplatesIfAvailable(undefined, { templates: { read: true } }), /template/);
  assert.deepEqual(absentTemplateIds(["old", "missing"], ctx().templates), ["missing"]);
  const shown = visibleTemplates(ctx().templates, "org", ["old"], []);
  assert.equal(shown.some(item => item.id === "old" && item.active === false), true);
  assert.equal(shown.some(item => item.id === "t1"), true);
  assert.equal(visibleTemplates(ctx().templates, "org", [], []).some(item => item.id === "old"), false);
  assert.equal(visibleTemplates([], "org", ["kept"], []).some(item => item.name.includes("record unavailable")), true);
  assert.equal(canMutatePrograms(true, "admin"), true);
  assert.equal(canMutatePrograms(true, "clinician"), false);
  assert.equal(canMutatePrograms(false, "admin"), false);
  assert.equal(historyInScope([{ organization_id: "org", program_id: "p1", program_service_id: null }], "program", "p1", "org"), true);
  assert.equal(historyInScope([{ organization_id: "org", program_id: "p2", program_service_id: null }], "program", "p1", "org"), false);
  assert.equal(historyInScope([{ organization_id: "other", program_id: "p1", program_service_id: null }], "program", "p1", "org"), false);
  assert.equal(historyInScope([{ organization_id: "org", program_id: "p1", program_service_id: "s1" }], "service", "s1", "org"), true);
  assert.equal(historyInScope([{ organization_id: "org", program_id: "p1", program_service_id: "s1" }], "service", "s2", "org"), false);
  assert.equal(staleScope(1, 1, "a", "a", "org", "org"), false);
  assert.equal(staleScope(1, 2, "a", "a", "org", "org"), true);
  assert.equal(staleScope(1, 1, "a", "b", "org", "org"), true);
  assert.equal(staleScope(1, 1, "a", "a", "org", "other"), true);
});
