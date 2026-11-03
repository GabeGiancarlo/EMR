import { useRef, useState } from "react";
import type { FormEvent } from "react";
import { clientName, date, label, value } from "./api";
import type { Bootstrap, FormField } from "./api";
import { Button, Icon } from "./ui";
import { serviceSnapshot } from "./programRouting";
export default function NoteComposer({
  data,
  initial,
  onSave,
  onClose,
}: {
  data: Bootstrap;
  initial?: Record<string, unknown>;
  onSave: (body: Record<string, unknown>) => Promise<void>;
  onClose: () => void;
}) {
  const [clientId, setClient] = useState(String(initial?.client_id || ""));
  const [sessionId, setSession] = useState(String(initial?.session_id || ""));
  const [templateId, setTemplate] = useState("");
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [source, setSource] = useState("");
  const [excerpt, setExcerpt] = useState("");
  const [target, setTarget] = useState("narrative");
  const [mapped, setMapped] = useState(false);
  const [reviewed, setReviewed] = useState(false);
  const sourceVersion = useRef(0);
  const template = data.templates.find((t) => t.id === templateId);
  const savedService=serviceSnapshot(data.sessions.find(s=>s.id===sessionId));
  const suggestedTemplate=data.templates.find(t=>t.id===savedService?.note_template_id);
  const fields = (template?.fields || []) as FormField[];
  async function save(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      if (mapped && !reviewed)
        throw new Error(
          "Review the imported excerpts and confirm their accuracy before saving.",
        );
      const sections = fields
        .map((f) => `${f.label}\n${answers[f.key] || ""}`)
        .join("\n\n");
      const narrative = [sections, body].filter(Boolean).join("\n\n");
      if (!narrative.trim())
        throw new Error("Add the clinical narrative before saving.");
      await onSave({
        client_id: clientId,
        session_id: sessionId || null,
        template_id: templateId || null,
        title,
        body: narrative,
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : "Unable to save note.");
    } finally {
      setBusy(false);
    }
  }
  return (
    <form onSubmit={save}>
      <div className="form-body">
        {savedService && <div className="scheduling-zone-note"><strong>{value(savedService,"name")}</strong> · {value(savedService,"service_code")}
          {suggestedTemplate?.active===true ? <><p>The encounter's saved service suggests {value(suggestedTemplate,"name")}. Applying it replaces the current form answers; the narrative remains.</p><Button variant="secondary small" onClick={()=>{if(Object.values(answers).some(Boolean) && !window.confirm("Replace the current form answers with this service's suggested template?"))return;setTemplate(suggestedTemplate.id);setAnswers({});}}>Use suggested template</Button></> : savedService.note_template_id ? <p>The suggested template is inactive or unavailable. Select an available template before documenting.</p> : <p>No note template was configured for this service. Choose the appropriate form for the care delivered.</p>}
        </div>}
        <div className="form-grid">
          <div className="form-field">
            <label htmlFor="note-client">Client *</label>
            <select
              id="note-client"
              required
              value={clientId}
              onChange={(e) => {
                sourceVersion.current += 1;
                setClient(e.target.value);
                setSession("");
                setTitle("");
                setBody("");
                setAnswers({});
                setSource("");
                setExcerpt("");
                setMapped(false);
                setReviewed(false);
                setError("");
              }}
            >
              <option value="">Choose a client</option>
              {data.clients.map((c) => (
                <option key={c.id} value={c.id}>
                  {clientName(c)}
                </option>
              ))}
            </select>
            <small>Changing clients clears this draft's text and source excerpts.</small>
          </div>
          <div className="form-field">
            <label htmlFor="note-session">Linked session</label>
            <select
              id="note-session"
              value={sessionId}
              onChange={(e) => setSession(e.target.value)}
            >
              <option value="">General chart note</option>
              {data.sessions
                .filter((s) => value(s, "client_id") === clientId)
                .map((s) => (
                  <option key={s.id} value={s.id}>
                    {label(value(s, "kind"))} · {date(s.starts_at, true)}
                  </option>
                ))}
            </select>
          </div>
          <div className="form-field">
            <label htmlFor="note-title">Title *</label>
            <input
              id="note-title"
              required
              value={title}
              onChange={(e) => setTitle(e.target.value)}
            />
          </div>
          <div className="form-field">
            <label htmlFor="note-template">Use a form</label>
            <select
              id="note-template"
              value={templateId}
              onChange={(e) => {
                setTemplate(e.target.value);
                setAnswers({});
              }}
            >
              <option value="">Free-form clinical note</option>
              {data.templates
                .filter((t) => t.active !== false)
                .map((t) => (
                  <option key={t.id} value={t.id}>
                    {value(t, "name")}
                  </option>
                ))}
            </select>
          </div>
        </div>
        <details className="transcript-tool">
          <summary>
            <Icon name="notes" size={17} /> Bring in reviewed source text{" "}
            <span>Optional</span>
          </summary>
          <div className="transcript-tool-body">
            <p>
              Paste a transcript or import a plain-text file. Select an excerpt
              and choose where it belongs. Nothing is sent to an AI service, and
              the source text is not saved separately.
            </p>
            <label className="file-import">
              Import .txt file
              <input
                type="file"
                accept=".txt,text/plain"
                onChange={async (e) => {
                  const file = e.target.files?.[0];
                  if (!file) return;
                  if (file.size > 1024 * 1024) {
                    setError("Choose a text file smaller than 1 MB.");
                    return;
                  }
                  const version = ++sourceVersion.current;
                  try {
                    const text = await file.text();
                    if (version !== sourceVersion.current) return;
                    setSource(text);
                    setExcerpt("");
                    setReviewed(false);
                  } catch {
                    if (version !== sourceVersion.current) return;
                    setError("That file could not be read.");
                  }
                }}
              />
            </label>
            <div className="form-field">
              <label htmlFor="source-transcript">Source text</label>
              <textarea
                id="source-transcript"
                rows={5}
                value={source}
                placeholder="Paste source text, then highlight an excerpt to use."
                onChange={(e) => {
                  sourceVersion.current += 1;
                  setSource(e.target.value);
                  setExcerpt("");
                }}
                onSelect={(e) => {
                  const input = e.currentTarget;
                  setExcerpt(
                    input.value.slice(input.selectionStart, input.selectionEnd),
                  );
                }}
              />
            </div>
            <div className="excerpt-mapping">
              <div className="form-field">
                <label htmlFor="excerpt-target">Place excerpt in</label>
                <select
                  id="excerpt-target"
                  value={target}
                  onChange={(e) => setTarget(e.target.value)}
                >
                  <option value="narrative">Additional narrative</option>
                  {fields
                    .filter((f) => !["date", "number"].includes(f.type))
                    .map((f) => (
                      <option value={f.key} key={f.key}>
                        {f.label}
                      </option>
                    ))}
                </select>
              </div>
              <Button
                variant="secondary small"
                disabled={!excerpt.trim()}
                onClick={() => {
                  if (target === "narrative")
                    setBody((b) =>
                      [b, excerpt.trim()].filter(Boolean).join("\n\n"),
                    );
                  else
                    setAnswers((a) => ({
                      ...a,
                      [target]: [a[target], excerpt.trim()]
                        .filter(Boolean)
                        .join("\n\n"),
                    }));
                  setMapped(true);
                  setReviewed(false);
                  setExcerpt("");
                }}
              >
                Add selected excerpt <Icon name="arrow" size={15} />
              </Button>
            </div>
            {mapped && (
              <label className="review-check">
                <input
                  type="checkbox"
                  checked={reviewed}
                  onChange={(e) => setReviewed(e.target.checked)}
                />{" "}
                I reviewed the imported excerpts for accuracy, relevance, and
                the correct client.
              </label>
            )}
          </div>
        </details>
        {template && (
          <div className="template-context">
            <Icon name="notes" size={20} />
            <div>
              <strong>{value(template, "name")}</strong>
              <p>
                {value(template, "description") ||
                  "Complete the fields defined by your organization."}
              </p>
            </div>
          </div>
        )}
        {fields.map((f) => (
          <div className="form-field" key={f.key}>
            <label htmlFor={`answer-${f.key}`}>
              {f.label}
              {f.required ? " *" : ""}
            </label>
            {f.type === "textarea" ? (
              <textarea
                id={`answer-${f.key}`}
                required={f.required}
                rows={4}
                value={answers[f.key] || ""}
                onChange={(e) =>
                  setAnswers({ ...answers, [f.key]: e.target.value })
                }
              />
            ) : (
              <input
                id={`answer-${f.key}`}
                type={["date", "number"].includes(f.type) ? f.type : "text"}
                required={f.required}
                value={answers[f.key] || ""}
                onChange={(e) =>
                  setAnswers({ ...answers, [f.key]: e.target.value })
                }
              />
            )}
          </div>
        ))}
        <div className="form-field">
          <label htmlFor="note-body">
            {fields.length ? "Additional narrative" : "Clinical narrative *"}
          </label>
          <textarea
            id="note-body"
            rows={8}
            value={body}
            onChange={(e) => setBody(e.target.value)}
            placeholder="Document the care delivered, observations, and next steps."
          />
          <small>
            Imported or transcribed content should be reviewed by the author.
            Local audio transcription is available when completing a structured form.
          </small>
        </div>
        {error && (
          <div className="error-banner" role="alert">
            {error}
          </div>
        )}
      </div>
      <div className="modal-foot">
        <span className="form-assurance">Saved as a draft for your review</span>
        <Button variant="secondary" onClick={onClose} disabled={busy}>
          Cancel
        </Button>
        <Button type="submit" disabled={busy}>
          {busy ? "Saving…" : "Save draft"}
          <Icon name="check" size={17} />
        </Button>
      </div>
    </form>
  );
}
