export type TranscriptSegment={id:string;start_ms:number;end_ms:number;text:string;speaker_participant_id:string|null;speaker_verified:boolean;patient_id:string|null;context_verified:boolean;uncertain:boolean;excluded:boolean;review_note:string};
export type TranscriptVersion={id:string;transcript_id:string;number:number;state:"draft"|"reviewed";segments:TranscriptSegment[];reason:string;created_at:string;reviewed_at:string|null};
export type TranscriptSummary={id:string;track_id:string;duration_ms:number;engine:string;model_label:string;model_sha256:string;created_at:string;current_version_id:string};
export type TranscriptionJob={id:string;track_id:string;state:"queued"|"running"|"ready"|"failed"|"cancelled";purpose:string;error_code:string;created_at:string;started_at:string|null;completed_at:string|null};
export type TranscriptParticipant={id:string;label:string;role:"host"|"client";client_id:string|null};
export type TranscriptDetail={transcript:TranscriptSummary;current_version:TranscriptVersion;versions:Omit<TranscriptVersion,"segments">[];participants:TranscriptParticipant[];clients:{id:string;label:string}[];excerpts:{id:string;client_id:string;version_id:string;text:string;created_at:string}[]};
export type TranscriptIndex={configured:boolean;jobs:TranscriptionJob[];transcripts:TranscriptSummary[]};
