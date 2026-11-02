import { apiUrl } from "./api";
export class VisitError extends Error {
  constructor(message:string,public status:number,public code:string){super(message);}
}
export async function visitRequest<T>(token:string,action:string,body?:unknown,signal?:AbortSignal):Promise<T> {
  const controller=new AbortController();
  const abort=()=>controller.abort();
  signal?.addEventListener("abort",abort,{once:true});
  if(signal?.aborted)controller.abort();
  const timer=setTimeout(abort,8000);
  try {
    const response=await fetch(apiUrl(`/api/telehealth/visit${action ? `/${action}` : ""}`),{method:body===undefined?"GET":"POST",headers:{"Content-Type":"application/json","Authorization":`Bearer ${token}`},body:body===undefined?undefined:JSON.stringify(body),cache:"no-store",signal:controller.signal});
    const data=await response.json().catch(()=>null);
    if(!response.ok)throw new VisitError(data?.error?.message||"The visit could not be reached.",response.status,data?.error?.code||"visit_error");
    return data as T;
  }finally{clearTimeout(timer);signal?.removeEventListener("abort",abort);}
}
export const visitTokenKey="nexus.visit-access";
export function readVisitToken():string {
  const params=new URLSearchParams(window.location.hash.slice(1));
  const incoming=params.get("token");
  if(incoming&&/^[a-f0-9]{64}$/.test(incoming)) {
    sessionStorage.setItem(visitTokenKey,incoming);
    history.replaceState(null,"",window.location.pathname);
    return incoming;
  }
  const saved=sessionStorage.getItem(visitTokenKey)||"";
  return /^[a-f0-9]{64}$/.test(saved)?saved:"";
}
export const visitLink=(token:string)=>`${location.origin}/visit#token=${encodeURIComponent(token)}`;
