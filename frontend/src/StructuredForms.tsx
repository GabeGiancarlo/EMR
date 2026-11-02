import { useState } from "react";
import type { FormEvent } from "react";
import { clientName, date, label, value } from "./api";
import type { Bootstrap, FormField, Row } from "./api";
import { Badge, Button, Empty, Icon, Modal, Panel } from "./ui";
import type { WorkflowProps } from "./CarePlanning";
type StructuredField = FormField & { options?: string[] };
type Answers = Record<string, string | number | boolean | null>;
type Provenance = Record<string, { excerpt: string }>;

function ResponseEditor({
  data,
  clientId,
  record,
  onClose,
  onSave,
}: {
  data: Bootstrap;
  clientId?: string;
  record?: Row;
  onClose: () => void;
  onSave: (body: unknown) => Promise<void>;
}) {
  const [client, setClient] = useState(clientId || value(record, "client_id"));
  const [session, setSession] = useState(value(record, "session_id"));
  const [episode, setEpisode] = useState(value(record, "episode_id"));
  const [template, setTemplate] = useState(value(record, "template_id"));
  const [answers, setAnswers] = useState<Answers>(
    (record?.answers || {}) as Answers,
  );
  const [source, setSource] = useState(value(record, "source_excerpt"));
  const [selected, setSelected] = useState("");
  const [target, setTarget] = useState("");
  const [provenance, setProvenance] = useState<Provenance>(
    (record?.field_provenance || {}) as Provenance,
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const snapshot = (record?.template_snapshot ||
    data.templates.find((t) => t.id === template)) as Row | undefined;
  const fields = (snapshot?.fields || []) as StructuredField[];
  async function save(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      await onSave({
        ...(!record
          ? {
              client_id: client,
              session_id: session || null,
              episode_id: episode || null,
              template_id: template,
            }
          : {}),
        answers,
        source_type: source.trim() ? "transcript" : "manual",
        source_excerpt: source,
        field_provenance: provenance,
      });
      onClose();
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "The form could not be saved.",
      );
    } finally {
      setBusy(false);
    }
  }
  const update = (key: string, answer: Answers[string]) =>
    setAnswers((a) => ({ ...a, [key]: answer }));
  return (
    <Modal
      title={record ? "Edit form response" : "Start a form response"}
      description="Save a partial draft, review each answer, then sign. The form definition is preserved with the response."
      onClose={onClose}
      wide
    >
      <form onSubmit={save}>
        <div className="form-body">
          {!record ? (
            <div className="form-grid">
              <div className="form-field">
                <label htmlFor="response-client">Client *</label>
                <select
                  id="response-client"
                  required
                  value={client}
                  onChange={(e) => {
                    setClient(e.target.value);
                    setSession("");
                    setEpisode("");
                  }}
                >
                  <option value="">Choose a client</option>
                  {data.clients.map((c) => (
                    <option key={c.id} value={c.id}>
                      {clientName(c)}
                    </option>
                  ))}
                </select>
              </div>
              <div className="form-field">
                <label htmlFor="response-template">Form template *</label>
                <select
                  id="response-template"
                  required
                  value={template}
                  onChange={(e) => {
                    setTemplate(e.target.value);
                    setAnswers({});
                    setProvenance({});
                    setTarget("");
                  }}
                >
                  <option value="">Choose a template</option>
                  {data.templates
                    .filter((t) => t.active !== false)
                    .map((t) => (
                      <option key={t.id} value={t.id}>
                        {value(t, "name")}
                      </option>
                    ))}
                </select>
              </div>
              <div className="form-field">
                <label htmlFor="response-episode">Care episode</label>
                <select
                  id="response-episode"
                  value={episode}
                  onChange={(e) => setEpisode(e.target.value)}
                >
                  <option value="">Not linked</option>
                  {data.episodes
                    .filter((ep) => ep.client_id === client)
                    .map((ep) => (
                      <option key={ep.id} value={ep.id}>
                        {label(value(ep, "level_of_care"))} ·{" "}
                        {date(ep.start_date)}
                      </option>
                    ))}
                </select>
              </div>
              <div className="form-field">
                <label htmlFor="response-session">Session</label>
                <select
                  id="response-session"
                  value={session}
                  onChange={(e) => setSession(e.target.value)}
                >
                  <option value="">General client form</option>
                  {data.sessions
                    .filter((s) => s.client_id === client)
                    .map((s) => (
                      <option key={s.id} value={s.id}>
                        {label(value(s, "kind"))} · {date(s.starts_at, true)}
                      </option>
                    ))}
                </select>
              </div>
            </div>
          ) : (
            <div className="response-context">
              <Icon name="notes" size={22} />
              <div>
                <strong>{value(snapshot, "name")}</strong>
                <p>
                  {clientName(data.clients.find((c) => c.id === client))} ·
                  Original form definition preserved
                </p>
              </div>
            </div>
          )}
          {fields.length > 0 && (
            <>
              <details className="transcript-tool">
                <summary>
                  <Icon name="notes" size={17} /> Map source excerpts to fields{" "}
                  <span>Manual review</span>
                </summary>
                <div className="transcript-tool-body">
                  <p>
                    Paste reviewed text or import a transcript. Highlight an
                    excerpt and choose a field. Source excerpts are retained
                    with this form for traceability; no AI or speech service is
                    called.
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
                        setSource(await file.text());
                        setProvenance({});
                        setSelected("");
                      }}
                    />
                  </label>
                  <div className="form-field">
                    <label htmlFor="response-source">Source text</label>
                    <textarea
                      id="response-source"
                      rows={5}
                      value={source}
                      onChange={(e) => {
                        setSource(e.target.value);
                        setProvenance({});
                        setSelected("");
                      }}
                      onSelect={(e) =>
                        setSelected(
                          e.currentTarget.value.slice(
                            e.currentTarget.selectionStart,
                            e.currentTarget.selectionEnd,
                          ),
                        )
                      }
                    />
                    <small>
                      Changing the source clears previous field provenance
                      links. Review and remap those excerpts before saving.
                    </small>
                  </div>
                  <div className="excerpt-mapping">
                    <div className="form-field">
                      <label htmlFor="response-target">Destination field</label>
                      <select
                        id="response-target"
                        value={target}
                        onChange={(e) => setTarget(e.target.value)}
                      >
                        <option value="">Choose a field</option>
                        {fields
                          .filter((f) => ["text", "textarea"].includes(f.type))
                          .map((f) => (
                            <option key={f.key} value={f.key}>
                              {f.label}
                            </option>
                          ))}
                      </select>
                    </div>
                    <Button
                      variant="secondary small"
                      disabled={!selected.trim() || !target}
                      onClick={() => {
                        update(target, selected.trim());
                        setProvenance((p) => ({
                          ...p,
                          [target]: { excerpt: selected.trim() },
                        }));
                        setSelected("");
                      }}
                    >
                      Use selected excerpt <Icon name="arrow" size={15} />
                    </Button>
                  </div>
                </div>
              </details>
              <div className="structured-answers">
                {fields.map((f) => (
                  <div className="form-field" key={f.key}>
                    <label htmlFor={`response-field-${f.key}`}>
                      {f.label}
                      {f.required && (
                        <span className="required"> · Required for review</span>
                      )}
                    </label>
                    {f.type === "checkbox" ? (
                      <label className="review-check">
                        <input
                          id={`response-field-${f.key}`}
                          type="checkbox"
                          checked={Boolean(answers[f.key])}
                          onChange={(e) => update(f.key, e.target.checked)}
                        />{" "}
                        Confirmed
                      </label>
                    ) : f.type === "select" ? (
                      <select
                        id={`response-field-${f.key}`}
                        value={String(answers[f.key] ?? "")}
                        onChange={(e) => update(f.key, e.target.value || null)}
                      >
                        <option value="">Not answered</option>
                        {f.options?.map((o) => (
                          <option key={o} value={o}>
                            {o}
                          </option>
                        ))}
                      </select>
                    ) : f.type === "textarea" ? (
                      <textarea
                        id={`response-field-${f.key}`}
                        rows={4}
                        value={String(answers[f.key] ?? "")}
                        onChange={(e) => update(f.key, e.target.value)}
                      />
                    ) : (
                      <input
                        id={`response-field-${f.key}`}
                        type={
                          ["date", "number"].includes(f.type) ? f.type : "text"
                        }
                        value={String(answers[f.key] ?? "")}
                        onInput={
                          f.type === "date"
                            ? (e) => update(f.key, e.currentTarget.value)
                            : undefined
                        }
                        onChange={(e) =>
                          update(
                            f.key,
                            f.type === "number"
                              ? e.target.value === ""
                                ? null
                                : Number(e.target.value)
                              : e.target.value,
                          )
                        }
                      />
                    )}{" "}
                    {provenance[f.key] && (
                      <div className="field-provenance">
                        <Icon name="link" size={14} />
                        <span>Source: “{provenance[f.key].excerpt}”</span>
                        <button
                          type="button"
                          className="icon-button"
                          aria-label={`Remove source link for ${f.label}`}
                          onClick={() =>
                            setProvenance((p) =>
                              Object.fromEntries(
                                Object.entries(p).filter(
                                  ([key]) => key !== f.key,
                                ),
                              ),
                            )
                          }
                        >
                          <Icon name="close" size={13} />
                        </button>
                      </div>
                    )}
                  </div>
                ))}
              </div>
            </>
          )}
          {error && (
            <div className="error-banner" role="alert">
              {error}
            </div>
          )}
        </div>
        <div className="modal-foot">
          <span className="form-assurance">
            Partial drafts can be saved. Review is a separate step.
          </span>
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button type="submit" disabled={busy || !template}>
            {busy ? "Saving…" : "Save draft"}
            <Icon name="check" size={16} />
          </Button>
        </div>
      </form>
    </Modal>
  );
}

export default function StructuredForms({
  data,
  clientId,
  canWrite,
  action,
  mutate,
}: WorkflowProps & { clientId?: string }) {
  const all =
    (data as Bootstrap & { form_responses?: Row[] }).form_responses || [];
  const responses = all.filter((r) => !clientId || r.client_id === clientId);
  const [editor, setEditor] = useState<Row | null | undefined>(undefined);
  const [view, setView] = useState<Row | null>(null);
  const rowTitle = (r: Row) =>
    value(r.template_snapshot as Row, "name") || "Form response";
  return (
    <>
      <Panel
        title="Structured forms"
        subtitle="Answers stay linked to their original form, review history, and source excerpts."
        action={
          canWrite ? (
            <Button
              variant="secondary small"
              onClick={() => setEditor(null)}
              disabled={!data.templates.some((t) => t.active !== false)}
            >
              <Icon name="plus" size={16} /> Start a form
            </Button>
          ) : undefined
        }
      >
        {responses.length ? (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Form</th>
                  <th>Client</th>
                  <th>Created</th>
                  <th>Status</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {responses.map((r) => (
                  <tr key={r.id}>
                    <td>
                      <strong>{rowTitle(r)}</strong>
                      <small>
                        {r.source_type === "transcript"
                          ? "Source excerpts linked"
                          : "Manual entry"}
                      </small>
                    </td>
                    <td>
                      {clientName(
                        data.clients.find((c) => c.id === r.client_id),
                      )}
                    </td>
                    <td>{date(r.created_at)}</td>
                    <td>
                      <Badge status={r.status} />
                    </td>
                    <td>
                      <div className="row-actions">
                        <Button variant="text small" onClick={() => setView(r)}>
                          View
                        </Button>
                        {canWrite && r.status !== "signed" && (
                          <Button
                            variant="text small"
                            onClick={() => setEditor(r)}
                          >
                            Edit draft
                          </Button>
                        )}
                        {canWrite && r.status === "draft" && (
                          <Button
                            variant="text small"
                            onClick={() =>
                              action(
                                `/api/form_responses/${r.id}/review`,
                                "Review form response",
                                "Confirm that you have checked each answer for completeness, accuracy, and the correct client. Required fields will be validated.",
                                [],
                                {},
                                "Record review",
                              )
                            }
                          >
                            Review
                          </Button>
                        )}
                        {canWrite && r.status === "reviewed" && (
                          <Button
                            variant="text small"
                            onClick={() =>
                              action(
                                `/api/form_responses/${r.id}/sign`,
                                "Sign form response",
                                "Attest to the reviewed answers. The signed record and original form definition will become immutable.",
                                [],
                                {},
                                "Sign form",
                              )
                            }
                          >
                            Sign
                          </Button>
                        )}
                        {canWrite && r.status === "signed" && (
                          <>
                            <Button
                              variant="text small"
                              onClick={() =>
                                action(
                                  `/api/form_responses/${r.id}/create-note`,
                                  "Create note from signed form",
                                  "Use the actual field labels and answers to create a draft note. No clinical content will be invented. Review the note before signing it.",
                                  [
                                    {
                                      key: "title",
                                      label: "Note title",
                                      required: true,
                                    },
                                  ],
                                  { title: rowTitle(r) },
                                  "Create draft note",
                                )
                              }
                            >
                              Create note
                            </Button>
                            <Button
                              variant="text small"
                              onClick={() =>
                                action(
                                  `/api/form_responses/${r.id}/amend`,
                                  "Amend form response",
                                  "Preserve the signed response and create a new draft linked to it.",
                                  [
                                    {
                                      key: "reason",
                                      label: "Reason for amendment",
                                      type: "textarea",
                                      required: true,
                                    },
                                  ],
                                  {},
                                  "Create amendment",
                                )
                              }
                            >
                              Amend
                            </Button>
                          </>
                        )}
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <Empty
            icon="notes"
            title={
              data.templates.length
                ? "A clear form. A considered response."
                : "Your own forms, ready for your team."
            }
            description={
              data.templates.length
                ? "Select a form, complete its fields, and review the response before signing. Source excerpts can be mapped explicitly."
                : "Create a template in Workspace settings → Forms & templates, then return here to complete it for a client."
            }
            action={
              canWrite && data.templates.some((t) => t.active !== false) ? (
                <Button variant="secondary" onClick={() => setEditor(null)}>
                  Start the first response
                </Button>
              ) : undefined
            }
          />
        )}
      </Panel>
      {editor !== undefined && (
        <ResponseEditor
          data={data}
          clientId={clientId}
          record={editor || undefined}
          onClose={() => setEditor(undefined)}
          onSave={(body) =>
            mutate(
              `/api/form_responses${editor ? `/${editor.id}` : ""}`,
              body,
              editor ? "PATCH" : "POST",
            )
          }
        />
      )}
      {view && (
        <Modal
          title={rowTitle(view)}
          description={`${clientName(data.clients.find((c) => c.id === view.client_id))} · ${date(view.created_at, true)}`}
          onClose={() => setView(null)}
          wide
        >
          <div className="form-body">
            <div className="note-meta">
              <Badge status={view.status} />
              <span>
                {view.reviewed_at
                  ? `Reviewed ${date(view.reviewed_at, true)}`
                  : "Review pending"}
              </span>
              {Boolean(view.signed_at) && (
                <span>Signed {date(view.signed_at, true)}</span>
              )}
            </div>
            <div className="response-readonly">
              {(
                ((view.template_snapshot as Row)
                  ?.fields as StructuredField[]) || []
              ).map((f) => {
                const answer = (view.answers as Answers)?.[f.key];
                const source = (view.field_provenance as Provenance)?.[f.key]
                  ?.excerpt;
                return (
                  <section key={f.key}>
                    <h3>{f.label}</h3>
                    <p>
                      {answer === true
                        ? "Confirmed"
                        : answer === false
                          ? "Not confirmed"
                          : answer == null || answer === ""
                            ? "Not answered"
                            : String(answer)}
                    </p>
                    {source && (
                      <blockquote>Source excerpt: “{source}”</blockquote>
                    )}
                  </section>
                );
              })}
            </div>
          </div>
          <div className="modal-foot">
            <Button variant="secondary" onClick={() => setView(null)}>
              Close
            </Button>
          </div>
        </Modal>
      )}
    </>
  );
}
