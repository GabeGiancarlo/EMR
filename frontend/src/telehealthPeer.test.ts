import assert from "node:assert/strict";
import { test } from "node:test";
import { VisitPeers } from "./telehealthPeer.ts";

// Protocol/lifecycle regression checks. Actual media transport is exercised
// separately in three browser windows using generated audio/video tracks.
class Stream {
  tracks: any[];
  constructor(tracks: any[] = []) { this.tracks = [...tracks]; }
  getTracks() { return [...this.tracks]; }
  getAudioTracks() { return this.tracks.filter(t => t.kind === "audio"); }
  getVideoTracks() { return this.tracks.filter(t => t.kind === "video"); }
  addTrack(t: any) { this.tracks.push(t); }
  removeTrack(t: any) { this.tracks = this.tracks.filter(x => x !== t); }
}
class Connection {
  static all: Connection[] = [];
  senders: any[] = []; signalingState = "stable"; connectionState = "new";
  localDescription: any = null; remoteDescription: any = null;
  candidates: any[] = []; descriptions: any[] = []; restarts = 0;
  ontrack: any; onicecandidate: any; onnegotiationneeded: any; onconnectionstatechange: any;
  constructor() { Connection.all.push(this); }
  addTransceiver() { const sender = { track: null, fail: false, async replaceTrack(t: any) { if (this.fail) throw Error("replace failed"); this.track = t; } }; this.senders.push(sender); return { sender }; }
  async setLocalDescription() { this.localDescription = { type: this.signalingState === "have-remote-offer" ? "answer" : "offer", sdp: "v=0\r\n" }; this.signalingState = this.localDescription.type === "answer" ? "stable" : "have-local-offer"; }
  async setRemoteDescription(d: any) { this.descriptions.push(d); this.remoteDescription = d; this.signalingState = d.type === "offer" ? "have-remote-offer" : "stable"; }
  async addIceCandidate(c: any) { this.candidates.push(c); }
  restartIce() { this.restarts++; }
  close() { this.connectionState = "closed"; this.signalingState = "closed"; }
}
const tick = () => new Promise(resolve => setImmediate(resolve));
const participant = (generation = "g1", state = "admitted", online = true) => ({ id: "b", label: "Synthetic", role: "client", state, online, connection_id: generation } as any);
function fixture(self = "a") {
  Connection.all = [];
  Object.assign(globalThis, { RTCPeerConnection: Connection, MediaStream: Stream });
  const sent: any[] = [], errors: string[] = [], remote = new Map<string, any>();
  const mesh = new VisitPeers(self, "self-generation", [], { send: async (...v) => { sent.push(v); }, remote: (id,s) => remote.set(id,s), state: () => {}, error: e => errors.push(e) });
  let id = 0;
  const receive = (payload: any, generation = "g1") => mesh.receive({ id: ++id, from_id: "b", from_connection: generation, payload });
  return { mesh, sent, errors, remote, receive };
}
test("admission, generation replacement and closure remove peers without stopping local devices", async () => {
  const {mesh, receive, remote} = fixture(); let stopped = 0;
  const camera = {kind:"video", stop:()=>stopped++}, audio = {kind:"audio", stop:()=>stopped++};
  await mesh.setStream(new Stream([audio,camera]) as any);
  mesh.sync([participant("g1","waiting"), participant("g2","admitted",false)]); assert.equal(Connection.all.length,0);
  mesh.sync([participant()]); await tick(); const first = Connection.all[0];
  assert.equal(first.senders[1].track,camera);
  await receive({description:{type:"offer",sdp:"v=0"}},"old"); assert.equal(first.descriptions.length,0);
  mesh.sync([participant("g2")]); await tick(); assert.equal(first.connectionState,"closed");
  assert.equal(Connection.all.length,2); assert.equal(remote.get("b"),null);
  mesh.close(); mesh.close(); assert.equal(Connection.all[1].connectionState,"closed"); assert.equal(stopped,0);
  mesh.sync([participant()]); assert.equal(Connection.all.length,2);
});
test("ICE waits for SDP, duplicates are ignored, and impolite glare discards abandoned ICE", async () => {
  const {mesh,receive} = fixture(); mesh.sync([participant()]); await tick(); const pc = Connection.all[0];
  await receive({candidate:{candidate:"candidate:queued"}}); assert.equal(pc.candidates.length,0);
  await receive({description:{type:"offer",sdp:"v=0"}}); assert.equal(pc.candidates.length,1); assert.equal(pc.localDescription.type,"answer");
  const duplicate = {id:900,from_id:"b",from_connection:"g1",payload:{candidate:null}};
  await mesh.receive(duplicate); await mesh.receive(duplicate); assert.equal(pc.candidates.length,2);
  pc.remoteDescription=null; pc.signalingState="have-local-offer";
  await receive({candidate:{candidate:"candidate:abandoned"}});
  await receive({description:{type:"offer",sdp:"v=0"}});
  await receive({candidate:{candidate:"candidate:ignored"}});
  await receive({description:{type:"answer",sdp:"v=0"}});
  assert.equal(pc.candidates.length,2); mesh.close();
});
test("polite glare accepts a remote offer and screen replacement reuses the video sender", async () => {
  const {mesh,receive,remote,errors} = fixture("z"); mesh.sync([participant()]); await tick(); const pc = Connection.all[0];
  pc.signalingState="have-local-offer"; await receive({description:{type:"offer",sdp:"v=0"}});
  assert.equal(pc.localDescription.type,"answer");
  const camera={kind:"video"}, screen={kind:"video"}, audio={kind:"audio"};
  await mesh.setStream(new Stream([camera,audio]) as any); await mesh.setStream(new Stream([screen,audio]) as any);
  assert.equal(pc.senders.length,2); assert.equal(pc.senders[1].track,screen);
  const old={kind:"video"}, replacement={kind:"video"}; pc.ontrack({track:old}); pc.ontrack({track:replacement});
  assert.deepEqual(remote.get("b").getVideoTracks(),[replacement]);
  mesh.restart(); assert.equal(pc.restarts,1);
  pc.senders[1].fail=true; await assert.rejects(mesh.setStream(new Stream([camera]) as any));
  assert.equal(errors.length,1); mesh.close();
});
