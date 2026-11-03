import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import { date, label, money, request, value } from "./api";
import type { Bootstrap } from "./api";
import { Badge, Button, Empty, Icon, Panel, Tabs } from "./ui";
import {
  applyProgramUpdate,
  applyServiceUpdate,
  bounds,
  buildProgramCreate,
  buildProgramPatch,
  buildServiceCreate,
  buildServicePatch,
  catalogPath,
  censusDefinition,
  codeSystems,
  displaySnapshot,
  emptyProgramDraft,
  emptyServiceDraft,
  filterPrograms,
  isProgramDraft,
  levelsOfCare,
  parseHistory,
  parseProgram,
  parseProgramCatalog,
  parseService,
  programBlockers,
  programDraftFrom,
  programHistoryPath,
  programPath,
  absentTemplateIds,
  canMutatePrograms,
  historyInScope,
  readTemplatesIfAvailable,
  staleScope,
  templateAccess,
  visibleTemplates,
  replaceProgram,
  replaceService,
  serviceDraftFrom,
  serviceHistoryPath,
  servicePath,
  servicesForProgram,
  sessionKinds,
  toggleId,
} from "./programTypes";
import type { CareProgram, Check, FacilityRef, ProgramCatalog, ProgramDraft, ProgramService, Revision, ServiceDraft, TemplateRef } from "./programTypes";
import "./programSettings.css";

type Props = { data: Bootstrap; onChanged: () => Promise<void>; onDirtyChange: (dirty: boolean) => void };
type Editor =
  | { type: "program-create" }
  | { type: "program-edit"; program: CareProgram }
  | { type: "program-active"; program: CareProgram; active: boolean }
  | { type: "service-create"; programId: string }
  | { type: "service-edit"; service: ProgramService }
  | { type: "service-active"; service: ProgramService; active: boolean };
type HistoryView = { kind: "program" | "service"; id: string; title: string; revisions: Revision[] };

export default function ProgramSettings({ data, onChanged, onDirtyChange }: Props) {
  const [catalog, setCatalog] = useState<ProgramCatalog | null>(null);
  const [catalogActor, setCatalogActor] = useState("");
  const [catalogOrg, setCatalogOrg] = useState("");
  const [selectedId, setSelectedId] = useState("");
  const [serviceId, setServiceId] = useState("");
  const [facilityId, setFacilityId] = useState("all");
  const [status, setStatus] = useState<"all" | "active" | "inactive">("all");
  const [editor, setEditor] = useState<Editor | null>(null);
  const [draft, setDraft] = useState<ProgramDraft | ServiceDraft | null>(null);
  const [history, setHistory] = useState<HistoryView | null>(null);
  const [historyActor, setHistoryActor] = useState("");
  const [historyOrg, setHistoryOrg] = useState("");
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [loading, setLoading] = useState(true);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [discard, setDiscard] = useState(false);
  const generation = useRef(0);
  const historyRequest = useRef(0);
  const actorRef = useRef("");
  const orgRef = useRef("");
  actorRef.current = String(data.actor?.id ?? "");
  orgRef.current = value(data.actor, "organization_id");
  const actorId = actorRef.current;
  const orgId = orgRef.current;
  const catalogVisible = catalog && catalogActor === actorId && catalogOrg === orgId ? catalog : null;
  const historyVisible = history && historyActor === actorId && historyOrg === orgId ? history : null;
  const dirty = editor !== null || busy;
  const canWrite = !!catalogVisible && canMutatePrograms(catalogVisible.can_manage, value(data.actor, "role"));
  let templates: TemplateRef[] = [];
  let templateError = "";
  try { templates = readTemplatesIfAvailable(data.templates, data.permissions); } catch (e) { templateError = e instanceof Error ? e.message : "Templates could not be read."; }
  const facilities: FacilityRef[] = data.facilities.map(facility => ({ id: facility.id, name: value(facility, "name") || "Facility unavailable", active: facility.active === true }));
  const hasActiveFacility = facilities.some(facility => facility.active);
  const selected = catalogVisible?.programs.find(program => program.id === selectedId) ?? null;
  const programServices = selected && catalogVisible ? servicesForProgram(catalogVisible.services, selected.id) : [];
  const selectedService = programServices.find(service => service.id === serviceId) ?? null;
  const facilityPrograms = catalogVisible ? filterPrograms(catalogVisible.programs, facilityId, "all") : [];
  const rows = catalogVisible ? filterPrograms(catalogVisible.programs, facilityId, status) : [];

  async function load(): Promise<"applied" | "stale" | "error"> {
    const ticket = ++generation.current;
    const loadActor = actorRef.current;
    const loadOrg = orgRef.current;
    setLoading(true);
    if (!loadActor || !loadOrg) {
      if (ticket === generation.current) {
        setCatalog(null);
        setCatalogActor("");
        setCatalogOrg("");
        setError("Current operator is unavailable.");
        setLoading(false);
      }
      return "error";
    }
    try {
      const result = parseProgramCatalog(await request<unknown>(catalogPath), loadOrg);
      if (staleScope(ticket, generation.current, loadActor, actorRef.current, loadOrg, orgRef.current)) return "stale";
      setCatalog(result);
      setCatalogActor(loadActor);
      setCatalogOrg(loadOrg);
      setSelectedId(current => result.programs.some(program => program.id === current) ? current : result.programs[0]?.id ?? "");
      setError("");
      return "applied";
    } catch (e) {
      if (staleScope(ticket, generation.current, loadActor, actorRef.current, loadOrg, orgRef.current)) return "stale";
      setError(e instanceof Error ? e.message : "Program catalog could not be loaded.");
      return "error";
    } finally {
      if (ticket === generation.current) setLoading(false);
    }
  }
  async function refresh() {
    const status = await load();
    if (status !== "applied") return;
    try { await onChanged(); } catch { if (actorRef.current === actorId && orgRef.current === orgId) setError("Catalog refreshed. Workspace refresh failed."); }
  }
  useEffect(() => {
    setEditor(null);
    setDraft(null);
    setDiscard(false);
    setHistory(null);
    setHistoryActor("");
    setHistoryOrg("");
    setHistoryLoading(false);
    setNotice("");
    void load();
    return () => { generation.current += 1; historyRequest.current += 1; };
  }, [actorId, orgId]);
  useEffect(() => { onDirtyChange(dirty); return () => onDirtyChange(false); }, [dirty, onDirtyChange]);
  useEffect(() => {
    if (!dirty) return;
    const handler = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ""; };
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, [dirty]);
  useEffect(() => {
    if (!catalogVisible || !selected) return;
    setServiceId(current => programServices.some(service => service.id === current) ? current : "");
  }, [catalogVisible, selectedId]);

  function closeEditor() { setEditor(null); setDraft(null); setDiscard(false); setError(""); }
  function staffName(id: string) { return value(data.staff.find(staff => staff.id === id), "name") || "Staff record unavailable"; }
  function facilityName(id: string) { return facilities.find(facility => facility.id === id)?.name || "Facility unavailable"; }
  function templateLabel(id: string) {
    const template = templates.find(item => item.id === id);
    if (!template) return `Template ${id} (record unavailable)`;
    return template.active ? template.name : `${template.name} (inactive)`;
  }
  function nameOf(id: string) { return templates.find(item => item.id === id)?.name || facilities.find(item => item.id === id)?.name || ""; }
  function setProgramField<K extends keyof ProgramDraft>(key: K, fieldValue: ProgramDraft[K]) {
    setError("");
    setDraft(current => current && isProgramDraft(current) ? { ...current, [key]: fieldValue } : current);
  }
  function setServiceField<K extends keyof ServiceDraft>(key: K, fieldValue: ServiceDraft[K]) {
    setError("");
    setDraft(current => current && !isProgramDraft(current) ? { ...current, [key]: fieldValue } : current);
  }
  function openProgramCreate() {
    if (!canWrite || !hasActiveFacility || dirty) return;
    setNotice(""); setError(""); setDiscard(false);
    setEditor({ type: "program-create" });
    setDraft(emptyProgramDraft(facilities.find(facility => facility.active)?.id ?? ""));
  }
  function openProgramEdit(program: CareProgram) {
    if (!canWrite || dirty) return;
    setNotice(""); setError(""); setDiscard(false);
    setEditor({ type: "program-edit", program });
    setDraft(programDraftFrom(program));
  }
  function openProgramActive(program: CareProgram, active: boolean) {
    if (!canWrite || dirty || program.active === active) return;
    setNotice(""); setError(""); setDiscard(false);
    setEditor({ type: "program-active", program, active });
    setDraft({ ...programDraftFrom(program), active, change_reason: "" });
  }
  function openServiceCreate(program: CareProgram) {
    if (!canWrite || dirty || !program.active || !facilities.find(facility => facility.id === program.facility_id)?.active) return;
    setNotice(""); setError(""); setDiscard(false);
    setEditor({ type: "service-create", programId: program.id });
    setDraft(emptyServiceDraft(program.id));
  }
  function openServiceEdit(service: ProgramService) {
    if (!canWrite || dirty) return;
    setNotice(""); setError(""); setDiscard(false);
    setEditor({ type: "service-edit", service });
    setDraft(serviceDraftFrom(service));
  }
  function openServiceActive(service: ProgramService, active: boolean) {
    if (!canWrite || dirty || service.active === active) return;
    setNotice(""); setError(""); setDiscard(false);
    setEditor({ type: "service-active", service, active });
    setDraft({ ...serviceDraftFrom(service), active, change_reason: "" });
  }
  async function openHistory(kind: "program" | "service", id: string, title: string) {
    if (!canWrite || historyLoading) return;
    const ticket = ++historyRequest.current;
    const loadActor = actorRef.current;
    const loadOrg = orgRef.current;
    setHistory(null);
    setHistoryActor("");
    setHistoryOrg("");
    setHistoryLoading(true); setError("");
    try {
      const revisions = parseHistory(await request<unknown>(kind === "program" ? programHistoryPath(id) : serviceHistoryPath(id)), kind, loadOrg);
      if (ticket !== historyRequest.current || loadActor !== actorRef.current || loadOrg !== orgRef.current) return;
      if (!historyInScope(revisions, kind, id, loadOrg)) throw new Error("History belongs to another configuration record. Refresh before continuing.");
      setHistory({ kind, id, title, revisions });
      setHistoryActor(loadActor);
      setHistoryOrg(loadOrg);
    } catch (e) {
      if (ticket !== historyRequest.current || loadActor !== actorRef.current || loadOrg !== orgRef.current) return;
      setError(e instanceof Error ? e.message : "History could not be loaded.");
    } finally {
      if (ticket === historyRequest.current) setHistoryLoading(false);
    }
  }
  async function save(event: FormEvent) {
    event.preventDefault();
    if (!editor || !draft || !catalogVisible || busy) return;
    const writeCtx = { organizationId: orgId, templates, facilities, programs: catalogVisible.programs, services: catalogVisible.services };
    let method = "POST";
    let path = programPath();
    let targetId = "";
    let kind: "program" | "service" = "program";
    let checked: Check | null = null;
    if (editor.type === "program-create" && isProgramDraft(draft)) checked = buildProgramCreate(draft, writeCtx);
    else if ((editor.type === "program-edit" || editor.type === "program-active") && isProgramDraft(draft)) {
      method = "PATCH"; path = programPath(editor.program.id); targetId = editor.program.id; checked = buildProgramPatch(editor.program, draft, writeCtx);
    } else if (editor.type === "service-create" && !isProgramDraft(draft)) {
      kind = "service"; path = servicePath(); checked = buildServiceCreate(draft, writeCtx);
    } else if ((editor.type === "service-edit" || editor.type === "service-active") && !isProgramDraft(draft)) {
      kind = "service"; method = "PATCH"; path = servicePath(editor.service.id); targetId = editor.service.id; checked = buildServicePatch(editor.service, draft, writeCtx);
    }
    if (!checked) { setNotice(""); setError("This editor cannot be saved."); return; }
    if (!checked.ok) { setNotice(""); setError(checked.error); return; }
    const ticket = generation.current;
    const saveActor = actorRef.current;
    const saveOrg = orgRef.current;
    const previous = catalog;
    const previousActor = catalogActor;
    const previousOrg = catalogOrg;
    const body = checked.body;
    let optimistic = false;
    setBusy(true); setError(""); setNotice("");
    try {
      if (method === "PATCH" && previous) {
        setCatalog(kind === "service" ? applyServiceUpdate(previous, targetId, body) : applyProgramUpdate(previous, targetId, body));
        setCatalogActor(saveActor);
        setCatalogOrg(saveOrg);
        optimistic = true;
      }
      const payload = await request<unknown>(path, method, body);
      if (staleScope(ticket, generation.current, saveActor, actorRef.current, saveOrg, orgRef.current)) return;
      const parsed = kind === "service" ? parseService(payload, saveOrg) : parseProgram(payload, saveOrg);
      if (targetId && parsed.id !== targetId) throw new Error("The saved record did not match this edit. Refresh before continuing.");
      setCatalog(current => current ? (kind === "service" ? replaceService(current, parsed as ProgramService) : replaceProgram(current, parsed as CareProgram)) : current);
      setCatalogActor(saveActor);
      setCatalogOrg(saveOrg);
      setEditor(null); setDraft(null); setDiscard(false); setNotice("Saved."); setError("");
      const refreshed = await load();
      if (saveActor !== actorRef.current || saveOrg !== orgRef.current) return;
      if (refreshed === "error") setError("Saved. Refresh the catalog to confirm the latest record.");
      else {
        try { await onChanged(); } catch { if (actorRef.current === saveActor && orgRef.current === saveOrg) setError("Saved. Refresh the workspace to update other sections."); }
      }
    } catch (e) {
      if (!staleScope(ticket, generation.current, saveActor, actorRef.current, saveOrg, orgRef.current)) {
        if (optimistic) { setCatalog(previous); setCatalogActor(previousActor); setCatalogOrg(previousOrg); }
        setError(e instanceof Error ? e.message : "The program change could not be saved.");
      }
    } finally { setBusy(false); }
  }

  const programDraft = draft && isProgramDraft(draft) ? draft : null;
  const serviceDraft = draft && !isProgramDraft(draft) ? draft : null;
  const originalProgram = editor && (editor.type === "program-edit" || editor.type === "program-active") ? editor.program : null;
  const originalService = editor && (editor.type === "service-edit" || editor.type === "service-active") ? editor.service : null;
  function templateChoices(ids: string[], original: string[]) {
    return visibleTemplates(templates, orgId, ids, original);
  }
  function templatePicker(title: string, ids: string[], original: string[], onToggle: (id: string) => void) {
    const choices = templateChoices(ids, original);
    return <div className="form-field full"><span className="program-label" id={`program-${title}`}>{title}</span>{!choices.length && <small>{templateAccess(data.permissions) === "denied" ? "Template records are unavailable." : "No template records to select. That is not a configuration error."}</small>}<div className="program-checks" role="group" aria-labelledby={`program-${title}`}>{choices.map(template => <label key={template.id}><input type="checkbox" checked={ids.includes(template.id)} onChange={() => onToggle(template.id)} /><span>{template.name}{template.active ? "" : " (inactive, still required)"}</span></label>)}</div></div>;
  }

  return <div className="program-settings">
    <Panel title="Programs" subtitle={censusDefinition} action={<div className="program-actions"><Button variant="secondary small" disabled={dirty || loading} onClick={() => void refresh()}><Icon name="refresh" size={16} /> Refresh</Button>{canWrite && hasActiveFacility && <Button disabled={dirty || loading} onClick={openProgramCreate}><Icon name="plus" size={16} /> Add program</Button>}</div>}>
      {catalogVisible && <Tabs tabs={[{ id: "all", label: "All", count: facilityPrograms.length }, { id: "active", label: "Active", count: facilityPrograms.filter(program => program.active).length }, { id: "inactive", label: "Inactive", count: facilityPrograms.filter(program => !program.active).length }]} value={status} onChange={id => { if (id === "all" || id === "active" || id === "inactive") setStatus(id); }} />}
      <div className="program-filters"><label htmlFor="program-facility-filter">Facility<select id="program-facility-filter" value={facilityId} onChange={event => setFacilityId(event.target.value)}><option value="all">All facilities</option>{facilities.map(facility => <option key={facility.id} value={facility.id}>{facility.name}{facility.active ? "" : " (inactive)"}</option>)}</select></label></div>
      {error && <div className="error-banner" role="alert">{error}</div>}
      {templateError && <div className="error-banner" role="alert">{templateError}</div>}
      {notice && <p className="program-notice" role="status">{notice}</p>}
      {catalogVisible && !canWrite && <p className="program-note">Review only. Creating, editing, retiring, and history require an active administrator.</p>}
      {canWrite && !hasActiveFacility && <p className="program-note">An active facility is required to add a program.</p>}
      {loading && !catalogVisible && <p className="program-notice" role="status">Loading program catalog…</p>}
      {!loading && !catalogVisible && <Empty icon="settings" title="Program catalog unavailable" description="Refresh to load programs, capacity, and services." action={<Button variant="secondary small" disabled={dirty} onClick={() => void refresh()}>Refresh</Button>} />}
      {catalogVisible && !rows.length && <Empty icon="settings" title={catalogVisible.programs.length ? "No matching programs" : "No programs are configured"} description={catalogVisible.programs.length ? "No programs match these filters." : "Configured programs will appear here."} action={catalogVisible.programs.length ? <Button variant="secondary small" onClick={() => { setFacilityId("all"); setStatus("all"); }}>Clear filters</Button> : undefined} />}
      {catalogVisible && !!rows.length && <div className="table-wrap"><table><caption className="sr-only">Care programs. {censusDefinition}</caption><thead><tr><th scope="col">Program</th><th scope="col">Facility</th><th scope="col">Level</th><th scope="col">Ages</th><th scope="col">Capacity</th><th scope="col" title={censusDefinition}>Active census</th><th scope="col" title={censusDefinition}>Free places</th><th scope="col">Status</th><th scope="col">Actions</th></tr></thead><tbody>{rows.map(program => <tr key={program.id} className={program.id === selectedId ? "selected clickable" : "clickable"} aria-selected={program.id === selectedId} tabIndex={0} onClick={() => setSelectedId(program.id)} onKeyDown={event => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); setSelectedId(program.id); } }}><td><strong>{program.name}</strong><small>{program.code} · v{program.version}</small></td><td>{facilityName(program.facility_id)}</td><td>{label(program.level_of_care)}</td><td>{program.min_age}{program.max_age == null ? "+" : `–${program.max_age}`}</td><td>{program.capacity}</td><td>{program.active_census}</td><td>{program.available_places}</td><td><Badge status={program.active ? "active" : "inactive"} /></td><td><div className="program-actions" onClick={event => event.stopPropagation()}><Button variant="text small" onClick={() => setSelectedId(program.id)}>Details</Button>{canWrite && <><Button variant="text small" disabled={dirty || loading} onClick={() => openProgramEdit(program)}>Edit</Button><Button variant="text small" disabled={dirty || loading} onClick={() => openProgramActive(program, !program.active)}>{program.active ? "Retire" : "Reactivate"}</Button><Button variant="text small" disabled={historyLoading} onClick={() => void openHistory("program", program.id, `${program.code} · ${program.name}`)}>History</Button></>}</div></td></tr>)}</tbody></table></div>}
      {selected && <div className="section-summary"><div><span>Active census</span><strong>{selected.active_census}</strong></div><div><span>Free places</span><strong>{selected.available_places}</strong></div><div><span>Capacity</span><strong>{selected.capacity}</strong></div><div><span>Version</span><strong>{selected.version}</strong></div></div>}
      {selected && <dl className="detail-list"><div><dt>Facility</dt><dd>{facilityName(selected.facility_id)}</dd></div><div><dt>Code and level</dt><dd>{selected.code} · {label(selected.level_of_care)}</dd></div><div><dt>Ages</dt><dd>{selected.min_age}{selected.max_age == null ? " and older" : `–${selected.max_age}`}</dd></div><div><dt>Updated</dt><dd>{date(selected.updated_at, true)}</dd></div><div><dt>Intake forms</dt><dd>{selected.required_intake_template_ids.length ? selected.required_intake_template_ids.map(templateLabel).join(", ") : "None"}</dd></div><div><dt>Discharge forms</dt><dd>{selected.required_discharge_template_ids.length ? selected.required_discharge_template_ids.map(templateLabel).join(", ") : "None"}</dd></div></dl>}
      {selected && !!programBlockers(selected, templates, orgId).length && <div className="program-blockers"><strong>Configuration blockers</strong><ul>{programBlockers(selected, templates, orgId).map(item => <li key={item}>{item}</li>)}</ul></div>}
      {selected && absentTemplateIds([...selected.required_intake_template_ids, ...selected.required_discharge_template_ids], templates).length > 0 && <p className="program-note">Some required template records are not loaded. That is not proof of misconfiguration, and the requirements stay listed.</p>}
      {selected && <div className="program-services"><div className="program-services-head"><h3>Services</h3>{canWrite && selected.active && facilities.find(facility => facility.id === selected.facility_id)?.active ? <Button variant="secondary small" disabled={dirty || loading} onClick={() => openServiceCreate(selected)}><Icon name="plus" size={16} /> Add service</Button> : canWrite ? <p>Add a service when the program and facility are active.</p> : null}</div>{!programServices.length && <p className="program-note">No services for this program.</p>}<div className="table-wrap"><table><caption className="sr-only">Services for {selected.name}</caption><thead><tr><th scope="col">Service</th><th scope="col">Kind</th><th scope="col">Minutes</th><th scope="col">Charge</th><th scope="col">Note template</th><th scope="col">Status</th><th scope="col">Actions</th></tr></thead><tbody>{programServices.map(service => <tr key={service.id} className={service.id === serviceId ? "selected clickable" : "clickable"} onClick={() => setServiceId(service.id)}><td><strong>{service.name}</strong><small>{service.code_system.toUpperCase()} {service.service_code} · v{service.version}</small></td><td>{label(service.session_kind)}</td><td>{service.min_minutes}–{service.default_minutes}–{service.max_minutes}</td><td>{service.default_charge_cents == null ? "None" : money(service.default_charge_cents)}</td><td>{service.note_template_id ? templateLabel(service.note_template_id) : "None"}</td><td><Badge status={service.active ? "active" : "inactive"} /></td><td><div className="program-actions" onClick={event => event.stopPropagation()}><Button variant="text small" onClick={() => setServiceId(service.id)}>Details</Button>{canWrite && <><Button variant="text small" disabled={dirty || loading} onClick={() => openServiceEdit(service)}>Edit</Button><Button variant="text small" disabled={dirty || loading || (!service.active && (!selected.active || !facilities.find(facility => facility.id === selected.facility_id)?.active))} onClick={() => openServiceActive(service, !service.active)}>{service.active ? "Retire" : "Reactivate"}</Button><Button variant="text small" disabled={historyLoading} onClick={() => void openHistory("service", service.id, `${service.service_code} · ${service.name}`)}>History</Button></>}</div></td></tr>)}</tbody></table></div>{selectedService && <dl className="detail-list"><div><dt>Program</dt><dd>{selected.code} · {selected.name}</dd></div><div><dt>Code</dt><dd>{selectedService.code_system.toUpperCase()} {selectedService.service_code}</dd></div><div><dt>Session kind</dt><dd>{label(selectedService.session_kind)}</dd></div><div><dt>Minutes</dt><dd>{selectedService.min_minutes} minimum, {selectedService.default_minutes} default, {selectedService.max_minutes} maximum</dd></div><div><dt>Default charge</dt><dd>{selectedService.default_charge_cents == null ? "None" : money(selectedService.default_charge_cents)}</dd></div><div><dt>Note template</dt><dd>{selectedService.note_template_id ? templateLabel(selectedService.note_template_id) : "None"}</dd></div><div><dt>Updated</dt><dd>{date(selectedService.updated_at, true)}</dd></div></dl>}</div>}
    </Panel>
    {editor && programDraft && <Panel title={editor.type === "program-create" ? "Add program" : editor.type === "program-edit" ? "Edit program" : programDraft.active ? "Reactivate program" : "Retire program"} subtitle={editor.type === "program-active" ? (programDraft.active ? "Admission can use this program while it and its facility are active." : "New intake and bookings stop. Existing visits stay on the record.") : undefined}>
      <form onSubmit={save} className="program-form"><fieldset disabled={busy}>
        {editor.type === "program-create" && <div className="form-grid"><div className="form-field"><label htmlFor="program-facility">Facility *</label><select id="program-facility" required value={programDraft.facility_id} onChange={event => setProgramField("facility_id", event.target.value)}><option value="">Select facility</option>{facilities.filter(facility => facility.active).map(facility => <option key={facility.id} value={facility.id}>{facility.name}</option>)}</select></div><div className="form-field"><label htmlFor="program-code">Code *</label><input id="program-code" required maxLength={bounds.codeMax} value={programDraft.code} onChange={event => setProgramField("code", event.target.value)} /></div><div className="form-field"><label htmlFor="program-level">Level of care *</label><select id="program-level" required value={programDraft.level_of_care} onChange={event => setProgramField("level_of_care", event.target.value)}><option value="">Select level</option>{levelsOfCare.map(level => <option key={level} value={level}>{label(level)}</option>)}</select></div></div>}
        {originalProgram && <p className="program-locked">{facilityName(originalProgram.facility_id)} · {originalProgram.code} · {label(originalProgram.level_of_care)}. Facility, code, and level of care do not change.</p>}
        {editor.type !== "program-active" && <div className="form-grid"><div className="form-field"><label htmlFor="program-name">Name *</label><input id="program-name" required maxLength={bounds.nameMax} value={programDraft.name} onChange={event => setProgramField("name", event.target.value)} /></div><div className="form-field"><label htmlFor="program-min-age">Minimum age *</label><input id="program-min-age" required type="number" min={bounds.minAge} max={bounds.maxMinAge} step={1} value={programDraft.min_age} onChange={event => setProgramField("min_age", event.target.value)} /><small>18 to 120.</small></div><div className="form-field"><label htmlFor="program-max-age">Maximum age</label><input id="program-max-age" type="number" min={bounds.minAge} max={bounds.maxAge} step={1} value={programDraft.max_age} onChange={event => setProgramField("max_age", event.target.value)} /><small>Optional. Minimum age through 130.</small></div><div className="form-field"><label htmlFor="program-capacity">Capacity *</label><input id="program-capacity" required type="number" min={bounds.minCapacity} max={bounds.maxCapacity} step={1} value={programDraft.capacity} onChange={event => setProgramField("capacity", event.target.value)} /><small>1 to 10000.{originalProgram ? ` Active census is ${originalProgram.active_census}.` : ""}</small></div><div className="form-field checkbox-field"><label htmlFor="program-active"><input id="program-active" type="checkbox" checked={programDraft.active} onChange={event => setProgramField("active", event.target.checked)} />Active</label></div>{templatePicker("Intake forms", programDraft.intake_ids, originalProgram?.required_intake_template_ids ?? [], id => setProgramField("intake_ids", toggleId(programDraft.intake_ids, id)))}{templatePicker("Discharge forms", programDraft.discharge_ids, originalProgram?.required_discharge_template_ids ?? [], id => setProgramField("discharge_ids", toggleId(programDraft.discharge_ids, id)))}</div>}
        <div className="form-field"><label htmlFor="program-reason">Change reason *</label><textarea id="program-reason" required minLength={bounds.reasonMin} maxLength={bounds.reasonMax} rows={3} value={programDraft.change_reason} onChange={event => setProgramField("change_reason", event.target.value)} /><small>10 to 2000 characters.</small></div>
        <div className="program-actions"><Button type="submit" disabled={busy}>{busy ? "Saving…" : editor.type === "program-active" ? (programDraft.active ? "Reactivate program" : "Retire program") : "Save program"}</Button><Button variant="text" disabled={busy} onClick={() => setDiscard(true)}>Discard changes</Button></div>
        {discard && <div className="program-discard" role="alert"><p>Discard unsaved program changes?</p><Button variant="secondary small" onClick={closeEditor}>Discard changes</Button><Button variant="text small" onClick={() => setDiscard(false)}>Keep editing</Button><Button variant="text small" disabled={busy} onClick={() => { closeEditor(); void refresh(); }}>Discard and refresh</Button></div>}
      </fieldset></form>
    </Panel>}
    {editor && serviceDraft && <Panel title={editor.type === "service-create" ? "Add service" : editor.type === "service-edit" ? "Edit service" : serviceDraft.active ? "Reactivate service" : "Retire service"} subtitle={editor.type === "service-active" && !serviceDraft.active ? "Booked visits keep their saved service snapshot." : undefined}>
      <form onSubmit={save} className="program-form"><fieldset disabled={busy}>
        {originalService && <p className="program-locked">{originalService.code_system.toUpperCase()} {originalService.service_code} · {label(originalService.session_kind)}. Program, code system, code, and session kind do not change.</p>}
        {editor.type !== "service-active" && <div className="form-grid">{editor.type === "service-create" && <><div className="form-field"><label htmlFor="service-system">Code system *</label><select id="service-system" required value={serviceDraft.code_system} onChange={event => setServiceField("code_system", event.target.value)}><option value="">Select code system</option>{codeSystems.map(system => <option key={system} value={system}>{system.toUpperCase()}</option>)}</select></div><div className="form-field"><label htmlFor="service-code">Service code *</label><input id="service-code" required maxLength={bounds.codeMax} value={serviceDraft.service_code} onChange={event => setServiceField("service_code", event.target.value)} /></div><div className="form-field"><label htmlFor="service-kind">Session kind *</label><select id="service-kind" required value={serviceDraft.session_kind} onChange={event => setServiceField("session_kind", event.target.value)}><option value="">Select session kind</option>{sessionKinds.map(kind => <option key={kind} value={kind}>{label(kind)}</option>)}</select></div></>}<div className="form-field"><label htmlFor="service-name">Name *</label><input id="service-name" required maxLength={bounds.nameMax} value={serviceDraft.name} onChange={event => setServiceField("name", event.target.value)} /></div><div className="form-field"><label htmlFor="service-min">Minimum minutes *</label><input id="service-min" required type="number" min={1} max={bounds.maxMinutes} step={1} value={serviceDraft.min_minutes} onChange={event => setServiceField("min_minutes", event.target.value)} /></div><div className="form-field"><label htmlFor="service-default">Default minutes *</label><input id="service-default" required type="number" min={1} max={bounds.maxMinutes} step={1} value={serviceDraft.default_minutes} onChange={event => setServiceField("default_minutes", event.target.value)} /></div><div className="form-field"><label htmlFor="service-max">Maximum minutes *</label><input id="service-max" required type="number" min={1} max={bounds.maxMinutes} step={1} value={serviceDraft.max_minutes} onChange={event => setServiceField("max_minutes", event.target.value)} /><small>1 to 480. Minimum, then default, then maximum.</small></div><div className="form-field"><label htmlFor="service-charge">Default charge (dollars)</label><input id="service-charge" inputMode="decimal" autoComplete="off" maxLength={16} value={serviceDraft.default_charge_cents} onChange={event => setServiceField("default_charge_cents", event.target.value)} /><small>Optional. Leave blank for no default; enter 0.00 for zero. Exact cents only. Suggested amount, not a billed charge.</small></div><div className="form-field"><label htmlFor="service-template">Note template</label><select id="service-template" value={serviceDraft.note_template_id} onChange={event => setServiceField("note_template_id", event.target.value)}><option value="">No note template</option>{templateChoices(serviceDraft.note_template_id ? [serviceDraft.note_template_id] : [], originalService?.note_template_id ? [originalService.note_template_id] : []).map(template => <option key={template.id} value={template.id}>{template.name}{template.active ? "" : " (inactive)"}</option>)}</select></div><div className="form-field checkbox-field"><label htmlFor="service-active"><input id="service-active" type="checkbox" checked={serviceDraft.active} onChange={event => setServiceField("active", event.target.checked)} />Active</label></div></div>}
        <div className="form-field"><label htmlFor="service-reason">Change reason *</label><textarea id="service-reason" required minLength={bounds.reasonMin} maxLength={bounds.reasonMax} rows={3} value={serviceDraft.change_reason} onChange={event => setServiceField("change_reason", event.target.value)} /><small>10 to 2000 characters.</small></div>
        <div className="program-actions"><Button type="submit" disabled={busy}>{busy ? "Saving…" : editor.type === "service-active" ? (serviceDraft.active ? "Reactivate service" : "Retire service") : "Save service"}</Button><Button variant="text" disabled={busy} onClick={() => setDiscard(true)}>Discard changes</Button></div>
        {discard && <div className="program-discard" role="alert"><p>Discard unsaved service changes?</p><Button variant="secondary small" onClick={closeEditor}>Discard changes</Button><Button variant="text small" onClick={() => setDiscard(false)}>Keep editing</Button></div>}
      </fieldset></form>
    </Panel>}
    {historyVisible && <Panel title="Configuration history" subtitle={historyVisible.title} action={<Button variant="text small" onClick={() => setHistory(null)}>Close history</Button>}>
      {!historyVisible.revisions.length && <p className="program-note">No revisions returned.</p>}
      <div className="program-history">{historyVisible.revisions.map((revision, index) => <details key={revision.id} open={index === 0}><summary>Version {revision.version} · {staffName(revision.actor_id)} · {date(revision.created_at, true)}</summary><p>{revision.reason}</p><small>{revision.actor_id}</small><dl className="program-snapshot">{displaySnapshot(revision.snapshot, nameOf).map(line => <div key={line.label}><dt>{line.label}</dt><dd>{line.value}</dd></div>)}{!displaySnapshot(revision.snapshot, nameOf).length && <div><dt>Snapshot</dt><dd>No configuration fields</dd></div>}</dl></details>)}</div>
    </Panel>}
  </div>;
}
