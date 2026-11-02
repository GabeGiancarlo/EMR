import { useEffect, useRef, useState } from "react";
import { actorStorageKey, apiUrl, date, value } from "./api";
import type { Bootstrap } from "./api";
import { MAX_WAV_BYTES, validateWav, WAV_ERROR_MESSAGE } from "./wav";
import { Button, Icon } from "./ui";

export default function SpeechImport({ clientId, data, onTranscript }: {
  clientId: string;
  data: Bootstrap;
  onTranscript: (transcript: string) => void;
}) {
  const [consentId, setConsentId] = useState("");
  const [file, setFile] = useState<File | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [done, setDone] = useState(false);
  const mounted = useRef(true);
  const abortRef = useRef<AbortController | null>(null);
  const client = data.clients.find((row) => row.id === clientId);
  const facility = data.facilities.find((row) => row.id === client?.facility_id);
  const parts = new Intl.DateTimeFormat("en-US", {
    timeZone: value(facility, "timezone") || "UTC", year: "numeric", month: "2-digit", day: "2-digit",
  }).formatToParts(new Date());
  const part = (type: string) => parts.find((item) => item.type === type)?.value || "";
  const today = `${part("year")}-${part("month")}-${part("day")}`;
  const allowed = ["admin", "clinical_lead", "clinician"].includes(value(data.actor, "role"));
  const consents = data.consents.filter((row) => row.client_id === clientId &&
    row.kind === "recording" && !row.revoked_on && value(row, "granted_on") <= today &&
    (!row.expires_on || value(row, "expires_on") >= today));

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; abortRef.current?.abort(); };
  }, []);

  async function transcribe() {
    if (busy || !allowed || !file || !consents.some((row) => row.id === consentId)) return;
    setError("");
    setDone(false);
    if (file.size > MAX_WAV_BYTES) {
      setError(WAV_ERROR_MESSAGE.too_large);
      return;
    }
    const controller = new AbortController();
    abortRef.current = controller;
    setBusy(true);
    try {
      const wav = await file.arrayBuffer();
      if (!mounted.current || controller.signal.aborted) return;
      const checked = validateWav(wav);
      if (!checked.ok) throw new Error(checked.message);
      const actor = localStorage.getItem(actorStorageKey);
      const response = await fetch(apiUrl("/api/speech/transcribe"), {
        method: "POST", signal: controller.signal,
        headers: {
          Accept: "application/json", "Content-Type": "audio/wav",
          ...(actor ? { "x-nexus-actor": actor } : {}),
          "x-nexus-client": clientId, "x-nexus-consent": consentId,
        },
        body: wav,
      });
      const payload = await response.json().catch(() => null);
      if (!response.ok && payload?.error?.code === "speech_unconfigured") {
        throw new Error("Local transcription is not set up for this workspace. Ask your administrator to enable it, or import reviewed text.");
      }
      if (!response.ok) throw new Error(payload?.error?.message || "Local transcription could not be completed.");
      if (payload?.manual_review_required !== true || payload?.engine !== "whisper.cpp" ||
          typeof payload.transcript !== "string" || !payload.transcript.trim()) {
        throw new Error("The recognizer did not return a reviewable transcript.");
      }
      if (!mounted.current || controller.signal.aborted) return;
      onTranscript(payload.transcript);
      setDone(true);
    } catch (err) {
      if (!mounted.current || controller.signal.aborted) return;
      setError(err instanceof Error ? err.message : "Local transcription could not be completed.");
    } finally {
      if (mounted.current) setBusy(false);
    }
  }

  if (!allowed) return null;
  return (
    <section className="speech-import" aria-label="Local audio transcription">
      <div className="speech-heading"><Icon name="activity" size={19} /><strong>Turn a recording into a draft</strong></div>
      <p>Transcription runs on this machine. Review the words before using them in a form.</p>
      {!clientId ? <p>Choose a client to see their recording consents.</p> :
        !consents.length ? <p>No active recording consent is on file. Record consent in the client chart before transcribing.</p> :
        <div className="form-field">
          <label htmlFor="speech-consent">Recording consent</label>
          <select id="speech-consent" value={consentId} disabled={busy} onChange={(e) => { setConsentId(e.target.value); setDone(false); }}>
            <option value="">Choose a recording consent</option>
            {consents.map((row) => <option key={row.id} value={row.id}>{value(row, "scope").slice(0, 80)} · {date(row.granted_on)}</option>)}
          </select>
        </div>}
      <label className="file-import">Choose WAV audio
        <input type="file" accept=".wav,audio/wav" disabled={busy || !clientId || !consents.length}
          onChange={(e) => { setFile(e.target.files?.[0] ?? null); setError(""); setDone(false); }} />
      </label>
      <small>PCM 16-bit mono, 16 kHz · Up to 10 minutes / 20 MiB. A new transcript replaces the source text and clears its field links.</small>
      {error && <div className="error-banner" role="alert">{error}</div>}
      {done && <p className="speech-success" role="status">Draft transcript added below. Review it, then map the excerpts you need.</p>}
      {busy && <p role="status">Transcribing locally… This can take up to two minutes.</p>}
      <Button variant="secondary small" disabled={busy || !clientId || !consentId || !file}
        onClick={() => void transcribe()}>{busy ? "Transcribing…" : "Transcribe recording"}</Button>
    </section>
  );
}
