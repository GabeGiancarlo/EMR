import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import { date, label, request, value } from "./api";
import type { Bootstrap, Row } from "./api";
import { facilityLocalToday } from "./adult";
import { Badge, Button, Empty, Icon, Panel } from "./ui";
import { allowedIntakeTransitions, isOpenIntake, parseIntake, transitionBlockers } from "./intakeTypes";
import type { IntakeCase, IntakeData, IntakeStatus } from "./intakeTypes";
import "./intake.css";

type Props = { clientId: string; data: Bootstrap; onChanged: () => Promise<void>; onOpenForms: (episodeId: string) => void; onOpenConsent: () => void; onEditPatient: () => void; onDirtyChange: (dirty: boolean) => void };
type Editor = { kind: "assign"; row:Row } | { kind: "create" } | { kind: "identity" } | { kind: "edit"; row: IntakeCase } | { kind: "transition"; row: IntakeCase; to: IntakeStatus };
const actions: Record<IntakeStatus, string> = { open: "Open", screening: "Begin screening", ready: "Mark ready", admitted: "Admit to care", declined: "Decline intake", withdrawn: "Record withdrawal" };

export default function PatientIntake({ clientId, data, onChanged, onOpenForms, onOpenConsent, onEditPatient, onDirtyChange }: Props) {
  const [state, setState] = useState<IntakeData | null>(null);
  const [selected, setSelected] = useState("");
  const [editor, setEditor] = useState<Editor | null>(null);
  const [fields, setFields] = useState<Record<string, string>>({});
  const [attested, setAttested] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [discard, setDiscard] = useState(false);
  const generation = useRef(0);
  const dirty = editor !== null || busy;
  const patient = data.clients.find(c => c.id === clientId);
  const role = value(data.actor, "role");
  const eligibleStaff = data.staff.filter(s => s.active === true && ["admin", "clinical_lead", "clinician", "support"].includes(value(s, "role")) && (s.role !== "clinician" || s.id === patient?.primary_clinician_id));
  const facilities = data.facilities.filter(f => f.active === true);
  const programs=(data.care_programs || []).filter(p=>p.active===true && facilities.some(f=>f.id===p.facility_id));
  const chosenProgram=programs.find(p=>p.id===fields.program_id);
  const olderPlanned=data.episodes.filter(e=>e.client_id===clientId && e.status==="planned" && !state?.cases.some(c=>c.episode_id===e.id) && e.facility_id===chosenProgram?.facility_id && e.level_of_care===chosenProgram?.level_of_care && (!e.program_id || e.program_id===chosenProgram?.id));
  const programOptions=(editor?.kind==="assign" ? programs.filter(p=>p.facility_id===editor.row.facility_id && p.level_of_care===editor.row.level_of_care) : programs).map(p=>({id:p.id,name:`${value(p,"name")} · ${label(value(p,"level_of_care"))} · ${value(data.facilities.find(f=>f.id===p.facility_id),"name")} · ${value(p,"available_places")} places available`}));
  const staffName = (id: string) => value(data.staff.find(s => s.id === id), "name") || "Staff record unavailable";
  const facilityName = (id: string) => value(data.facilities.find(f => f.id === id), "name") || "Facility unavailable";
  const active = state?.cases.find(c => c.id === selected) ?? state?.cases[0];
  const confirmed = state?.identity_checks.some(c => c.current && c.outcome === "confirmed");

  async function load() {
    const ticket = ++generation.current;
    setLoading(true); setError("");
    try {
      const result = parseIntake(await request<unknown>(`/api/clients/${clientId}/intake`), clientId);
      if (ticket === generation.current) { setState(result); }
    } catch (e) { if (ticket === generation.current) { setState(null); setError(e instanceof Error ? e.message : "Intake could not be loaded."); } }
    finally { if (ticket === generation.current) setLoading(false); }
  }
  useEffect(() => { void load(); return () => { generation.current++; }; }, [clientId, data.actor?.id, patient?.updated_at]);
  useEffect(() => { onDirtyChange(dirty); return () => onDirtyChange(false); }, [dirty, onDirtyChange]);
  useEffect(() => { if (!dirty) return; const handler = (e: BeforeUnloadEvent) => { e.preventDefault(); e.returnValue = ""; }; window.addEventListener("beforeunload", handler); return () => window.removeEventListener("beforeunload", handler); }, [dirty]);

  function open(next: Editor) {
    setEditor(next); setError(""); setNotice(""); setAttested(false); setDiscard(false);
    const facilityId = value(patient, "facility_id");
    const today = facilityLocalToday(value(data.facilities.find(f => f.id === facilityId), "timezone"));
    if (next.kind === "create") setFields({ ...(data.program_configuration ? {program_id:"",legacy_episode_id:""} : {}), facility_id: facilities.some(f => f.id === facilityId) ? facilityId : "", level_of_care: "", starts_on: today, received_on: today, owner_id: data.actor?.id || "", referral_type: "", referrer_name: "", referrer_contact: "", referral_reason: "" });
    else if (next.kind === "edit") setFields({ owner_id: next.row.owner_id, referrer_name: next.row.referrer_name, referrer_contact: next.row.referrer_contact, referral_reason: next.row.referral_reason });
    else setFields({ reason: "", method: "", outcome: "" });
  }
  function close() { setEditor(null); setFields({}); setAttested(false); setDiscard(false); setError(""); }
  const change = (key: string, val: string) => setFields(f => {
    const next={...f,[key]:val};
    if(key==="program_id")next.legacy_episode_id="";
    if(key==="legacy_episode_id" && val){const prior=olderPlanned.find(e=>e.id===val);if(prior)next.starts_on=value(prior,"start_date");}
    return next;
  });
  async function save(e: FormEvent) {
    e.preventDefault(); if (!editor || busy) return;
    if ((editor.kind === "identity" || editor.kind === "transition" || editor.kind === "assign") && (!attested || (fields.reason || "").trim().length < 10)) { setError("Describe the review in at least 10 characters and confirm your attestation."); return; }
    setBusy(true); setError(""); const ticket = generation.current;
    try {
      let path = `/api/clients/${clientId}/intake`; let method = "POST"; let body: unknown = fields;
      if(editor.kind==="create" && data.program_configuration){const {facility_id:_facility,level_of_care:_level,...rest}=fields;body={...rest,legacy_episode_id:fields.legacy_episode_id || null};}
      if(editor.kind==="assign"){path=`/api/episodes/${editor.row.id}/assign-program`;body={program_id:fields.program_id,reason:fields.reason,attested,_expected_updated_at:editor.row.updated_at};}
      if (editor.kind === "identity") { path = `/api/clients/${clientId}/identity-checks`; body = { method: fields.method, outcome: fields.outcome, reason: fields.reason.trim(), attested }; }
      if (editor.kind === "edit") { path = `/api/intake-cases/${editor.row.id}`; method = "PATCH"; body = { ...fields, _expected_updated_at: editor.row.updated_at }; }
      if (editor.kind === "transition") { path = `/api/intake-cases/${editor.row.id}/transition`; body = { to_status: editor.to, reason: fields.reason.trim(), attested, _expected_updated_at: editor.row.updated_at }; }
      const result = await request<{ id: string }>(path, method, body);
      if (ticket !== generation.current) return;
      if (editor.kind === "create") setSelected(result.id);
      close(); setNotice("Saved to the patient record.");
      await load();
      try { await onChanged(); } catch { setError("The intake was saved. Refresh the workspace to update the other chart sections."); }
    } catch (e) { if (ticket === generation.current) setError(e instanceof Error ? e.message : "The intake action could not be saved."); }
    finally { setBusy(false); }
  }
  const textField = (key: string, title: string, required = true, maxLength = 200, type = "text") => <div className="form-field"><label htmlFor={`intake-${key}`}>{title}{required ? " *" : ""}</label><input id={`intake-${key}`} type={type} required={required} maxLength={maxLength} value={fields[key] || ""} onChange={e => change(key, e.target.value)} /></div>;
  const selectField = (key: string, title: string, options: { id: string; name: string }[]) => <div className="form-field"><label htmlFor={`intake-${key}`}>{title} *</label><select id={`intake-${key}`} required value={fields[key] || ""} onChange={e => change(key, e.target.value)}><option value="">Select {title.toLowerCase()}</option>{options.map(o => <option key={o.id} value={o.id}>{o.name}</option>)}</select></div>;
  const narrativeField = (key: string, title: string, minLength = 1) => <div className="form-field"><label htmlFor={`intake-${key}`}>{title} *</label><textarea id={`intake-${key}`} required minLength={minLength} maxLength={2000} rows={3} value={fields[key] || ""} onChange={e => change(key, e.target.value)} /></div>;

  return <div className="patient-intake">
    <Panel title="Intake & admission" subtitle="Carry the referral, identity review, and required documents into the next course of care." action={<Button variant="secondary small" disabled={dirty || loading} onClick={() => void load()}><Icon name="refresh" size={16} /> Refresh intake</Button>}>
      <div className="intake-summary"><div><span className="eyebrow">PATIENT IDENTITY</span><strong>{value(patient, "mrn")}</strong><span>{confirmed ? "Current identity confirmed by staff" : "Identity review needed"}</span></div><Button variant="secondary small" disabled={dirty || loading || !state} onClick={() => open({ kind: "identity" })}>Record identity check</Button></div>
      {error && <div className="error-banner" role="alert">{error}</div>}
      {notice && <p className="intake-notice" role="status">{notice}</p>}
      {loading && <p role="status" className="intake-notice">Loading intake history…</p>}
      {!loading && !state && <Empty title="Intake unavailable" description="Refresh intake to retrieve the complete case history before making changes." />}
      {state && !editor && <div className="intake-case-picker"><label htmlFor="intake-case">Intake history</label><select id="intake-case" value={active?.id || ""} onChange={e => setSelected(e.target.value)} disabled={loading || busy || !state.cases.length}>{!state.cases.length && <option value="">No intake cases</option>}{state.cases.map(c => <option key={c.id} value={c.id}>{date(c.received_on)} · {label(c.level_of_care)} · {label(c.status)}</option>)}</select><Button disabled={loading || busy || state.cases.some(c => isOpenIntake(c.status)) || (data.program_configuration && !programs.length)} onClick={() => open({ kind: "create" })}><Icon name="plus" size={16} /> Start intake</Button></div>}
      {data.program_configuration && !programs.length && <p className="intake-notice">An administrator must configure an active care program in Settings → Programs before a new intake can start.</p>}
      {state && !state.cases.length && !editor && <Empty title="Begin the next chapter of care" description="Create an intake case to capture the referral and intended program. Documents and admission checks will stay connected to that course of care." />}
    </Panel>
    {editor && <Panel title={editor.kind === "identity" ? "Confirm the patient’s identity" : editor.kind === "create" ? "New referral & planned care" : editor.kind === "edit" ? "Edit referral details" : editor.kind === "assign" ? "Assign the existing care program" : actions[editor.to]} subtitle={editor.kind === "identity" ? "Record what you reviewed. Do not enter identity-document numbers." : undefined}>
      <form onSubmit={save} className="intake-editor"><fieldset disabled={busy}>
        {chosenProgram && (editor.kind==="create" || editor.kind==="assign") && <p>Program ages: {value(chosenProgram,"min_age")}–{chosenProgram.max_age==null?"no upper limit":value(chosenProgram,"max_age")}. Capacity: {value(chosenProgram,"capacity")} active care places. Planned intake does not reserve a place.</p>}
        {(editor.kind === "create" || editor.kind === "edit") && <>
          <div className="form-grid">{editor.kind === "create" && <>{data.program_configuration ? selectField("program_id","Care program",programOptions) : <>{selectField("facility_id", "Facility", facilities.map(f => ({ id: f.id, name: value(f, "name") })))}{selectField("level_of_care", "Level of care", ["outpatient", "iop", "php"].map(id => ({ id, name: label(id) })))}</>}{textField("starts_on", "Intended start", true, 10, "date")}{textField("received_on", "Referral received", true, 10, "date")}{selectField("referral_type", "Referral source", ["self", "provider", "facility", "family", "justice", "other"].map(id => ({ id, name: label(id) })))}</>}{selectField("owner_id", "Intake owner", eligibleStaff.map(s => ({ id: s.id, name: value(s, "name") })))}{textField("referrer_name", "Referrer name", false)}{textField("referrer_contact", "Referrer contact", false)}</div>{editor.kind==="create" && data.program_configuration && olderPlanned.length>0 && <div className="form-field"><label htmlFor="intake-legacy-episode">Existing planned episode</label><select id="intake-legacy-episode" value={fields.legacy_episode_id || ""} onChange={e=>change("legacy_episode_id",e.target.value)}><option value="">Create a new planned episode</option>{olderPlanned.map(e=><option key={e.id} value={e.id}>{label(value(e,"level_of_care"))} · {date(e.start_date)}</option>)}</select><small>Continuing an older episode preserves its original identifier and start date.</small></div>}{narrativeField("referral_reason", "Reason for referral")}
        </>}
        {editor.kind === "assign" && <>{selectField("program_id","Care program",programOptions)}{narrativeField("reason","Assignment reason",10)}<p>The original episode, care status, and dates will be preserved. This assignment cannot be remapped.</p></>}
        {editor.kind === "identity" && <><div className="intake-identifiers"><strong>{value(patient, "first_name")} {value(patient, "last_name")}</strong><span>DOB {date(patient?.birth_date)} · MRN {value(patient, "mrn")}</span></div><div className="form-grid">{selectField("method", "Review method", [{ id: "patient_report", name: "Patient report" }, { id: "document_review", name: "Document review" }, { id: "prior_record", name: "Prior record" }])}{selectField("outcome", "Review outcome", [{ id: "confirmed", name: "Identity confirmed" }, { id: "unable", name: "Unable to confirm" }])}</div>{narrativeField("reason", "Review notes", 10)}</>}
        {editor.kind === "transition" && <><p>{label(editor.row.status)} → {label(editor.to)} · {label(editor.row.level_of_care)} · {facilityName(editor.row.facility_id)}</p>{editor.to === "admitted" && <p>This activates the linked care episode. Confirm the patient and program before recording admission.</p>}{narrativeField("reason", "Decision and reason", 10)}</>}
        {(editor.kind === "transition" || editor.kind === "identity" || editor.kind === "assign") && <label className="intake-attestation"><input type="checkbox" checked={attested} onChange={e => setAttested(e.target.checked)} required />I performed this review and confirm the information recorded above.</label>}
        <div className="intake-actions"><Button type="submit" disabled={busy}>{busy ? "Saving…" : editor.kind === "identity" ? "Save identity check" : editor.kind === "transition" ? actions[editor.to] : editor.kind === "assign" ? "Assign program" : "Save intake"}</Button><Button variant="text" disabled={busy} onClick={() => setDiscard(true)}>Discard changes</Button></div>
        {discard && <div className="intake-discard" role="alert"><p>Discard the information entered in this form?</p><Button variant="secondary small" onClick={close}>Discard this form</Button><Button variant="text small" onClick={() => setDiscard(false)}>Keep editing</Button></div>}
      </fieldset></form>
    </Panel>}
    {active && <>
      <div className="intake-columns"><Panel title="Referral & care context" action={<Badge status={active.status} />}>
        <dl className="intake-details"><div><dt>Program</dt><dd>{active.program_name ? `${active.program_name} · v${active.program_version} · ` : "Program assignment needed · "}{label(active.level_of_care)} · {facilityName(active.facility_id)}</dd></div><div><dt>Intended start</dt><dd>{date(active.starts_on)}</dd></div><div><dt>Intake owner</dt><dd>{staffName(active.owner_id)}</dd></div><div><dt>Referral received</dt><dd>{date(active.received_on)} · {label(active.referral_type)}</dd></div><div><dt>Referred by</dt><dd>{active.referrer_name || "Not provided"}{active.referrer_contact && <small>{active.referrer_contact}</small>}</dd></div><div><dt>Reason</dt><dd>{active.referral_reason}</dd></div></dl>
        {isOpenIntake(active.status) && <div className="intake-actions"><Button variant="secondary small" disabled={dirty || loading} onClick={() => open({ kind: "edit", row: active })}>Edit referral</Button></div>}
      </Panel><Panel title={isOpenIntake(active.status) ? "Readiness checklist" : "Current record checks"} subtitle={isOpenIntake(active.status) ? "Required organization forms and record checks are evaluated again when you mark ready or admit." : "These reflect the record today. The recorded disposition remains in history."}>
        <ul className="intake-checklist">{active.checklist.filter(i => i.required || (isOpenIntake(active.status) && i.code.startsWith("admission_"))).map(i => <li key={i.code}><span className={`check-circle ${i.passed ? "complete" : ""}`}><Icon name={i.passed ? "check" : "clock"} size={15} /></span><div><strong>{i.title}</strong><p>{i.detail}</p>{!i.required && <small>Checked at admission</small>}{!i.passed && !dirty && (i.code.startsWith("intake_form:") ? <Button variant="text small" onClick={() => onOpenForms(active.episode_id)}>Open intake forms</Button> : i.code === "identity_confirmation" ? <Button variant="text small" onClick={() => open({ kind: "identity" })}>Review identity</Button> : i.code === "treatment_consent" ? <Button variant="text small" onClick={onOpenConsent}>Open consents</Button> : i.code === "program_assignment" && !active.program_id ? <Button variant="text small" onClick={() => {const row=data.episodes.find(e=>e.id===active.episode_id);if(row)open({kind:"assign",row:{...row,updated_at:active.episode_updated_at || row.updated_at}});}}>Assign program</Button> : i.code === "primary_clinician" || i.code === "adult_eligibility" ? <Button variant="text small" onClick={onEditPatient}>Review patient details</Button> : null)}</div></li>)}</ul>
        <div className="intake-info">{active.checklist.filter(i => !i.required && !i.code.startsWith("admission_")).map(i => <p key={i.code}><strong>{i.title}:</strong> {i.detail}</p>)}</div>
      </Panel></div>
      {isOpenIntake(active.status) && <Panel title="Next step"><div className="intake-next"><p>{active.status === "open" ? "Start screening to review identity, consent, and the intake packet." : active.status === "screening" ? "Resolve the required checks before marking the case ready." : role === "support" ? "A clinical team member can review this case and record admission." : "Review the current checks and record admission when the patient is ready."}</p><div className="intake-actions">{allowedIntakeTransitions(active.status, role).map(to => { const blocked = transitionBlockers(active, to); return <div key={to}><Button variant={to === "declined" || to === "withdrawn" ? "text small" : "secondary"} disabled={dirty || loading || !!blocked.length} onClick={() => open({ kind: "transition", row: active, to })}>{active.status === "ready" && to === "screening" ? "Return to screening" : actions[to]}</Button>{blocked.length > 0 && <small className="intake-blocked">{blocked.length} {blocked.length === 1 ? "check needs" : "checks need"} attention</small>}</div>; })}</div></div></Panel>}
      {!!active.decision_evidence?.length && <Panel title="Saved admission decisions" subtitle="These configuration and checklist snapshots preserve the rules used for each decision.">{active.decision_evidence.map(d=><details className="intake-decision" key={d.id}><summary>{label(active.events.find(e=>e.id===d.event_id)?.to_status || "Decision")} · {String(d.program_snapshot.name || "Program")} v{d.program_version} · {date(d.created_at,true)}</summary><p>Capacity: {String(d.program_snapshot.capacity)} active places · ages {String(d.program_snapshot.min_age)}–{d.program_snapshot.max_age==null?"no upper limit":String(d.program_snapshot.max_age)}</p><ul className="intake-checklist">{d.checklist.filter(i=>i.required || i.code.startsWith("admission_")).map(i=><li key={i.code}><div><strong>{i.passed?"Passed":"Not passed"} · {i.title}</strong><p>{i.detail}</p></div></li>)}</ul></details>)}</Panel>}
      <Panel title="Disposition history"><ol className="intake-history"><li><strong>Intake opened</strong><span>{date(active.created_at, true)}</span></li>{active.events.map(e => <li key={e.id}><strong>{e.from_status === e.to_status ? "Owner reassigned" : `${label(e.from_status)} → ${label(e.to_status)}`}</strong><span>{date(e.created_at, true)} · {staffName(e.actor_id)}</span><p>{e.reason}</p></li>)}</ol></Panel>
    </>}
    {!!state?.identity_checks.length && <Panel title="Identity review history"><ol className="intake-history">{state.identity_checks.map(c => <li key={c.id}><strong>{label(c.outcome)} · {label(c.method)} {c.current ? "· Current identity" : "· Historical review"}</strong><span>{date(c.created_at, true)} · {staffName(c.actor_id)}</span><p>{c.reason}</p></li>)}</ol></Panel>}
  </div>;
}
