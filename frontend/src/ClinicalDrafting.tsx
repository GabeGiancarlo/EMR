import { useEffect, useRef, useState } from "react";
import { request, date, label, value, type Bootstrap, type Row } from "./api";
import { Button } from "./ui";
import "./drafting.css";
export type EvidencePick = {
  id: string;
  title: string;
  text: string;
  source_kind: string;
  source_date: string;
};
type Citation = { source_id: string; quote: string };
type Statement = { text: string; citations: Citation[] };
type Output = {
  model: string;
  model_sha256: string;
  prompt_version: string;
  sections: { key: string; label: string; statements: Statement[] }[];
  unresolved: string[];
};
type Decision = {
  section_key: string;
  index: number;
  decision: string;
  reason: string;
};
type Job = {
  id: string;
  title: string;
  state: string;
  created_at: string;
  error_code: string;
  output?: Output;
  sources?: EvidencePick[];
  review?: {
    decision: string;
    reason: string;
    note_id?: string;
    decisions?: Decision[];
  } | null;
};
export default function ClinicalDrafting({
  clientId,
  data,
  sources,
  onNoteCreated,
  onReviewDirty,
}: {
  clientId: string;
  data: Bootstrap;
  sources: EvidencePick[];
  onNoteCreated: (note: Row) => void | Promise<void>;
  onReviewDirty: (dirty: boolean) => void;
}) {
  const [jobs, setJobs] = useState<Job[]>([]),
    [configured, setConfigured] = useState(false),
    [selected, setSelected] = useState<Job | null>(null);
  const [template, setTemplate] = useState(""),
    [episode, setEpisode] = useState(""),
    [session, setSession] = useState(""),
    [title, setTitle] = useState(""),
    [purpose, setPurpose] = useState("");
  const [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [notice, setNotice] = useState("");
  const [decisions, setDecisions] = useState<Record<string, Decision>>({}),
    [reason, setReason] = useState(""),
    [attested, setAttested] = useState(false);
  const live = useRef(true),
    epoch = useRef(0),
    selectionEpoch = useRef(0);
  const key = (section: string, index: number) => `${section}:${index}`;
  async function refresh() {
    const sequence = epoch.current;
    const r = await request<{ configured: boolean; jobs: Job[] }>(
      `/api/clients/${clientId}/clinical-drafts`,
    );
    if (live.current && sequence === epoch.current) {
      setJobs(r.jobs);
      setConfigured(r.configured);
    }
  }
  useEffect(() => {
    live.current = true;
    epoch.current++;
    setJobs([]);
    setSelected(null);
    setDecisions({});
    setReason("");
    setAttested(false);
    setError("");
    setNotice("");
    setTemplate("");
    setEpisode("");
    setSession("");
    setTitle("");
    setPurpose("");
    void refresh().catch((e) => {
      if (live.current) setError(e.message);
    });
    return () => {
      live.current = false;
      epoch.current++;
      selectionEpoch.current++;
    };
  }, [clientId]);
  const dirty =
    !selected?.review &&
    (Object.keys(decisions).length > 0 || !!reason || attested);
  useEffect(() => {
    const warn = (e: BeforeUnloadEvent) => {
      if (dirty) {
        e.preventDefault();
        e.returnValue = "";
      }
    };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [dirty]);
  useEffect(() => {
    onReviewDirty(dirty);
    return () => onReviewDirty(false);
  }, [dirty, onReviewDirty]);
  const active = jobs.some((j) => ["queued", "running"].includes(j.state));
  useEffect(() => {
    if (!active) return;
    const timer = setInterval(
      () =>
        void refresh().catch((e) => {
          if (live.current) setError(e.message);
        }),
      3000,
    );
    return () => clearInterval(timer);
  }, [active, clientId]);
  useEffect(() => {
    if (
      selected &&
      ["queued", "running"].includes(selected.state) &&
      jobs.some((j) => j.id === selected.id && j.state !== selected.state)
    )
      void open(selected.id).catch((e) => {
        if (live.current) setError(e.message);
      });
  }, [jobs, selected?.id, selected?.state]);
  async function open(id: string) {
    const sequence = ++selectionEpoch.current;
    const generation = epoch.current;
    const row = await request<Job>(`/api/clinical-drafts/${id}`);
    if (
      live.current &&
      generation === epoch.current &&
      sequence === selectionEpoch.current
    ) {
      setSelected(row);
      setDecisions({});
      setReason("");
      setAttested(false);
    }
  }
  async function run(work: () => Promise<void>) {
    if (busy) return;
    const sequence = epoch.current;
    setBusy(true);
    setError("");
    setNotice("");
    try {
      await work();
    } catch (e) {
      if (live.current && sequence === epoch.current)
        setError(
          e instanceof Error
            ? e.message
            : "The request could not be completed.",
        );
    } finally {
      if (live.current && sequence === epoch.current) setBusy(false);
    }
  }
  async function create() {
    const row = await request<Job>(
      `/api/clients/${clientId}/clinical-drafts`,
      "POST",
      {
        template_id: template,
        episode_id: episode || null,
        session_id: session || null,
        title,
        purpose,
        source_ids: sources.map((s) => s.id),
      },
    );
    await refresh();
    await open(row.id);
    setNotice(
      "Draft requested. You can keep working while the local model prepares the proposal.",
    );
  }
  async function review(decision: "accept" | "reject") {
    if (!selected) return;
    const r = await request<{ review: Job["review"]; note: Row | null }>(
      `/api/clinical-drafts/${selected.id}/review`,
      "POST",
      {
        decision,
        reason,
        attested,
        decisions: decision === "accept" ? Object.values(decisions) : [],
      },
    );
    await refresh();
    await open(selected.id);
    if (r.note) {
      await onNoteCreated(r.note);
      setNotice(
        "Your reviewed statements were saved as an unsigned clinical note. Open Documentation to edit and sign it after the encounter is complete.",
      );
    } else
      setNotice(
        "Proposal rejected. The original model output and your reason remain in history.",
      );
  }
  const fields = selected?.output?.sections ?? [];
  const total = fields.reduce((n, s) => n + s.statements.length, 0);
  const decided = Object.keys(decisions).length;
  const accepted = Object.values(decisions).filter(
    (d) => d.decision === "accept",
  ).length;
  const exclusionsComplete = Object.values(decisions).every(
    (d) => d.decision !== "exclude" || d.reason.trim().length >= 10,
  );
  const update = (
    section_key: string,
    index: number,
    decision: string,
    explanation = "",
  ) => {
    setDecisions((old) => ({
      ...old,
      [key(section_key, index)]: {
        section_key,
        index,
        decision,
        reason: explanation,
      },
    }));
    setAttested(false);
  };
  return (
    <section
      className="clinical-drafting"
      aria-label="Source-linked clinical drafting"
    >
      <header>
        <div>
          <span className="eyebrow">FROM EVIDENCE TO DOCUMENTATION</span>
          <h3>Prepare a clinical draft</h3>
          <p>
            Choose evidence above, then ask the local model to organize it
            against a form. Every statement needs your review.
          </p>
        </div>
        <span className="draft-engine">
          {configured ? "Local model configured" : "Local model unavailable"}
        </span>
      </header>
      {error && (
        <p role="alert" className="draft-alert">
          {error}
        </p>
      )}
      {notice && (
        <p role="status" className="draft-notice">
          {notice}
        </p>
      )}
      <details className="draft-request" open={!jobs.length}>
        <summary>
          New draft · {sources.length} selected evidence excerpts
        </summary>
        {!configured && (
          <p>
            Local drafting is not configured for this workspace. Approved
            records remain searchable.
          </p>
        )}
        {sources.length ? (
          <ul className="draft-selected">
            {sources.map((s) => (
              <li key={s.id}>
                <strong>{s.title}</strong>
                <span>
                  {label(s.source_kind)} · {date(s.source_date, true)}
                </span>
                <p>{s.text}</p>
              </li>
            ))}
          </ul>
        ) : (
          <p>
            Select evidence from the knowledge results to begin. Only approved
            sources can be used.
          </p>
        )}
        <div className="draft-fields">
          <label>
            Note title
            <input
              maxLength={200}
              value={title}
              onChange={(e) => setTitle(e.target.value)}
            />
          </label>
          <label>
            Clinical form
            <select
              value={template}
              onChange={(e) => setTemplate(e.target.value)}
            >
              <option value="">Choose a form</option>
              {data.templates
                .filter(
                  (t) =>
                    t.active !== false &&
                    Array.isArray(t.fields) &&
                    t.fields.length,
                )
                .map((t) => (
                  <option key={t.id} value={t.id}>
                    {value(t, "name")}
                  </option>
                ))}
            </select>
          </label>
          <label>
            Target episode
            <select
              value={episode}
              onChange={(e) => {
                setEpisode(e.target.value);
                setSession("");
              }}
            >
              <option value="">General chart context</option>
              {data.episodes
                .filter((e) => e.client_id === clientId)
                .map((e) => (
                  <option key={e.id} value={e.id}>
                    {label(value(e, "level_of_care"))} · {date(e.start_date)} ·{" "}
                    {label(value(e, "status"))}
                  </option>
                ))}
            </select>
          </label>
          <label>
            Linked session
            <select
              value={session}
              onChange={(e) => setSession(e.target.value)}
            >
              <option value="">General chart note</option>
              {data.sessions
                .filter(
                  (s) =>
                    s.client_id === clientId &&
                    (!episode || s.episode_id === episode),
                )
                .map((s) => (
                  <option key={s.id} value={s.id}>
                    {label(value(s, "kind"))} · {date(s.starts_at, true)}
                  </option>
                ))}
            </select>
          </label>
        </div>
        <label>
          Documentation purpose
          <textarea
            rows={3}
            minLength={10}
            maxLength={2000}
            value={purpose}
            onChange={(e) => setPurpose(e.target.value)}
            placeholder="Describe the encounter and the documentation you need, using only selected evidence."
          />
        </label>
        <Button
          disabled={
            busy ||
            dirty ||
            active ||
            !configured ||
            !template ||
            !title.trim() ||
            purpose.trim().length < 10 ||
            !sources.length ||
            sources.length > 12
          }
          onClick={() => void run(create)}
        >
          Request local draft
        </Button>
        <small>
          Up to 12 source excerpts. Recognition and model output can be wrong;
          source linkage alone does not establish clinical accuracy.
        </small>
      </details>
      <div className="draft-history">
        <h4>Generation history</h4>
        <Button
          variant="secondary small"
          disabled={busy}
          onClick={() => void run(refresh)}
        >
          Refresh drafts
        </Button>
      </div>
      {!jobs.length && (
        <p className="draft-empty">
          No model drafts have been requested for this patient.
        </p>
      )}
      <div className="draft-job-list">
        {jobs.map((j) => (
          <button
            key={j.id}
            className={selected?.id === j.id ? "selected" : ""}
            disabled={busy || dirty}
            onClick={() => void run(() => open(j.id))}
          >
            <strong>{j.title}</strong>
            <span>{date(j.created_at, true)}</span>
            <b>
              {j.review
                ? j.review.decision === "accept"
                  ? "Accepted"
                  : "Rejected"
                : label(j.state)}
            </b>
          </button>
        ))}
      </div>
      {selected && (
        <section className="draft-proposal" aria-label="Model proposal review">
          <header>
            <div>
              <h4>{selected.title}</h4>
              <p>
                {date(selected.created_at, true)} · {label(selected.state)}
              </p>
            </div>
            <Button
              variant="secondary small"
              disabled={busy || dirty}
              onClick={() => void run(() => open(selected.id))}
            >
              Reload proposal
            </Button>
          </header>
          {["queued", "running"].includes(selected.state) && (
            <>
              <p>
                {selected.state === "queued"
                  ? "Waiting for the local drafting worker."
                  : "The local model is preparing the proposal. Progress is reported by job state."}
              </p>
              <Button
                variant="secondary"
                disabled={busy}
                onClick={() =>
                  void run(async () => {
                    await request(
                      `/api/clinical-drafts/${selected.id}/cancel`,
                      "POST",
                      {},
                    );
                    await refresh();
                    await open(selected.id);
                  })
                }
              >
                Cancel generation
              </Button>
            </>
          )}
          {["failed", "cancelled"].includes(selected.state) && (
            <p role="status">
              {selected.state === "failed"
                ? `Generation failed (${selected.error_code}).`
                : "Generation was cancelled."}{" "}
              Review current evidence and use New draft to request a fresh
              attempt. No clinical note was created.
            </p>
          )}
          {selected.output && (
            <>
              <details>
                <summary>Model and source provenance</summary>
                <p>
                  {selected.output.model} · prompt{" "}
                  {selected.output.prompt_version}
                </p>
                <code>{selected.output.model_sha256}</code>
                <ul>
                  {selected.sources?.map((s) => (
                    <li key={s.id}>
                      {s.title} · {date(s.source_date, true)}
                    </li>
                  ))}
                </ul>
              </details>
              {!!selected.output.unresolved.length && (
                <aside className="draft-unresolved">
                  <h5>Unresolved information</h5>
                  <ul>
                    {selected.output.unresolved.map((u, i) => (
                      <li key={i}>{u}</li>
                    ))}
                  </ul>
                  <p>
                    Missing information is not evidence that a symptom or risk
                    is absent.
                  </p>
                </aside>
              )}
              {fields.map((s) => (
                <section key={s.key} className="draft-section">
                  <h5>{s.label}</h5>
                  {!s.statements.length && (
                    <p>No supported statements proposed for this section.</p>
                  )}
                  {s.statements.map((st, i) => {
                    const decision = decisions[key(s.key, i)];
                    const recorded = selected.review?.decisions?.find(
                      (d) => d.section_key === s.key && d.index === i,
                    );
                    return (
                      <article key={i} className="draft-statement">
                        <p>{st.text}</p>
                        <details open>
                          <summary>Supporting evidence</summary>
                          {st.citations.map((c, k) => (
                            <blockquote key={k}>
                              <p>{c.quote}</p>
                              <cite>
                                {selected.sources?.find(
                                  (x) => x.id === c.source_id,
                                )?.title ?? "Saved source"}{" "}
                                ·{" "}
                                {date(
                                  selected.sources?.find(
                                    (x) => x.id === c.source_id,
                                  )?.source_date,
                                  true,
                                )}
                              </cite>
                            </blockquote>
                          ))}
                        </details>
                        {selected.review && recorded && (
                          <p className="draft-notice">
                            {recorded.decision === "accept"
                              ? "Accepted into the draft note"
                              : "Excluded from the draft note"}
                            {recorded.reason ? ` · ${recorded.reason}` : ""}
                          </p>
                        )}
                        {!selected.review && (
                          <fieldset disabled={busy}>
                            <legend>
                              Statement {i + 1} · {s.label}
                            </legend>
                            <label>
                              <input
                                type="radio"
                                name={`review-${s.key}-${i}`}
                                checked={decision?.decision === "accept"}
                                onChange={() => update(s.key, i, "accept")}
                              />
                              Accept after checking evidence and meaning
                            </label>
                            <label>
                              <input
                                type="radio"
                                name={`review-${s.key}-${i}`}
                                checked={decision?.decision === "exclude"}
                                onChange={() => update(s.key, i, "exclude")}
                              />
                              Exclude from the note
                            </label>
                            {decision?.decision === "exclude" && (
                              <label>
                                Reason for exclusion
                                <textarea
                                  value={decision.reason}
                                  maxLength={2000}
                                  onChange={(e) =>
                                    update(s.key, i, "exclude", e.target.value)
                                  }
                                />
                              </label>
                            )}
                          </fieldset>
                        )}
                      </article>
                    );
                  })}
                </section>
              ))}
              {selected.review ? (
                <aside className="draft-notice">
                  <strong>
                    {selected.review.decision === "accept"
                      ? "Review recorded · unsigned note created"
                      : "Proposal rejected"}
                  </strong>
                  <p>{selected.review.reason}</p>
                </aside>
              ) : (
                <div className="draft-review-actions">
                  <p>
                    {decided} of {total} statements reviewed · {accepted}{" "}
                    accepted
                  </p>
                  {dirty && (
                    <Button
                      variant="secondary small"
                      disabled={busy}
                      onClick={() => {
                        setDecisions({});
                        setReason("");
                        setAttested(false);
                      }}
                    >
                      Discard review choices
                    </Button>
                  )}
                  <label>
                    Review reason
                    <textarea
                      rows={3}
                      minLength={10}
                      maxLength={2000}
                      value={reason}
                      onChange={(e) => {
                        setReason(e.target.value);
                        setAttested(false);
                      }}
                    />
                  </label>
                  <label className="draft-attestation">
                    <input
                      type="checkbox"
                      checked={attested}
                      onChange={(e) => setAttested(e.target.checked)}
                    />
                    I checked the statements, dates, attribution, supporting
                    excerpts and unresolved information. This review does not
                    sign the clinical note.
                  </label>
                  <div>
                    <Button
                      disabled={
                        busy ||
                        !attested ||
                        reason.trim().length < 10 ||
                        decided !== total ||
                        !accepted ||
                        !exclusionsComplete
                      }
                      onClick={() => void run(() => review("accept"))}
                    >
                      Record review and create draft note
                    </Button>
                    <Button
                      variant="secondary"
                      disabled={busy || !attested || reason.trim().length < 10}
                      onClick={() => void run(() => review("reject"))}
                    >
                      Reject proposal
                    </Button>
                  </div>
                </div>
              )}
            </>
          )}
        </section>
      )}
    </section>
  );
}
