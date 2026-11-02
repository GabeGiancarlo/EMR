import { useState } from "react";
import { Button } from "./ui";
import type { RecordingContext } from "./recordingTypes";
import "./recording.css";

export function RecordingConsentPanel({
  context,
  selfId,
  host,
  busy,
  captureStatus,
  onAction,
}: {
  context: RecordingContext;
  selfId: string;
  host: boolean;
  busy: boolean;
  captureStatus: string;
  onAction: (action: string, body?: Record<string, unknown>) => Promise<void>;
}) {
  const [policyId, setPolicyId] = useState("");
  const [mode, setMode] = useState<"audio" | "video">("audio");
  const [confirmKey, setConfirmKey] = useState("");
  const active = context.policies.filter((p) => p.active);
  const current = context.current;
  const closed = !!current && ["ended", "declined", "interrupted"].includes(current.state);
  const policy = active.find((p) => p.id === policyId) ?? active[0];
  const chosen: "audio" | "video" = policy?.allow_video && mode === "video" ? "video" : "audio";
  const scopeKey = `${current?.id ?? "none"}:${current?.state ?? ""}:${policy?.id ?? ""}:${chosen}`;
  const confirmed = confirmKey === scopeKey;
  const self = current?.participants.find((p) => p.participant_id === selfId);
  const allOk =
    !!current &&
    current.participants.length > 0 &&
    current.participants.every((p) => p.decision === "accepted" && p.eligible);
  const saving = /saving/i.test(captureStatus);
  const go = (action: string, body: Record<string, unknown> = {}) => void onAction(action, body);
  const confirm = (on: boolean) => setConfirmKey(on ? scopeKey : "");
  const groups = [
    ["pending", "Pending"],
    ["accepted", "Accepted"],
    ["declined", "Declined"],
    ["withdrawn", "Withdrawn"],
  ] as const;
  return (
    <section className="rec-panel" role="region" aria-label="Recording consent">
      <div className="rec-head">
        <h2>Recording consent</h2>
        {current?.state === "recording" && (
          <strong className="rec-live" role="status" aria-live="assertive" aria-label="Recording in progress">RECORDING</strong>
        )}
      </div>
      <p className={`rec-status${saving ? " rec-saving" : ""}`} role="status" aria-live="polite" aria-label="Capture status">
        {saving ? "SAVING" : "Status"}: {captureStatus || (current?.state === "recording" ? "recording" : "idle")}
      </p>
      <p className="rec-scope">
        Each device records its own microphone and, if selected, camera. Shared screens are excluded. Declining stops the recording, not the care visit. The care team reviews saved recordings separately.
      </p>
      {!context.configured && (
        <p className="rec-note" role="status">The media vault needs local setup before a recording can start.</p>
      )}
      {closed && current && (
        <p className="rec-reason" role="status">
          Previous recording {current.state}.{current.reason ? ` Reason: ${current.reason}` : ""}
        </p>
      )}
      {current && (
        <>
          <p className="rec-meta" id="rec-details">
            <strong>{current.policy_title}</strong> — {current.purpose} Retention review: {current.retention_days} days; this is a review date, not automatic deletion. Mode: {current.mode === "video" ? "your camera and microphone" : "audio only"}. State: {current.state}.
          </p>
          <div className="rec-roster" aria-label="Pending, accepted, and declined roster">
            {groups.filter(([decision])=>current.participants.some(p=>p.decision===decision)).map(([decision, title]) => (
              <div key={decision}>
                <h3>{title}</h3>
                <ul aria-label={`${title} participants`}>
                  {current.participants.filter((p) => p.decision === decision).map((p) => (
                    <li key={p.participant_id}>
                      {p.label} ({p.role}{p.participant_id === selfId ? ", you" : ""}{p.eligible ? "" : ", not eligible"})
                    </li>
                  ))}
                  {current.participants.every((p) => p.decision !== decision) && <li>None</li>}
                </ul>
              </div>
            ))}
          </div>
        </>
      )}
      {current && !closed && self?.decision === "pending" && (
        <form className="rec-form" aria-label="Respond to recording request" aria-describedby="rec-details" onSubmit={(e) => e.preventDefault()}>
          <p className="rec-note">Review the purpose, retention review period, and capture scope before you accept.</p>
          <label className="rec-check">
            <input type="checkbox" checked={confirmed} onChange={(e) => confirm(e.target.checked)} aria-label="I explicitly confirm the purpose, retention, and capture scope." />
            I explicitly confirm the purpose, retention, and capture scope.
          </label>
          <div className="rec-actions" role="group" aria-label="Consent response">
            <Button disabled={busy || !confirmed} onClick={() => go("accept", {})}>Accept recording</Button>
            <Button variant="danger" disabled={busy} onClick={() => go("decline", {})}>Decline recording</Button>
          </div>
        </form>
      )}
      {current && !closed && self?.decision === "accepted" && (
        <div className="rec-actions" role="group" aria-label="Withdraw consent">
          <Button variant="danger" disabled={busy} onClick={() => go("withdraw", {})}>Withdraw consent</Button>
        </div>
      )}
      {host && current?.state === "requested" && allOk && context.configured && (
        <div className="rec-actions" role="group" aria-label="Start recording">
          <Button disabled={busy} onClick={() => go("start", {})}>Start recording</Button>
        </div>
      )}
      {host && current?.state === "requested" && context.configured && !allOk && (
        <p className="rec-note">Start is available only after every participant has accepted and is eligible.</p>
      )}
      {host && current && (current.state === "recording" || current.state === "paused") && (
        <div className="rec-actions" role="group" aria-label="Recording controls">
          {current.state === "recording" && <Button disabled={busy} onClick={() => go("pause", {})}>Pause recording</Button>}
          {current.state === "paused" && <Button disabled={busy} onClick={() => go("resume", {})}>Resume recording</Button>}
          <Button variant="danger" disabled={busy} onClick={() => go("stop", {})}>Stop recording</Button>
        </div>
      )}
      {(!current || closed) && host && (
        <form
          className="rec-form"
          aria-label="Request a recording"
          onSubmit={(e) => {
            e.preventDefault();
            if (!policy || busy || !confirmed) return;
            go("request", { policy_id: policy.id, mode: chosen });
          }}
        >
          <label htmlFor="rec-policy">
            Active policy
            <select id="rec-policy" aria-label="Active recording policy" value={policy?.id ?? ""} disabled={busy || !policy} onChange={(e) => setPolicyId(e.target.value)}>
              {active.map((p) => (
                <option key={p.id} value={p.id}>{p.title}</option>
              ))}
            </select>
          </label>
          {policy && <p className="rec-meta">Purpose: {policy.purpose} Retention review: {policy.retention_days} days.</p>}
          <fieldset>
            <legend>Capture mode</legend>
            <label className="rec-check">
              <input type="radio" name="recording-mode" value="audio" checked={chosen === "audio"} disabled={busy} onChange={() => setMode("audio")} />
              Audio only (your microphone)
            </label>
            {policy?.allow_video && (
              <label className="rec-check">
                <input type="radio" name="recording-mode" value="video" checked={chosen === "video"} disabled={busy} onChange={() => setMode("video")} />
                Your camera and microphone
              </label>
            )}
          </fieldset>
          <label className="rec-check">
            <input type="checkbox" checked={confirmed} onChange={(e) => confirm(e.target.checked)} aria-label="I explicitly confirm this policy, purpose, and retention before requesting." />
            I explicitly confirm this policy, purpose, and retention before requesting.
          </label>
          {!policy && <p className="rec-note">No active recording policy is available.</p>}
          <div className="rec-actions">
            <Button type="submit" disabled={busy || !policy || !confirmed}>Request recording</Button>
          </div>
        </form>
      )}
      {(!current || closed) && !host && <p className="rec-note">No recording request is open.</p>}
    </section>
  );
}
