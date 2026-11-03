import { useEffect, useState } from "react";
import { date, label, request, value } from "./api";
import type { Bootstrap, Row } from "./api";
import { Button, Panel } from "./ui";

export default function EpisodeProgramAssignment({data,clientId,onChanged,onDirtyChange}:{data:Bootstrap;clientId:string;onChanged:()=>Promise<void>;onDirtyChange:(dirty:boolean)=>void}) {
  const [episode,setEpisode]=useState<Row|null>(null);
  const [program,setProgram]=useState("");
  const [reason,setReason]=useState("");
  const [attested,setAttested]=useState(false);
  const [busy,setBusy]=useState(false);
  const [discard,setDiscard]=useState(false);
  const [error,setError]=useState("");
  const [notice,setNotice]=useState("");
  const role=value(data.actor,"role");
  const eligible=data.episodes.filter(e=>e.client_id===clientId && !e.program_id && (e.status==="planned" && ["admin","clinical_lead","clinician","support"].includes(role) || e.status==="active" && ["admin","clinical_lead"].includes(role)));
  const programs=(data.care_programs||[]).filter(p=>p.active===true && p.facility_id===episode?.facility_id && p.level_of_care===episode?.level_of_care && data.facilities.some(f=>f.id===p.facility_id && f.active===true));
  const dirty=!!episode || busy;
  useEffect(()=>{onDirtyChange(dirty);return()=>onDirtyChange(false);},[dirty,onDirtyChange]);
  useEffect(()=>{if(!dirty)return;const stop=(event:BeforeUnloadEvent)=>{event.preventDefault();event.returnValue="";};window.addEventListener("beforeunload",stop);return()=>window.removeEventListener("beforeunload",stop);},[dirty]);
  if(!data.program_configuration || (!eligible.length && !notice))return null;
  const close=()=>{setEpisode(null);setReason("");setProgram("");setAttested(false);setDiscard(false);setError("");};
  return <Panel title="Connect existing care to a program" subtitle="Older episodes keep their original history. Assign a matching program before booking new services.">
    {error && <div className="error-banner" role="alert">{error}</div>}{notice && <p role="status">{notice}</p>}
    {!episode ? <div className="intake-actions">{eligible.map(row=><Button key={row.id} variant="secondary small" onClick={()=>{setEpisode(row);setNotice("");}}>{label(value(row,"level_of_care"))} · {date(row.start_date)} · {label(value(row,"status"))}</Button>)}</div> : <form className="intake-editor" onSubmit={async event=>{
      event.preventDefault();if(busy)return;setBusy(true);setError("");
      try{await request(`/api/episodes/${episode.id}/assign-program`,"POST",{program_id:program,reason:reason.trim(),attested,_expected_updated_at:episode.updated_at});close();setNotice("Program assigned. The episode's original status and dates are preserved.");try{await onChanged();}catch{setError("The assignment was saved. Refresh the workspace before continuing.");}}
      catch(error){setError(error instanceof Error?error.message:"The program assignment could not be saved.");}finally{setBusy(false);}
    }}><fieldset disabled={busy}><p>{label(value(episode,"level_of_care"))} · {date(episode.start_date)} · {value(data.facilities.find(f=>f.id===episode.facility_id),"name")}</p>
      <div className="form-field"><label htmlFor="episode-program">Care program *</label><select id="episode-program" required value={program} onChange={event=>setProgram(event.target.value)}><option value="">Select a matching program</option>{programs.map(p=><option key={p.id} value={p.id}>{value(p,"name")} · {value(p,"available_places")} places available</option>)}</select>{!programs.length && <small>An administrator must configure an active program for this facility and care level.</small>}</div>
      <div className="form-field"><label htmlFor="episode-program-reason">Assignment reason *</label><textarea id="episode-program-reason" required minLength={10} maxLength={2000} value={reason} onChange={event=>setReason(event.target.value)} /></div>
      <label className="intake-attestation"><input type="checkbox" required checked={attested} onChange={event=>setAttested(event.target.checked)} />I reviewed this care episode and confirm the program assignment. It cannot be remapped.</label>
      <div className="intake-actions"><Button type="submit" disabled={busy || !programs.length}>{busy?"Assigning…":"Assign program"}</Button><Button variant="text" disabled={busy} onClick={()=>setDiscard(true)}>Discard changes</Button></div>
      {discard && <div className="intake-discard" role="alert"><p>Discard this assignment form?</p><Button variant="secondary small" onClick={close}>Discard form</Button><Button variant="text small" onClick={()=>setDiscard(false)}>Keep editing</Button></div>}
    </fieldset></form>}
  </Panel>;
}
