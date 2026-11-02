import { useCallback, useEffect, useState } from "react";
import type { FormEvent } from "react";
import { request, value, withOriginalUpdatedAt } from "./api";
import type { Bootstrap, Row } from "./api";
import { Badge, Button, Empty, Modal, Panel } from "./ui";
import { facilityWallTime, occurrenceLabel, resolveFacilityTime, wallTimeCandidates } from "./schedulingTime";
import { safeTimeZone } from "./calendar";

const manages = (data: Bootstrap, row?: Row) => ["admin","clinical_lead","support"].includes(value(data.actor,"role")) || (value(data.actor,"role") === "clinician" && (!row || row.staff_id === data.actor?.id));
const zoneFor = (data: Bootstrap, staffId: string) => {
  const staff = data.staff.find(row => row.id === staffId);
  return safeTimeZone(value(data.facilities.find(row => row.id === staff?.facility_id),"timezone"));
};

export default function StaffAvailability({data}: {data: Bootstrap}) {
  const [rows,setRows] = useState<Row[]>([]);
  const [loading,setLoading] = useState(true);
  const [error,setError] = useState("");
  const [filter,setFilter] = useState("");
  const [inactive,setInactive] = useState(false);
  const [editing,setEditing] = useState<Row | null | undefined>(undefined);
  const [busy,setBusy] = useState(false);
  const reload = useCallback(async () => {
    setLoading(true); setError("");
    try { setRows(await request<Row[]>("/api/scheduling/unavailability")); }
    catch (error) { setError(error instanceof Error ? error.message : "Availability could not be loaded."); }
    finally { setLoading(false); }
  },[]);
  useEffect(()=> { void reload(); },[reload]);
  async function changeActive(row:Row) {
    setBusy(true); setError("");
    try { await request(`/api/scheduling/unavailability/${row.id}`,"PATCH",withOriginalUpdatedAt(row,{active:row.active !== true})); await reload(); }
    catch(error) { setError(error instanceof Error ? error.message : "Availability could not be changed."); }
    finally { setBusy(false); }
  }
  const shown = rows.filter(row => (!filter || row.staff_id === filter) && (inactive || row.active === true)).sort((a,b)=>value(a,"starts_at").localeCompare(value(b,"starts_at")));
  return <Panel title="Staff availability" subtitle="Record time away from care. Active blocks prevent conflicting appointments and group assignments." action={manages(data) ? <Button onClick={()=>setEditing(null)}>Add unavailable time</Button> : undefined}>
    <div className="toolbar">
      <label>Team member <select className="compact-select" value={filter} onChange={event=>setFilter(event.target.value)}><option value="">All team members</option>{data.staff.filter(row => ["admin","clinical_lead","clinician"].includes(value(row,"role"))).map(row=><option key={row.id} value={row.id}>{value(row,"name")}</option>)}</select></label>
      <label><input type="checkbox" checked={inactive} onChange={event=>setInactive(event.target.checked)} /> Include cancelled blocks</label>
      <Button variant="secondary small" disabled={loading} onClick={()=>void reload()}>Refresh availability</Button>
    </div>
    {error && <p className="error-banner" role="alert">{error}</p>}
    {loading ? <p className="availability-state" role="status">Loading availability…</p> : !shown.length ? <Empty icon="schedule" title="No unavailable time in this view" description="Add a block when a clinician will be away from scheduled care." /> : <div className="table-wrap"><table><thead><tr><th>Team member</th><th>Unavailable time</th><th>Reason</th><th>Status</th><th>Actions</th></tr></thead><tbody>{shown.map(row=>{
      const zone=zoneFor(data,value(row,"staff_id"));
      return <tr key={row.id}><td>{value(data.staff.find(staff=>staff.id===row.staff_id),"name")}</td><td><strong>{facilityWallTime(value(row,"starts_at"),zone).replace("T"," ")}</strong><small>to {facilityWallTime(value(row,"ends_at"),zone).replace("T"," ")} · {zone}</small></td><td>{value(row,"reason")}</td><td><Badge status={row.active ? "active" : "cancelled"} /></td><td>{manages(data,row) && <div className="row-actions"><Button variant="text small" disabled={busy} onClick={()=>setEditing(row)}>Edit</Button><Button variant="text small" disabled={busy} onClick={()=>void changeActive(row)}>{row.active ? "Cancel block" : "Reactivate"}</Button></div>}</td></tr>;
    })}</tbody></table></div>}
    {editing !== undefined && <Modal title={editing ? "Edit unavailable time" : "Add unavailable time"} description="Use a brief scheduling reason. Keep personal and clinical details out of this field." onClose={()=>setEditing(undefined)}>
      <AvailabilityForm key={editing?.id || "new"} data={data} record={editing} onClose={()=>setEditing(undefined)} onSaved={async()=>{setEditing(undefined); await reload();}} />
    </Modal>}
  </Panel>;
}

function AvailabilityForm({data,record,onClose,onSaved}:{data:Bootstrap;record:Row|null;onClose:()=>void;onSaved:()=>Promise<void>}) {
  const [staffId,setStaffId]=useState(value(record,"staff_id") || (value(data.actor,"role")==="clinician" ? data.actor!.id : ""));
  const zone=zoneFor(data,staffId);
  const [start,setStart]=useState(record ? facilityWallTime(value(record,"starts_at"),zone) : "");
  const [end,setEnd]=useState(record ? facilityWallTime(value(record,"ends_at"),zone) : "");
  const [reason,setReason]=useState(value(record,"reason"));
  const minute=(text:string)=>new Date(Math.floor(Date.parse(text)/60000)*60000).toISOString();
  const [startChoice,setStartChoice]=useState(record ? minute(value(record,"starts_at")) : "");
  const [endChoice,setEndChoice]=useState(record ? minute(value(record,"ends_at")) : "");
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState("");
  const choices=data.staff.filter(row=>row.id === record?.staff_id || (row.active === true && ["admin","clinical_lead","clinician"].includes(value(row,"role")) && manages(data,{id:"candidate",staff_id:row.id})));
  async function submit(event:FormEvent) {
    event.preventDefault(); setBusy(true); setError("");
    try {
      if (!staffId || !reason.trim()) throw new Error("Choose a team member and enter a scheduling reason.");
      const starts_at=resolveFacilityTime(start,zone,startChoice,value(record,"starts_at"));
      const ends_at=resolveFacilityTime(end,zone,endChoice,value(record,"ends_at"));
      if (Date.parse(ends_at)<=Date.parse(starts_at)) throw new Error("End time must be after start time.");
      const body={starts_at,ends_at,reason:reason.trim()};
      if (record) await request(`/api/scheduling/unavailability/${record.id}`,"PATCH",withOriginalUpdatedAt(record,body));
      else await request("/api/scheduling/unavailability","POST",{...body,staff_id:staffId});
      await onSaved();
    } catch(error) {setError(error instanceof Error ? error.message : "Unavailable time could not be saved.");}
    finally {setBusy(false);}
  }
  return <form onSubmit={submit}><div className="form-body"><p className="scheduling-zone-note">Times use <strong>{zone}</strong>.</p><div className="form-grid">
    <label className="form-field full">Team member<select required value={staffId} disabled={Boolean(record)} onChange={event=>{setStaffId(event.target.value);setStartChoice("");setEndChoice("");}}><option value="">Choose…</option>{choices.map(row=><option key={row.id} value={row.id}>{value(row,"name")}</option>)}</select></label>
    {([{label:"Start",text:start,set:setStart,choice:startChoice,setChoice:setStartChoice},{label:"End",text:end,set:setEnd,choice:endChoice,setChoice:setEndChoice}]).map(field=><div className="form-field" key={field.label}><label>{field.label}<input type="datetime-local" required value={field.text} onChange={event=>{field.set(event.target.value);field.setChoice("");}} onInput={event=>{field.set(event.currentTarget.value);field.setChoice("");}} /></label>{wallTimeCandidates(field.text,zone).length>1 && <label>{field.label} occurrence<select required value={field.choice} onChange={event=>field.setChoice(event.target.value)}><option value="">Choose first or second…</option>{wallTimeCandidates(field.text,zone).map((instant,index)=><option key={instant} value={instant}>{occurrenceLabel(instant,zone,index)}</option>)}</select></label>}</div>)}
    <label className="form-field full">Scheduling reason<input required maxLength={200} value={reason} onChange={event=>setReason(event.target.value)} placeholder="For example, training or time away" /></label>
  </div>{error && <p className="error-banner" role="alert">{error}</p>}</div><div className="modal-foot"><Button variant="secondary" disabled={busy} onClick={onClose}>Cancel</Button><Button type="submit" disabled={busy}>{busy ? "Saving…" : "Save unavailable time"}</Button></div></form>;
}
