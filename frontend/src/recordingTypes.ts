export type RecordingPolicy = {id:string;title:string;purpose:string;retention_days:number;allow_video:boolean;active:boolean};
export type RecordingParticipant = {participant_id:string;label:string;role:"host"|"client";decision:"pending"|"accepted"|"declined"|"withdrawn";eligible:boolean};
export type RecordingState = "requested"|"recording"|"paused"|"ended"|"declined"|"interrupted";
export type RecordingSnapshot = {id:string;state:RecordingState;mode:"audio"|"video";purpose:string;policy_title:string;retention_days:number;reason:string;participants:RecordingParticipant[];created_at:string;started_at:string|null;stopped_at:string|null};
export type RecordingContext = {configured:boolean;policies:RecordingPolicy[];current:RecordingSnapshot|null};
export type CaptureCallbacks = {
  open:(mimeType:string)=>Promise<{trackId:string;uploadToken:string}>;
  upload:(trackId:string,uploadToken:string,sequence:number,blob:Blob)=>Promise<void>;
  finish:(trackId:string,uploadToken:string,expectedChunks:number,complete:boolean)=>Promise<void>;
  status:(state:"starting"|"recording"|"saving"|"saved"|"failed",message:string)=>void;
};
// Worker contract: LocalRecordingCapture(stream:MediaStream, mode:"audio"|"video", callbacks:CaptureCallbacks).
// start():Promise<void>; stop():Promise<void> flushes final data and ordered uploads;
// abort():void discards unsent data, never stops caller-owned media tracks.
// No fetch, device capture, secrets, timers pretending to be measured billable time, or UI dependencies in this engine.
