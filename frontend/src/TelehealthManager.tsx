import { useCallback,useEffect,useState } from "react";
import type { Bootstrap,Row } from "./api";
import { request,value,clientName } from "./api";
import { schedulingZone } from "./schedulingTime";
import { Button,Modal,Badge } from "./ui";
import { visitLink } from "./telehealthApi";
import type { VisitParticipant } from "./telehealthTypes";
import "./telehealth.css";

type Managed={room:Row;participants:(VisitParticipant&{client_id:string|null;staff_id:string|null;eligible:boolean})[];events:Row[];invitees:Row[]};
type Invitation={token:string;expires_at:string;participant:VisitParticipant};
export default function TelehealthManager({data,encounter,kind,onClose}:{data:Bootstrap;encounter:Row;kind:"sessions"|"groups";onClose:()=>void}) {
  const zone=schedulingZone(data,encounter,kind);
  const date=(input:string,_time=true)=>`${new Intl.DateTimeFormat("en-US",{timeZone:zone,dateStyle:"medium",timeStyle:"short"}).format(new Date(input))} ${zone}`;
  const[managed,setManaged]=useState<Managed|null>(null),[error,setError]=useState(""),[busy,setBusy]=useState(false),[invitation,setInvitation]=useState<Invitation|null>(null),[copied,setCopied]=useState(false);
  const reload=useCallback(async(roomId:string)=>setManaged(await request<Managed>(`/api/telehealth/rooms/${roomId}`)),[]);
  useEffect(()=>{let alive=true; request<Row>("/api/telehealth/rooms","POST",{[kind==="groups"?"group_id":"session_id"]:encounter.id}).then(async room=>{const record=await request<Managed>(`/api/telehealth/rooms/${room.id}`);if(alive)setManaged(record);}).catch(error=>{if(alive)setError(error.message);});return()=>{alive=false;};},[encounter.id,kind]);
  const clients=managed?.invitees||(kind==="sessions"?data.clients.filter(row=>row.id===encounter.client_id):[]);
  async function issue(client?:Row){if(!managed)return;setBusy(true);setError("");setCopied(false);setInvitation(null);try{setInvitation(await request<Invitation>(`/api/telehealth/rooms/${managed.room.id}/${client?"invite":"host"}`,"POST",client?{client_id:client.id}:{}));await reload(managed.room.id);}catch(error){setError(error instanceof Error?error.message:"The invitation could not be created.");}finally{setBusy(false);}}
  async function command(action:string){if(!managed)return;setBusy(true);setError("");try{await request(`/api/telehealth/rooms/${managed.room.id}/${action}`,"POST",{});setInvitation(null);await reload(managed.room.id);}catch(error){setError(error instanceof Error?error.message:"The visit could not be updated.");}finally{setBusy(false);}}
  const ended=managed?.room.ended_at|| (managed&&Date.parse(value(managed.room,"expires_at"))<=Date.now());
  return <Modal title="Telehealth visit" description="Prepare participant access for this encounter. Admission happens inside the clinician's visit." onClose={onClose} wide>
    <div className="form-body visit-management">
      <div className="visit-context"><strong>{kind==="groups"?value(encounter,"title"):clientName(clients[0])}</strong><span>{date(value(encounter,"starts_at"),true)}</span><Badge status={value(encounter,"status")} /></div>
      {error&&<p className="error-banner" role="alert">{error}</p>}
      {!managed&&!error&&<p role="status">Preparing the visit…</p>}
      {managed&&<>
        <p>Room access expires {date(value(managed.room,"expires_at"),true)}. Each invitation is for one participant. It does not verify their identity.</p>
        <p className="visit-local-note">Local review · Use separate browser windows on this computer. Remote connectivity and identity are not configured.</p>
        {ended?<p role="status">This room has ended. Closing and reopening telehealth prepares a new room if the encounter is still scheduled.</p>:<>
          <Button disabled={busy} onClick={()=>void issue()}>Prepare my clinician access</Button>
          <h3>Client invitations</h3>
          {!clients.length?<p>No eligible client is enrolled in this encounter. Check the episode and group enrollment before inviting a participant.</p>:clients.map(client=><div className="visit-invite-row" key={client.id}><span>{clientName(client)}</span><Button variant="secondary small" disabled={busy} onClick={()=>void issue(client)}>Create invitation</Button></div>)}
        </>}
        {invitation&&<section className="visit-invitation" aria-label="New visit invitation"><strong>{invitation.participant.role==="host"?"Your clinician visit":"Participant invitation"} · {invitation.participant.label}</strong><p>This link replaces any earlier invitation for this participant. Share it only with the intended person.</p><div className="visit-actions"><a className="button" href={visitLink(invitation.token)} target="_blank" rel="noopener noreferrer">Open visit in new window</a><Button variant="secondary" onClick={async()=>{try{await navigator.clipboard.writeText(visitLink(invitation.token));setCopied(true);}catch{setError("Copy is unavailable. Open the visit link in a new window.");}}}>{copied?"Invitation copied":"Copy invitation"}</Button></div><small>Expires {date(invitation.expires_at,true)}. Copy it now; Nexus does not retain a retrievable copy of this link.</small></section>}
        <div className="visit-section-heading"><h3>Participation</h3><Button variant="text small" disabled={busy} onClick={()=>void reload(managed.room.id).catch(e=>setError(e.message))}>Refresh participation</Button></div>
        {managed.participants.map(p=><div className="visit-invite-row" key={p.id}><span>{p.label} · {p.role==="host"?"Clinician":"Client"}</span><span>{p.state}{p.online?" · connected":""}{!p.eligible?" · no longer eligible":""}</span>{!ended&&p.state==="waiting"&&p.eligible&&<Button variant="secondary small" disabled={busy} onClick={()=>void command(`participants/${p.id}/admit`)}>Admit</Button>}{!ended&&!["removed","left"].includes(p.state)&&<Button variant="text small" disabled={busy} onClick={()=>void command(`participants/${p.id}/remove`)}>Remove</Button>}</div>)}
        <details><summary>Participation history ({managed.events.length})</summary><ol className="visit-history">{managed.events.map(e=><li key={e.id}><strong>{value(e,"action").replaceAll("_"," ")}</strong> · {date(value(e,"occurred_at"),true)}</li>)}</ol></details>
        <p>Visit connection history does not establish attendance, delivered care, or billable duration. Recording is not available in this build.</p>
      </>}
    </div><div className="modal-foot"><Button variant="secondary" onClick={onClose}>Close</Button>{managed&&!ended&&<Button variant="danger" disabled={busy} onClick={()=>void command("end")}>End room for everyone</Button>}</div>
  </Modal>;
}
