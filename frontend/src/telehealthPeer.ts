import type { PeerCallbacks,VisitParticipant,VisitSignal } from "./telehealthTypes";
type Peer={id:string;generation:string;pc:RTCPeerConnection;audio:RTCRtpSender;video:RTCRtpSender;stream:MediaStream;polite:boolean;makingOffer:boolean;ignoreOffer:boolean;answerPending:boolean;pendingIce:(RTCIceCandidateInit|null)[];chain:Promise<void>;outbound:Promise<void>;closed:boolean};
/** Browser-native media mesh; generation-scoped signaling and W3C perfect negotiation. */
export class VisitPeers {
  private peers=new Map<string,Peer>();private stream:MediaStream|null=null;private closed=false;private seen=new Set<number>();
  private selfId:string;private iceServers:RTCIceServer[];private callbacks:PeerCallbacks;
  constructor(selfId:string,connectionId:string,iceServers:RTCIceServer[],callbacks:PeerCallbacks){if(!selfId||!connectionId)throw new Error("A current visit connection is required.");this.selfId=selfId;this.iceServers=iceServers;this.callbacks=callbacks;}
  private active(peer:Peer){return !this.closed&&!peer.closed&&this.peers.get(peer.id)===peer;}
  private report(peer:Peer,message:string){if(this.active(peer))this.callbacks.error(message);}
  private queue(peer:Peer,operation:()=>Promise<void>,propagate=false):Promise<void>{const run=peer.chain.then(async()=>{if(this.active(peer))await operation();});peer.chain=run.catch(()=>this.report(peer,"A media connection needs attention. Use Reconnect media to try again."));return propagate?run:peer.chain;}
  private send(peer:Peer,payload:VisitSignal["payload"]){peer.outbound=peer.outbound.then(async()=>{if(this.active(peer))await this.callbacks.send(peer.id,peer.generation,payload);}).catch(()=>this.report(peer,"The media connection could not be negotiated. Check the visit connection and retry."));return peer.outbound;}
  private add(participant:VisitParticipant){
    if(!participant.connection_id||this.closed)return;
    const pc=new RTCPeerConnection({iceServers:this.iceServers,bundlePolicy:"max-bundle"});
    const peer:Peer={id:participant.id,generation:participant.connection_id,pc,audio:pc.addTransceiver("audio",{direction:"sendrecv"}).sender,video:pc.addTransceiver("video",{direction:"sendrecv"}).sender,stream:new MediaStream(),polite:this.selfId>participant.id,makingOffer:false,ignoreOffer:false,answerPending:false,pendingIce:[],chain:Promise.resolve(),outbound:Promise.resolve(),closed:false};
    this.peers.set(peer.id,peer);
    pc.onicecandidate=event=>{if(this.active(peer))void this.send(peer,{candidate:event.candidate?.toJSON()||null});};
    pc.ontrack=event=>{if(!this.active(peer))return;for(const old of peer.stream.getTracks())if(old!==event.track&&old.kind===event.track.kind)peer.stream.removeTrack(old);if(!peer.stream.getTracks().includes(event.track))peer.stream.addTrack(event.track);this.callbacks.remote(peer.id,peer.stream);event.track.onended=()=>{if(!this.active(peer))return;peer.stream.removeTrack(event.track);this.callbacks.remote(peer.id,peer.stream.getTracks().length?peer.stream:null);};};
    pc.onconnectionstatechange=()=>{if(this.active(peer))this.callbacks.state(peer.id,pc.connectionState);};
    pc.onnegotiationneeded=()=>{void this.queue(peer,async()=>{if(pc.signalingState!=="stable")return;try{peer.makingOffer=true;await pc.setLocalDescription();if(this.active(peer)&&pc.localDescription)await this.send(peer,{description:{type:pc.localDescription.type,sdp:pc.localDescription.sdp}});}finally{peer.makingOffer=false;}});};
    void this.queue(peer,()=>this.replace(peer));
  }
  private async replace(peer:Peer){await peer.audio.replaceTrack(this.stream?.getAudioTracks()[0]||null);if(this.active(peer))await peer.video.replaceTrack(this.stream?.getVideoTracks()[0]||null);}
  async setStream(stream:MediaStream){if(this.closed)return;this.stream=stream;await Promise.all([...this.peers.values()].map(peer=>this.queue(peer,()=>this.replace(peer),true)));}
  sync(participants:VisitParticipant[]){if(this.closed)return;const desired=new Map(participants.filter(p=>p.id!==this.selfId&&p.connection_id&&p.state==="admitted"&&p.online).map(p=>[p.id,p]));for(const peer of this.peers.values()){const next=desired.get(peer.id);if(!next||next.connection_id!==peer.generation)this.remove(peer);}for(const p of desired.values())if(!this.peers.has(p.id))this.add(p);}
  async receive(signal:VisitSignal){const peer=this.peers.get(signal.from_id);if(!peer||peer.generation!==signal.from_connection||!this.active(peer)||this.seen.has(signal.id))return;this.seen.add(signal.id);if(this.seen.size>2048)this.seen.delete(this.seen.values().next().value!);await this.queue(peer,async()=>{
    const {description,candidate}=signal.payload;const pc=peer.pc;
    if(description){
      const ready=!peer.makingOffer&&(pc.signalingState==="stable"||peer.answerPending);const collision=description.type==="offer"&&!ready;peer.ignoreOffer=!peer.polite&&collision;if(peer.ignoreOffer){peer.pendingIce=[];return;}
      peer.answerPending=description.type==="answer";
      try{await pc.setRemoteDescription(description);}finally{peer.answerPending=false;}
      if(!this.active(peer))return;
      for(const ice of peer.pendingIce.splice(0)){if(!this.active(peer))return;await pc.addIceCandidate(ice||undefined);}
      if(description.type==="offer"&&this.active(peer)){await pc.setLocalDescription();if(this.active(peer)&&pc.localDescription)await this.send(peer,{description:{type:pc.localDescription.type,sdp:pc.localDescription.sdp}});}
    }else if("candidate" in signal.payload){
      if(peer.ignoreOffer)return;
      if(!pc.remoteDescription){if(peer.pendingIce.length>=256)throw new Error("Candidate queue exceeded");peer.pendingIce.push(candidate||null);}else await pc.addIceCandidate(candidate||undefined);
    }
  });}
  restart(){for(const peer of this.peers.values())if(this.active(peer))peer.pc.restartIce();}
  private remove(peer:Peer){peer.closed=true;peer.pc.ontrack=null;peer.pc.onicecandidate=null;peer.pc.onnegotiationneeded=null;peer.pc.onconnectionstatechange=null;peer.pc.close();peer.pendingIce=[];this.peers.delete(peer.id);this.callbacks.remote(peer.id,null);this.callbacks.state(peer.id,"closed");}
  close(){if(this.closed)return;this.closed=true;for(const peer of [...this.peers.values()])this.remove(peer);this.stream=null;this.seen.clear();}
}
