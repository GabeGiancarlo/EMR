import test from "node:test";
import assert from "node:assert/strict";
import { setImmediate as flushEvents } from "node:timers/promises";
import {LocalRecordingCapture} from "./recordingCapture.ts";

type Finish = {chunks: number; complete: boolean};
type Track = {
  kind: string;
  label: string;
  getSettings: () => {displaySurface?: string};
  stop: () => void;
};

class FakeStream {
  tracks: Track[];
  constructor(tracks: Track[]) { this.tracks = tracks; }
  getTracks(): Track[] { return this.tracks; }
}

class FakeRecorder {
  static instances: FakeRecorder[] = [];
  static isTypeSupported(type: string): boolean {
    return type === "audio/webm;codecs=opus" || type === "video/webm;codecs=vp9,opus" || type === "video/webm" || type === "audio/webm";
  }
  state = "inactive";
  mimeType: string;
  stream: FakeStream;
  timeslice?: number;
  holdEvents = false;
  ondataavailable: ((ev: {data: Blob}) => void) | null = null;
  onstop: (() => void) | null = null;
  onerror: ((ev: {error?: unknown}) => void) | null = null;
  constructor(stream: FakeStream, options?: {mimeType?: string}) {
    this.stream = stream;
    this.mimeType = options?.mimeType ?? "";
    FakeRecorder.instances.push(this);
  }
  start(timeslice?: number): void {
    this.state = "recording";
    this.timeslice = timeslice;
  }
  stop(): void {
    if (this.state === "inactive") return;
    this.state = "inactive";
    if (this.holdEvents) return;
    this.ondataavailable?.({data: new Blob(["final"])});
    this.onstop?.();
  }
  emit(data: Blob): void { this.ondataavailable?.({data}); }
}

function track(kind: string, label: string, displaySurface?: string): Track {
  return {
    kind,
    label,
    getSettings: () => ({displaySurface}),
    stop: () => { throw new Error("caller-owned track stopped"); },
  };
}

function install(): void {
  FakeRecorder.instances = [];
  (globalThis as unknown as {MediaRecorder: typeof FakeRecorder}).MediaRecorder = FakeRecorder;
  (globalThis as unknown as {MediaStream: typeof FakeStream}).MediaStream = FakeStream;
}

function harness(upload?: (seq: number, blob: Blob) => Promise<void>) {
  const statuses: string[] = [];
  const finishes: Finish[] = [];
  let openedMime = "";
  const callbacks = {
    open: async (mime: string) => {
      openedMime = mime;
      return {trackId: "track-1", uploadToken: "token-1"};
    },
    upload: async (_id: string, _token: string, seq: number, blob: Blob) => {
      if (upload) await upload(seq, blob);
    },
    finish: async (_id: string, _token: string, chunks: number, complete: boolean) => {
      finishes.push({chunks, complete});
    },
    status: (state: "starting" | "recording" | "saving" | "saved" | "failed") => {
      statuses.push(state);
    },
  };
  return {statuses, finishes, callbacks, mime: () => openedMime};
}

test("finish waits for stop event and ordered final upload", async () => {
  install();
  const events: string[] = [];
  const releases: Array<() => void> = [];
  const bag = harness((seq) => new Promise((resolve) => {
    events.push("up:" + seq);
    releases.push(() => { events.push("done:" + seq); resolve(); });
  }));
  const originalFinish = bag.callbacks.finish;
  bag.callbacks.finish = async (id, token, chunks, complete) => {
    events.push("finish:" + chunks + ":" + complete);
    await originalFinish(id, token, chunks, complete);
  };
  const stream = new FakeStream([track("video", "cam"), track("audio", "mic"), track("video", "screen", "monitor")]);
  const cap = new LocalRecordingCapture(stream as unknown as MediaStream, "video", bag.callbacks);
  await cap.start();
  const rec = FakeRecorder.instances[0];
  assert.equal(rec.timeslice, 2000);
  assert.deepEqual(rec.stream.getTracks().map((item) => item.label), ["cam", "mic"]);
  rec.holdEvents = true;
  rec.emit(new Blob(["part"]));
  const stopped = cap.stop();
  assert.equal(cap.stop(), stopped);
  rec.stop();
  assert.equal(events.some((item) => item.startsWith("finish")), false);
  rec.ondataavailable?.({data: new Blob(["tail"])});
  await Promise.resolve();
  assert.deepEqual(events.filter((item) => item.startsWith("up")), ["up:0"]);
  releases[0]();
  await flushEvents();
  assert.ok(events.includes("up:1"));
  assert.equal(events.some((item) => item.startsWith("finish")), false);
  rec.onstop?.();
  await Promise.resolve();
  assert.equal(events.some((item) => item.startsWith("finish")), false);
  releases[1]();
  await stopped;
  assert.ok(events.includes("finish:2:true"));
  assert.deepEqual(events.filter((item) => item.startsWith("up") || item.startsWith("done") || item.startsWith("finish")), [
    "up:0", "done:0", "up:1", "done:1", "finish:2:true",
  ]);
  assert.equal(bag.statuses.includes("saved"), true);
});

test("failed upload finishes incomplete and never reports saved", async () => {
  install();
  let uploads = 0;
  const bag = harness(async (seq) => {
    uploads += 1;
    if (seq === 0) throw new Error("upload failed");
  });
  const stream = new FakeStream([track("audio", "mic"), track("video", "cam")]);
  const cap = new LocalRecordingCapture(stream as unknown as MediaStream, "video", bag.callbacks);
  await cap.start();
  const rec = FakeRecorder.instances[0];
  rec.emit(new Blob(["a"]));
  rec.emit(new Blob(["b"]));
  await cap.stop();
  assert.equal(uploads, 1);
  assert.equal(rec.state, "inactive");
  assert.deepEqual(bag.finishes, [{chunks: 0, complete: false}]);
  assert.equal(bag.statuses.includes("failed"), true);
  assert.equal(bag.statuses.includes("saved"), false);
});

test("audio mode excludes video tracks and chooses audio webm", async () => {
  install();
  const bag = harness();
  const mic = track("audio", "mic");
  const stream = new FakeStream([mic, track("video", "cam"), track("video", "screen", "browser")]);
  const cap = new LocalRecordingCapture(stream as unknown as MediaStream, "audio", bag.callbacks);
  await cap.start();
  const rec = FakeRecorder.instances[0];
  assert.equal(bag.mime(), "audio/webm;codecs=opus");
  assert.equal(rec.mimeType, "audio/webm;codecs=opus");
  assert.deepEqual(rec.stream.getTracks(), [mic]);
  assert.equal(rec.timeslice, 2000);
  cap.abort();
});

test("abort while opening does not record, upload, or update status", async () => {
  install();
  const statuses: string[] = [];
  const finishes: Finish[] = [];
  let uploads = 0;
  let release: (value: {trackId: string; uploadToken: string}) => void = () => {};
  const stream = new FakeStream([track("audio", "mic"), track("video", "cam")]);
  const cap = new LocalRecordingCapture(stream as unknown as MediaStream, "video", {
    open: () => new Promise((resolve) => { release = resolve; }),
    upload: async () => { uploads += 1; },
    finish: async (_id, _token, chunks, complete) => { finishes.push({chunks, complete}); },
    status: (state) => { statuses.push(state); },
  });
  const starting = cap.start();
  assert.deepEqual(statuses, ["starting"]);
  cap.abort();
  cap.abort();
  release({trackId: "track-9", uploadToken: "token-9"});
  await starting;
  assert.equal(FakeRecorder.instances.length, 0);
  assert.equal(uploads, 0);
  assert.deepEqual(finishes, [{chunks: 0, complete: false}]);
  await cap.stop();
  assert.deepEqual(statuses, ["starting"]);
});

test("chunk and queue caps fail incomplete without retry or saved", async () => {
  install();
  let uploads = 0;
  let release: () => void = () => {};
  const bag = harness(() => {
    uploads += 1;
    return new Promise((resolve) => { release = resolve; });
  });
  const stream = new FakeStream([track("audio", "mic")]);
  const cap = new LocalRecordingCapture(stream as unknown as MediaStream, "audio", bag.callbacks);
  await cap.start();
  const rec = FakeRecorder.instances[0];
  rec.emit({size: 8 * 1024 * 1024 + 1} as Blob);
  await cap.stop();
  assert.equal(uploads, 0);
  assert.equal(bag.finishes[0]?.complete, false);
  assert.equal(bag.statuses.includes("saved"), false);

  install();
  uploads = 0;
  const blocked = harness(() => {
    uploads += 1;
    return new Promise((resolve) => { release = resolve; });
  });
  const second = new LocalRecordingCapture(stream as unknown as MediaStream, "audio", blocked.callbacks);
  await second.start();
  const live = FakeRecorder.instances[0];
  const eight = {size: 8 * 1024 * 1024} as Blob;
  live.emit(eight);
  live.emit(eight);
  live.emit(eight);
  live.emit({size: 1} as Blob);
  assert.equal(uploads, 1);
  release();
  await second.stop();
  assert.equal(uploads, 1);
  assert.equal(blocked.statuses.includes("failed"), true);
  assert.equal(blocked.statuses.includes("saved"), false);
  assert.equal(blocked.finishes[0]?.complete, false);
});
