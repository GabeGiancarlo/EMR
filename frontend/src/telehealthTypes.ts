import type { RecordingContext } from "./recordingTypes";
// Participant APIs never return chart identifiers or clinical narratives.
export type VisitParticipant = { id:string; label:string; role:"host"|"client"; state:"invited"|"waiting"|"admitted"|"left"|"removed"; connection_id:string|null; online:boolean };
export type VisitSignal = { id:number; from_id:string; from_connection:string; payload: { description?:RTCSessionDescriptionInit; candidate?:RTCIceCandidateInit|null } };
export type BoardStroke = { id:string; author_id?:string; color:string; points:[number,number][] };
export type VisitSnapshot = { room:{id:string; state:"open"|"ended"; expires_at:string}; self:VisitParticipant; participants:VisitParticipant[]; signals:VisitSignal[]; cursor:number; host_present:boolean; board:BoardStroke[]; ice_servers:RTCIceServer[]; connectivity:"local"|"configured"; recording:RecordingContext };
export type PeerCallbacks = {
  send:(to:string,toConnection:string,payload:VisitSignal["payload"])=>Promise<void>;
  remote:(id:string,stream:MediaStream|null)=>void;
  state:(id:string,state:RTCPeerConnectionState)=>void;
  error:(message:string)=>void;
};
// Worker implementation contract: export class VisitPeers with constructor(selfId:string, connectionId:string, iceServers:RTCIceServer[], callbacks:PeerCallbacks).
// Methods: setStream(stream:MediaStream):Promise<void>; sync(participants:VisitParticipant[]):void; receive(signal:VisitSignal):Promise<void>; restart():void; close():void.
// sync excludes self and removes disconnected/replaced peers; setStream swaps outgoing audio/video (screen sharing uses replaceTrack).
