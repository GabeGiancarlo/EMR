import type {CaptureCallbacks} from "./recordingTypes";

const SLICE_MS = 2000;
const CHUNK_CAP = 8 * 1024 * 1024;
const QUEUE_CAP = 16 * 1024 * 1024;

interface RecorderHandle {
  state: string;
  start(timeslice?: number): void;
  stop(): void;
  ondataavailable: ((ev: {data?: Blob}) => void) | null;
  onstop: (() => void) | null;
  onerror: ((ev: {error?: unknown}) => void) | null;
}

type RecorderCtor = {
  new (stream: MediaStream, options?: {mimeType?: string}): RecorderHandle;
  isTypeSupported(type: string): boolean;
};

type StatusName = "starting" | "recording" | "saving" | "saved" | "failed";

function recorderCtor(): RecorderCtor {
  const ctor = (globalThis as unknown as {MediaRecorder?: RecorderCtor}).MediaRecorder;
  if (!ctor?.isTypeSupported) throw new Error("MediaRecorder unavailable");
  return ctor;
}

function chooseMime(mode: "audio" | "video"): string {
  const mimes = mode === "audio"
    ? ["audio/webm;codecs=opus", "audio/webm"]
    : ["video/webm;codecs=vp9,opus", "video/webm;codecs=vp8,opus", "video/webm"];
  const ctor = recorderCtor();
  for (const mime of mimes) if (ctor.isTypeSupported(mime)) return mime;
  throw new Error("No supported webm mime type");
}

function selectTracks(stream: MediaStream, mode: "audio" | "video"): MediaStreamTrack[] {
  return stream.getTracks().filter((track) => {
    if (track.kind === "audio") return true;
    if (mode !== "video" || track.kind !== "video") return false;
    const settings = track.getSettings?.() as {displaySurface?: string} | undefined;
    return !settings?.displaySurface;
  });
}

function subsetStream(tracks: MediaStreamTrack[]): MediaStream {
  const Stream = (globalThis as unknown as {MediaStream?: new (tracks: MediaStreamTrack[]) => MediaStream}).MediaStream;
  if (!Stream) throw new Error("MediaStream unavailable");
  return new Stream(tracks);
}

export class LocalRecordingCapture {
  private stream: MediaStream;
  private mode: "audio" | "video";
  private callbacks: CaptureCallbacks;
  private recorder: RecorderHandle | null = null;
  private trackId = "";
  private uploadToken = "";
  private opened = false;
  private uploaded = 0;
  private queue: Blob[] = [];
  private queuedBytes = 0;
  private uploading = false;
  private abortRequested = false;
  private stopRequested = false;
  private failed = false;
  private failureReported = false;
  private reporting = false;
  private finishCalled = false;
  private recorderDone = false;
  private failError: unknown = undefined;
  private startTask: Promise<void> | null = null;
  private stopTask: Promise<void> | null = null;
  private drainWaiters: Array<() => void> = [];

  constructor(stream: MediaStream, mode: "audio" | "video", callbacks: CaptureCallbacks) {
    this.stream = stream;
    this.mode = mode;
    this.callbacks = callbacks;
  }

  start(): Promise<void> {
    if (!this.startTask) this.startTask = this.openAndRecord();
    return this.startTask;
  }

  stop(): Promise<void> {
    if (!this.stopTask) {
      this.stopRequested = true;
      this.stopTask = this.completeStop();
    }
    return this.stopTask;
  }

  abort(): void {
    if (this.abortRequested) return;
    this.abortRequested = true;
    this.queue = [];
    this.queuedBytes = 0;
    this.silenceRecorder();
    if (!this.uploading) this.finishAborted();
    this.notify();
  }

  private async openAndRecord(): Promise<void> {
    if (this.abortRequested) return;
    this.emitStatus("starting", "Starting recording");
    try {
      const mime = chooseMime(this.mode);
      const tracks = selectTracks(this.stream, this.mode);
      if (tracks.length === 0) throw new Error("No eligible tracks");
      const session = await this.callbacks.open(mime);
      this.opened = true;
      this.trackId = session.trackId;
      this.uploadToken = session.uploadToken;
      if (this.abortRequested) {
        this.finishAborted();
        return;
      }
      if (this.stopRequested) {
        this.recorderDone = true;
        return;
      }
      const recorder = new (recorderCtor())(subsetStream(tracks), {mimeType: mime});
      this.recorder = recorder;
      recorder.ondataavailable = (ev) => this.onData(ev?.data);
      recorder.onstop = () => {
        this.recorderDone = true;
        this.notify();
        if (!this.stopRequested && !this.failed && !this.abortRequested) void this.stop();
      };
      recorder.onerror = (ev) => {
        void this.fail(ev?.error ?? new Error("MediaRecorder error"));
      };
      recorder.start(SLICE_MS);
      if (this.abortRequested) {
        this.queue = [];
        this.queuedBytes = 0;
        this.silenceRecorder();
        this.finishAborted();
        return;
      }
      if (this.stopRequested) {
        this.requestRecorderStop();
        return;
      }
      this.emitStatus("recording", "Recording");
    } catch (err) {
      await this.fail(err);
    }
  }

  private onData(blob: Blob | undefined): void {
    if (!blob || blob.size === 0 || this.abortRequested || this.failed) return;
    if (blob.size > CHUNK_CAP || this.queuedBytes + blob.size > QUEUE_CAP) {
      const message = blob.size > CHUNK_CAP ? "Chunk exceeds 8 MiB" : "Queued media exceeds 16 MiB";
      void this.fail(new Error(message));
      return;
    }
    this.queue.push(blob);
    this.queuedBytes += blob.size;
    this.pump();
  }

  private pump(): void {
    if (this.uploading || this.failed || this.abortRequested) return;
    const blob = this.queue.shift();
    if (!blob) {
      this.notify();
      return;
    }
    this.queuedBytes -= blob.size;
    const sequence = this.uploaded;
    this.uploading = true;
    void this.callbacks.upload(this.trackId, this.uploadToken, sequence, blob).then(() => {
      this.uploading = false;
      this.uploaded += 1;
      if (this.abortRequested) {
        this.finishAborted();
        this.notify();
      } else if (this.failed) void this.reportFailure();
      else this.pump();
    }, (err: unknown) => {
      this.uploading = false;
      if (this.abortRequested) {
        this.finishAborted();
        this.notify();
        return;
      }
      void this.fail(err);
    });
  }

  private async completeStop(): Promise<void> {
    if (this.startTask) await this.startTask;
    if (this.failed) { await this.waitDrain(); return; }
    if (this.abortRequested || this.finishCalled || !this.opened) return;
    this.emitStatus("saving", "Saving recording");
    this.requestRecorderStop();
    await this.waitDrain();
    if (this.abortRequested || this.failed || this.finishCalled) return;
    this.finishCalled = true;
    try {
      await this.callbacks.finish(this.trackId, this.uploadToken, this.uploaded, this.uploaded > 0);
    } catch (err) {
      this.failed = true;
      this.failureReported = true;
      this.emitStatus("failed", err instanceof Error ? err.message : "Finish failed");
      return;
    }
    if (!this.abortRequested && !this.failed) this.emitStatus(this.uploaded > 0 ? "saved" : "failed", this.uploaded > 0 ? "This device’s recording segment is saved" : "No media was captured; the track is incomplete");
  }

  private async fail(err: unknown): Promise<void> {
    if (this.failed || this.abortRequested) return;
    this.failed = true;
    this.failError = err;
    this.queue = [];
    this.queuedBytes = 0;
    this.silenceRecorder();
    if (!this.uploading) await this.reportFailure();
  }

  private async reportFailure(): Promise<void> {
    if (this.failureReported || this.reporting) return;
    this.reporting = true;
    if (!this.finishCalled && this.opened && !this.abortRequested) {
      this.finishCalled = true;
      try {
        await this.callbacks.finish(this.trackId, this.uploadToken, this.uploaded, false);
      } catch { /* incomplete finish is best-effort */ }
    }
    if (!this.abortRequested) {
      const reason = this.failError;
      this.callbacks.status("failed", reason instanceof Error ? reason.message : "Recording failed");
    }
    this.failureReported = true;
    this.reporting = false;
    this.notify();
  }

  private finishAborted(): void {
    if (this.finishCalled || !this.opened) return;
    this.finishCalled = true;
    void this.callbacks.finish(this.trackId, this.uploadToken, this.uploaded, false).catch(() => undefined);
  }

  private requestRecorderStop(): void {
    if (!this.recorder) {
      this.recorderDone = true;
      this.notify();
      return;
    }
    // The inactive state may precede the final dataavailable/stop events.
    if (this.recorder.state === "inactive") return;
    this.recorder.stop();
  }

  private silenceRecorder(): void {
    const recorder = this.recorder;
    if (recorder) {
      recorder.ondataavailable = null;
      recorder.onerror = null;
      recorder.onstop = null;
      if (recorder.state !== "inactive") {
        try { recorder.stop(); } catch { /* discard path */ }
      }
    }
    this.recorderDone = true;
    this.notify();
  }

  private notify(): void {
    if (!this.recorderDone || this.uploading || this.queue.length > 0) return;
    if (this.failed && !this.failureReported) return;
    const waiters = this.drainWaiters.splice(0);
    for (const waiter of waiters) waiter();
  }

  private waitDrain(): Promise<void> {
    if (this.recorderDone && !this.uploading && this.queue.length === 0 && (!this.failed || this.failureReported)) {
      return Promise.resolve();
    }
    return new Promise((resolve) => { this.drainWaiters.push(resolve); });
  }

  private emitStatus(state: StatusName, message: string): void {
    if (this.abortRequested) return;
    this.callbacks.status(state, message);
  }
}
