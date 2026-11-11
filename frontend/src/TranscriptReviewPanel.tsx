import { useEffect, useRef, useState } from "react";
import { request } from "./api";
import { Button } from "./ui";
import type { TranscriptDetail, TranscriptIndex, TranscriptSegment, TranscriptionJob } from "./transcriptTypes";
import "./transcript.css";

const msg = (e: unknown) => e instanceof Error ? e.message : "The request could not be completed.";
const isStale = (e: unknown) => /409|stale|conflict|expected version|version mismatch|newer version|another transcript version/i.test(msg(e));
const stamp = (ms: number) => { const s = Math.max(0, ms) / 1000; const m = Math.floor(s / 60); return `${m}:${(s - m * 60).toFixed(1).padStart(4, "0")}`; };
const eligible = (s: TranscriptSegment, client: string) => Boolean(client && s.text.trim() && !s.excluded && !s.uncertain && s.speaker_verified && s.context_verified && s.patient_id === client);
const payload = (source: TranscriptSegment[]): TranscriptSegment[] => source.map(s => ({ ...s, text: s.text, review_note: s.review_note ?? "", speaker_participant_id: s.speaker_participant_id || null, patient_id: s.patient_id || null, speaker_verified: Boolean(s.speaker_participant_id && s.speaker_verified), context_verified: Boolean(s.patient_id && s.context_verified), uncertain: Boolean(s.uncertain), excluded: Boolean(s.excluded), start_ms: Math.round(Number(s.start_ms)), end_ms: Math.round(Number(s.end_ms)) }));

export default function TranscriptReviewPanel({ roomId, trackId, zone, onClose, onDirtyChange }: { roomId: string; trackId: string; zone: string; onClose: () => void; onDirtyChange:(dirty:boolean)=>void }) {
  const date=(value:string)=>new Intl.DateTimeFormat("en-US",{timeZone:zone,dateStyle:"medium",timeStyle:"short"}).format(new Date(value));
  const [page,setPage]=useState(0);
  const [index, setIndex] = useState<TranscriptIndex | null>(null);
  const [detail, setDetail] = useState<TranscriptDetail | null>(null);
  const [timeInputs,setTimeInputs] = useState<Record<string,{start_ms:string;end_ms:string}>>({});
  const [rows, setRows] = useState<TranscriptSegment[]>([]);
  const [docId, setDocId] = useState("");
  const [pinned, setPinned] = useState("");
  const [purpose, setPurpose] = useState("");
  const [reason, setReason] = useState("");
  const [error, setError] = useState("");
  const [conflict, setConflict] = useState("");
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [attested, setAttested] = useState(false);
  const [patientId, setPatientId] = useState("");
  const [picked, setPicked] = useState<string[]>([]);
  const [jobEpoch, setJobEpoch] = useState(0);
  const [detailEpoch, setDetailEpoch] = useState(0);
  const live = useRef(true), dirtyRef = useRef(false), hotRef = useRef(false), docRef = useRef(""), detailRef = useRef<TranscriptDetail | null>(null), baseline = useRef("");
  docRef.current = docId;
  useEffect(()=>{onDirtyChange(dirty);},[dirty,onDirtyChange]);
  useEffect(()=>()=>onDirtyChange(false),[onDirtyChange]);
  useEffect(()=>{const guard=(event:BeforeUnloadEvent)=>{if(dirtyRef.current){event.preventDefault();event.returnValue="";}};window.addEventListener("beforeunload",guard);return()=>window.removeEventListener("beforeunload",guard);},[]);
  useEffect(() => { live.current = true; return () => { live.current = false; }; }, []);
  useEffect(() => {
    setDocId(""); setPinned(""); setDetail(null); setRows([]); setPatientId(""); setPicked([]); setConflict(""); setError(""); setNote("");
    detailRef.current = null; dirtyRef.current = false; setDirty(false);
  }, [roomId, trackId]);

  function adopt(d: TranscriptDetail) {
    setPage(0);
    setTimeInputs(Object.fromEntries(d.current_version.segments.map(s=>[s.id,{start_ms:String(s.start_ms/1000),end_ms:String(s.end_ms/1000)}])));
    detailRef.current = d; baseline.current = JSON.stringify(d.current_version.segments); dirtyRef.current = false;
    setDetail(d); setRows(d.current_version.segments.map(s => ({ ...s }))); setDirty(false); setAttested(false); setPicked([]); setReason(""); setConflict("");
    setIndex(prev => {
      if (!prev) return prev;
      const has = prev.transcripts.some(t => t.id === d.transcript.id);
      return { ...prev, transcripts: has ? prev.transcripts.map(t => t.id === d.transcript.id ? d.transcript : t) : [...prev.transcripts, d.transcript] };
    });
  }
  function fail(e: unknown) {
    if (!live.current) return;
    setError(isStale(e) ? `${msg(e)} Local edits were not overwritten.` : msg(e));
    if (isStale(e)) setConflict("Stale version conflict. Open edits were kept.");
  }
  async function run(work: () => Promise<void>) {
    setBusy(true); setError(""); setNote("");
    try { await work(); } catch (e) { fail(e); } finally { if (live.current) setBusy(false); }
  }
  function patch(id: string, part: Partial<TranscriptSegment>) {
    dirtyRef.current = true; setDirty(true); setAttested(false);
    setRows(prev => prev.map(s => s.id === id ? { ...s, ...part } : s));
  }
  function commitTime(id:string,key:"start_ms"|"end_ms",raw:string){
    setTimeInputs(prev=>({...prev,[id]:{...prev[id],[key]:raw}}));
    dirtyRef.current=true;setDirty(true);setAttested(false);
    const n=Number(raw);if(raw.trim()&&Number.isFinite(n)&&n>=0)patch(id,{[key]:Math.round(n*1000)});
  }

  useEffect(() => {
    let stop = false, timer = 0;
    const tick = async () => {
      try {
        const next = await request<TranscriptIndex>(`/api/telehealth/rooms/${roomId}/transcripts?track_id=${encodeURIComponent(trackId)}`);
        if (stop) return;
        setIndex(next);
        const mine = next.transcripts.find(t => docRef.current ? t.id === docRef.current : t.track_id === trackId);
        const open = detailRef.current;
        if (mine && !docRef.current) setDocId(mine.id);
        if (mine && open?.transcript.id === mine.id && mine.current_version_id !== open.transcript.current_version_id) {
          if (dirtyRef.current || open.current_version.id !== open.transcript.current_version_id) setConflict("The latest saved version changed on the server. Open text was not overwritten.");
          else setDetailEpoch(n => n + 1);
        }
        hotRef.current = next.jobs.some(j => j.track_id === trackId && (j.state === "queued" || j.state === "running"));
        if (hotRef.current) timer = window.setTimeout(() => void tick(), 3000);
      } catch (e) {
        if (stop) return;
        setError(msg(e));
        if (hotRef.current) timer = window.setTimeout(() => void tick(), 3000);
      }
    };
    void tick();
    return () => { stop = true; window.clearTimeout(timer); };
  }, [roomId, trackId, jobEpoch]);

  useEffect(() => {
    if (!docId) return;
    let stop = false;
    const q = pinned ? `?version_id=${encodeURIComponent(pinned)}` : "";
    void request<TranscriptDetail>(`/api/transcripts/${docId}${q}`).then(d => {
      if (stop || !live.current) return;
      if (dirtyRef.current) { setConflict("Reload skipped. Unsaved edits were not overwritten."); return; }
      adopt(d);
    }).catch(e => { if (!stop && live.current) setError(msg(e)); });
    return () => { stop = true; };
  }, [docId, pinned, detailEpoch]);

  const jobs = (index?.jobs ?? []).filter(j => j.track_id === trackId);
  const docs = (index?.transcripts ?? []).filter(t => t.track_id === trackId);
  const active = jobs.some(j => j.state === "queued" || j.state === "running");
  const retryable = jobs.some(j => j.state === "failed" || j.state === "cancelled");
  const latest = Boolean(detail && detail.current_version.id === detail.transcript.current_version_id);
  const locked = !latest;
  const canStart = Boolean(index?.configured) && purpose.trim().length >= 10 && !active && !busy;
  const versions = detail ? [...(detail.versions.length ? detail.versions : [detail.current_version])].sort((a, b) => b.number - a.number) : [];
  const savedChoices = detail && latest && detail.current_version.state === "reviewed" ? detail.current_version.segments.filter(s => eligible(s, patientId)) : [];

  async function start(manual: boolean) {
    if (!index?.configured) throw new Error("Recognition is not configured on this machine.");
    if (purpose.trim().length < 10) throw new Error("Purpose must be at least 10 characters.");
    if (active) throw new Error("A job is already queued or running. Wait for the server state or cancel it.");
    hotRef.current = true;
    const job = await request<TranscriptionJob>(`/api/telehealth/rooms/${roomId}/transcription-jobs`, "POST", { track_id: trackId, purpose: purpose.trim() });
    if (!live.current) return;
    setIndex(prev => prev ? { ...prev, jobs: [job, ...prev.jobs.filter(j => j.id !== job.id)] } : prev);
    setJobEpoch(n => n + 1);
    setNote(manual ? "Manual retry requested a new job. The previous job was not restarted." : "A new recognition job was requested. Status comes from the server.");
  }
  async function cancel(id: string) {
    const job = await request<TranscriptionJob>(`/api/transcription-jobs/${id}/cancel`, "POST", {});
    if (!live.current) return;
    setIndex(prev => prev ? { ...prev, jobs: prev.jobs.map(j => j.id === job.id ? job : j) } : prev);
    setJobEpoch(n => n + 1);
  }
  async function save() {
    if (!detail) return;
    if (!latest) throw new Error("Open the latest version before saving a draft or correction.");
    const why = reason.trim();
    if (why.length < 10) throw new Error("A reason is required to save a draft or corrected version.");
    if(Object.values(timeInputs).some(t=>[t.start_ms,t.end_ms].some(v=>!v.trim()||!Number.isFinite(Number(v))||Number(v)<0)))throw new Error("Enter valid nonnegative seconds for every segment. Unsaved edits were kept.");
    const segments = payload(rows);
    if (segments.some(s => s.start_ms < 0 || s.end_ms < s.start_ms || s.end_ms > detail.transcript.duration_ms)) throw new Error("Segment times must be nonnegative and inside the measured duration. Edits were kept.");
    const d = await request<TranscriptDetail>(`/api/transcripts/${detail.transcript.id}/versions`, "POST", { expected_version_id: detail.transcript.current_version_id, reason: why, segments });
    if (!live.current) return;
    adopt(d); setPinned(""); setNote("Saved a new draft. Previous versions, including original recognition, were kept.");
  }
  async function approve() {
    if (!detail) return;
    if (dirtyRef.current || dirty || JSON.stringify(rows) !== baseline.current) throw new Error("Save a new draft before approval. Unsaved text cannot be approved.");
    if (!latest || detail.current_version.state !== "draft") throw new Error("Only the latest draft can be approved.");
    if (!attested) throw new Error("Approval requires an explicit attestation.");
    const d = await request<TranscriptDetail>(`/api/transcripts/${detail.transcript.id}/approve`, "POST", { expected_version_id: detail.transcript.current_version_id, attested: true });
    if (!live.current) return;
    adopt(d); setNote("Latest draft approved. Reviewed text is immutable; a correction must be a new draft.");
  }
  async function createExcerpt() {
    if (dirtyRef.current) throw new Error("Save or discard corrections before creating an excerpt.");
    if (!detail || !latest || detail.current_version.state !== "reviewed") throw new Error("Excerpts can be created only from the current approved version.");
    if (!patientId || !picked.length || picked.some(id => !savedChoices.some(s => s.id === id))) throw new Error("Select a patient and eligible segments. Nothing is copied as a group.");
    await request(`/api/transcripts/${detail.transcript.id}/excerpts`, "POST", { version_id: detail.transcript.current_version_id, client_id: patientId, segment_ids: picked });
    if (!live.current) return;
    setPicked([]); setNote("Patient documentation excerpt saved from the approved text, not from unsaved edits.");
    if (dirtyRef.current) setConflict("Excerpt used the approved saved text. Unsaved edits were not overwritten.");
    else setDetailEpoch(n => n + 1);
  }
  function openVersion(id: string) {
    if (!detail) return;
    if (dirtyRef.current) { setError("Save or discard unsaved edits before opening another version."); return; }
    setPinned(id === detail.transcript.current_version_id ? "" : id);
  }

  return <section className="tx-panel" aria-label="Transcript review" data-room-id={roomId} data-track-id={trackId} data-configured={index ? String(index.configured) : "unknown"} data-dirty={dirty ? "true" : "false"} data-polling={active ? "true" : "false"} data-latest={latest ? "true" : "false"} data-version-state={detail?.current_version.state ?? ""}>
    <div className="tx-head"><div><h3>Transcript review</h3><p className="tx-note">Listen to the saved recording, correct the text, and verify who spoke before preparing patient documentation.</p></div><Button variant="text small" onClick={()=>{if(dirtyRef.current){setError("Save or discard unsaved edits before closing transcript review.");return;}onClose();}}>Close transcript review</Button></div>
    {error && <p className="error-banner" role="alert">{error}</p>}
    {conflict && <p className="error-banner" role="alert">{conflict}</p>}
    {note && <p className="tx-note" role="status">{note}</p>}
    <details open={!detail}><summary>Processing and job history</summary>
    <p className={index?.configured ? "tx-ok" : "tx-warn"} role="status">{index == null ? "Checking whether local recognition is configured…" : index.configured ? "Local recognition is configured." : "Local recognition is not configured. A job cannot start until this machine is configured."}</p>
    <fieldset className="tx-consent"><legend>Processing consent</legend><p className="tx-note">Every recorded client needs a current transcription consent, separate from permission to record. Add or review those consents in the client chart before processing.</p></fieldset>
    <form className="tx-form" onSubmit={e => { e.preventDefault(); void run(() => start(false)); }}>
      <label>Purpose of recognition<textarea value={purpose} onChange={e => setPurpose(e.target.value)} required minLength={10} maxLength={2000} autoComplete="off" /></label>
      <div className="tx-actions"><Button type="submit" disabled={!canStart}>Start recognition job</Button>{retryable && <Button disabled={!canStart} onClick={() => void run(() => start(true))}>Manual retry as new job</Button>}<Button variant="secondary small" disabled={busy} onClick={() => setJobEpoch(n => n + 1)}>Refresh job status</Button></div>
    </form>
    <div aria-live="polite">{active && <p className="tx-note">Processing locally. You can leave this view and return to review the result.</p>}{!jobs.length && <p className="tx-note">No recognition jobs for this track.</p>}{jobs.map(j => <article key={j.id} className="tx-job" data-job-id={j.id} data-state={j.state}><p><span className="tx-state">{j.state}</span> · {j.purpose}</p><p className="tx-meta">{j.error_code ? `Error code: ${j.error_code}. ` : ""}Created {date(j.created_at)}{j.started_at ? ` · started ${date(j.started_at)}` : ""}{j.completed_at ? ` · completed ${date(j.completed_at)}` : ""}</p>{(j.state === "queued" || j.state === "running") && <Button variant="secondary small" disabled={busy} onClick={() => void run(() => cancel(j.id))}>Cancel job</Button>}</article>)}<p className="tx-note">You can retry a failed or cancelled job after resolving its cause.</p></div>
    </details>
    {!docs.length && <p className="tx-note">Start recognition to prepare a transcript for review.</p>}
    {docs.length > 0 && <label>Transcript document<select value={docId} onChange={e => { if (dirtyRef.current) { setError("Save or discard unsaved edits before opening another document."); return; } setPinned(""); setDocId(e.target.value); }}>{docs.map(t => <option key={t.id} value={t.id}>{date(t.created_at)} · {t.engine}</option>)}</select></label>}
    {detail && <>
      <details><summary>Recognition source and model</summary><p className="tx-meta">Engine {detail.transcript.engine}. Model {detail.transcript.model_label}. SHA-256 <code>{detail.transcript.model_sha256}</code>. Measured duration <span title={`${detail.transcript.duration_ms} ms`}>{stamp(detail.transcript.duration_ms)}</span>. {latest ? "Showing the latest version." : "Historical version is read-only. Original recognized text stays in history."}</p></details>
      <label>Version history<select value={detail.current_version.id} onChange={e => openVersion(e.target.value)}>{versions.map(v => <option key={v.id} value={v.id}>#{v.number} {v.state}{v.reason ? ` — ${v.reason.slice(0, 80)}` : ""}</option>)}</select></label>
      <p className="tx-note">{detail.current_version.state === "reviewed" ? `Approved ${detail.current_version.reviewed_at ? date(detail.current_version.reviewed_at) : ""}. Corrections create a new draft; this text is not edited in place.` : "Draft. Save a new version before approval."} Verification is never automatic. Changing speaker or patient clears that verification.</p>
      {!rows.length && <p className="tx-note">No recognized words. Nothing was added.</p>}
      {rows.length>5&&<nav className="tx-actions" aria-label="Transcript segments"><Button variant="secondary small" disabled={page===0} onClick={()=>setPage(p=>p-1)}>Previous segments</Button><span>Segments {page*5+1}–{Math.min((page+1)*5,rows.length)} of {rows.length}</span><Button variant="secondary small" disabled={(page+1)*5>=rows.length} onClick={()=>setPage(p=>p+1)}>Next segments</Button></nav>}
      {rows.slice(page*5,(page+1)*5).map((s, localIndex) => {const i=page*5+localIndex;return <article key={s.id} className="tx-seg" data-segment-id={s.id} data-uncertain={s.uncertain ? "true" : "false"} data-excluded={s.excluded ? "true" : "false"}>
        <header><strong>Segment {i + 1}</strong><span>{stamp(s.start_ms)}–{stamp(s.end_ms)}</span></header>
        <label>Text<textarea value={s.text} disabled={locked || busy} autoComplete="off" spellCheck={false} onChange={e => patch(s.id, { text: e.target.value, uncertain: true, context_verified: false })} /></label>
        <div className="tx-times">
          <label>Start seconds<input key={`${s.id}:start:${detailEpoch}:${detail.current_version.id}`} value={timeInputs[s.id]?.start_ms ?? ""} inputMode="decimal" aria-label={`Start seconds for segment ${i + 1}`} disabled={locked || busy} onChange={e => commitTime(s.id, "start_ms", e.target.value)} /></label>
          <label>End seconds<input key={`${s.id}:end:${detailEpoch}:${detail.current_version.id}`} value={timeInputs[s.id]?.end_ms ?? ""} inputMode="decimal" aria-label={`End seconds for segment ${i + 1}`} disabled={locked || busy} onChange={e => commitTime(s.id, "end_ms", e.target.value)} /></label>
        </div>
        <div className="tx-grid">
          <label>Speaker<select value={s.speaker_participant_id ?? ""} disabled={locked || busy} aria-label={`Speaker for segment ${i + 1}`} onChange={e => patch(s.id, { speaker_participant_id: e.target.value || null, speaker_verified: false })}><option value="">Unknown speaker</option>{detail.participants.map(p => <option key={p.id} value={p.id}>{p.label} · {p.role}</option>)}</select></label>
          <label>Patient context<select value={s.patient_id ?? ""} disabled={locked || busy} aria-label={`Patient context for segment ${i + 1}`} onChange={e => patch(s.id, { patient_id: e.target.value || null, context_verified: false })}><option value="">No patient context</option>{detail.clients.map(c => <option key={c.id} value={c.id}>{c.label}</option>)}</select></label>
        </div>
        <label className="tx-check"><input type="checkbox" checked={s.speaker_verified} disabled={locked || busy || !s.speaker_participant_id} onChange={e => patch(s.id, { speaker_verified: e.target.checked })} />Speaker identity verified</label>
        <label className="tx-check"><input type="checkbox" checked={s.context_verified} disabled={locked || busy || !s.patient_id} onChange={e => patch(s.id, { context_verified: e.target.checked })} />Patient context verified</label>
        <label className="tx-check"><input type="checkbox" checked={s.uncertain} disabled={locked || busy} onChange={e => patch(s.id, { uncertain: e.target.checked })} />Uncertain recognition</label>
        <label className="tx-check"><input type="checkbox" checked={s.excluded} disabled={locked || busy} onChange={e => patch(s.id, { excluded: e.target.checked })} />Exclude from review</label>
        <label>Correction note<input value={s.review_note} maxLength={2000} disabled={locked || busy} onChange={e => patch(s.id, { review_note: e.target.value })} /></label>
      </article>;})}
      <label>Reason for the new version<textarea value={reason} onChange={e => setReason(e.target.value)} maxLength={2000} disabled={locked || busy} placeholder="Why this draft or correction is being saved" /></label>
      <div className="tx-actions">
        <Button disabled={busy || locked || reason.trim().length < 10} onClick={() => void run(save)}>{detail.current_version.state === "reviewed" ? "Save corrected version" : "Save new draft"}</Button>
        <Button variant="secondary small" disabled={busy} onClick={() => { dirtyRef.current = false; setDirty(false); setError(""); setDetailEpoch(n => n + 1); }}>{dirty ? "Discard unsaved edits and reload" : "Reload saved version"}</Button>
      </div>
      {latest && detail.current_version.state === "draft" && <fieldset><legend>Approval</legend><p className="tx-note">Review each included segment against the recording. Verify the speaker, resolve uncertainty, and confirm any patient context before approval.</p><label className="tx-check"><input type="checkbox" checked={attested} disabled={busy || dirty} onChange={e => setAttested(e.target.checked)} />I attest this latest saved draft is accurate for the included segments. This does not sign a note, diagnosis, medication, or billing item.</label><Button disabled={busy || dirty || !attested} onClick={() => void run(approve)}>Approve latest draft</Button></fieldset>}
      {latest && detail.current_version.state === "reviewed" && <fieldset><legend>Patient documentation excerpt</legend><p className="tx-note">Choose a patient and the reviewed segments relevant to their care. Only text with verified speaker and patient context is available.</p><label>Patient for excerpt<select value={patientId} onChange={e => { setPatientId(e.target.value); setPicked([]); }}><option value="">Select a patient</option>{detail.clients.map(c => <option key={c.id} value={c.id}>{c.label}</option>)}</select></label>{patientId && !savedChoices.length && <p className="tx-note">No eligible segments for this patient.</p>}{savedChoices.map(s => <label className="tx-check" key={s.id}><input type="checkbox" checked={picked.includes(s.id)} onChange={e => setPicked(prev => e.target.checked ? [...prev, s.id] : prev.filter(id => id !== s.id))} />{stamp(s.start_ms)}–{stamp(s.end_ms)} · {s.text}</label>)}<Button disabled={busy || dirty || !patientId || !picked.length} onClick={() => void run(createExcerpt)}>Save patient excerpt</Button>{detail.excerpts.map(x => <article key={x.id} className="tx-excerpt"><p>{date(x.created_at)} · {detail.clients.find(c => c.id === x.client_id)?.label ?? x.client_id}</p><blockquote>{x.text}</blockquote></article>)}</fieldset>}
    </>}
  </section>;
}
