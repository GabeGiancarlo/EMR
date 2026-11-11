import { useCallback, useRef, useState } from "react";
import type { RefObject } from "react";
import { apiUrl } from "./api";
import { visitRequest } from "./telehealthApi";
import { LocalRecordingCapture } from "./recordingCapture";
import type { VisitSnapshot } from "./telehealthTypes";

async function uploadRequest(path:string,token:string,method:string,body:Blob|Record<string,unknown>) {
  const response=await fetch(apiUrl(path),{method,headers:{Authorization:`Bearer ${token}`,"Content-Type":body instanceof Blob?"application/octet-stream":"application/json"},body:body instanceof Blob?body:JSON.stringify(body),signal:AbortSignal.timeout(8000)});
  if(!response.ok){const data=await response.json().catch(()=>null);throw new Error(data?.error?.message||"The recording could not be saved.");}
}
export function useVisitRecording(token:string,connection:RefObject<string>,camera:RefObject<MediaStream|null>) {
  const [status,setStatus]=useState("Not recording");
  const active=useRef<{id:string;capture:LocalRecordingCapture}|null>(null);
  const interrupted=useRef("");
  const suspend=useCallback(async(flush=false)=>{
    const prior=active.current;active.current=null;
    if(!prior)return;
    if(flush)await prior.capture.stop();
    else {interrupted.current=prior.id;prior.capture.abort();setStatus("Capture interrupted. Unsent media was discarded; ask the clinician to request a new recording.");}
  },[]);
  const sync=useCallback((next:VisitSnapshot)=>{
    const r=next.recording?.current;
    if(!r||next.self.state!=="admitted"||!next.host_present||r.state==="interrupted"||r.state==="declined"){void suspend();return;}
    if(r.state!=="recording"){void suspend(r.state==="paused"||r.state==="ended");return;}
    if(active.current?.id===r.id||interrupted.current===r.id)return;
    if(!r.participants.some(p=>p.participant_id===next.self.id&&p.decision==="accepted"&&p.eligible))return;
    const stream=camera.current;
    if(!stream?.getAudioTracks().some(t=>t.readyState==="live")){
      interrupted.current=r.id;setStatus("This device has no active microphone. Pause the recording, enable your devices, and request a new recording.");return;
    }
    const capture=new LocalRecordingCapture(stream,r.mode,{
      open:async mimeType=>{const out=await visitRequest<{track_id:string;upload_token:string}>(token,`recordings/${r.id}/tracks`,{connection_id:connection.current,mime_type:mimeType});return {trackId:out.track_id,uploadToken:out.upload_token};},
      upload:(id,key,sequence,blob)=>uploadRequest(`/api/recording-tracks/${id}/chunks/${sequence}`,key,"PUT",blob),
      finish:(id,key,count,complete)=>uploadRequest(`/api/recording-tracks/${id}/finish`,key,"POST",{expected_chunks:count,complete}),
      status:(_state,message)=>setStatus(message),
    });
    active.current={id:r.id,capture};void capture.start();
  },[camera,connection,suspend,token]);
  return {status,sync,suspend};
}
