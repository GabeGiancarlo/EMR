import { useState } from "react";
import type { FormEvent } from "react";
import { date, label, value } from "./api";
import type { Bootstrap, Resource, Row } from "./api";
import type { Field } from "./forms";
import { Badge, Button, Empty, Icon, Modal, Panel } from "./ui";

export type WorkflowProps = {
  data: Bootstrap;
  canWrite: boolean;
  create: (r: Resource, initial?: Record<string, unknown>) => void;
  edit: (r: Resource, row: Row) => void;
  action: (
    path: string,
    title: string,
    description: string,
    fields?: Field[],
    initial?: Record<string, unknown>,
    submitLabel?: string,
  ) => void;
  mutate: (path: string, body: unknown, method?: string) => Promise<void>;
};

function PlanReview({
  plan,
  goals,
  onSave,
  onClose,
}: {
  plan: Row;
  goals: Row[];
  onSave: (body: unknown) => Promise<void>;
  onClose: () => void;
}) {
  const [summary, setSummary] = useState("");
  const [nextDate, setNextDate] = useState("");
  const [progress, setProgress] = useState<
    Record<string, { status: string; observation: string }>
  >(() =>
    Object.fromEntries(
      goals.map((g) => [g.id, { status: "on_track", observation: "" }]),
    ),
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      await onSave({
        plan_id: plan.id,
        summary,
        next_review_date: nextDate || null,
        goal_progress: goals.map((g) => ({ goal_id: g.id, ...progress[g.id] })),
      });
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Unable to record this review.",
      );
    } finally {
      setBusy(false);
    }
  }
  return (
    <Modal
      title="Review treatment progress"
      description="Record the client’s progress against the signed goals. The original plan remains unchanged."
      onClose={onClose}
      wide
    >
      <form onSubmit={submit}>
        <div className="form-body">
          <div className="form-field">
            <label htmlFor="review-summary">Review summary *</label>
            <textarea
              id="review-summary"
              required
              rows={4}
              value={summary}
              onChange={(e) => setSummary(e.target.value)}
            />
          </div>
          <div className="review-goals">
            {goals.map((goal, i) => (
              <section className="review-goal" key={goal.id}>
                <span className="eyebrow">GOAL {i + 1}</span>
                <h3>{value(goal, "description")}</h3>
                <p>
                  {value(goal, "measure")} · Target: {value(goal, "target")}
                </p>
                <div className="form-grid">
                  <div className="form-field">
                    <label htmlFor={`progress-${goal.id}`}>
                      Current progress
                    </label>
                    <select
                      id={`progress-${goal.id}`}
                      value={progress[goal.id].status}
                      onChange={(e) =>
                        setProgress({
                          ...progress,
                          [goal.id]: {
                            ...progress[goal.id],
                            status: e.target.value,
                          },
                        })
                      }
                    >
                      {["on_track", "at_risk", "achieved", "discontinued"].map(
                        (s) => (
                          <option key={s} value={s}>
                            {label(s)}
                          </option>
                        ),
                      )}
                    </select>
                  </div>
                  <div className="form-field full">
                    <label htmlFor={`observation-${goal.id}`}>
                      Observation *
                    </label>
                    <textarea
                      id={`observation-${goal.id}`}
                      required
                      rows={3}
                      value={progress[goal.id].observation}
                      onChange={(e) =>
                        setProgress({
                          ...progress,
                          [goal.id]: {
                            ...progress[goal.id],
                            observation: e.target.value,
                          },
                        })
                      }
                    />
                  </div>
                </div>
              </section>
            ))}
          </div>
          <div className="form-field">
            <label htmlFor="review-next">Next review date</label>
            <input
              id="review-next"
              type="date"
              value={nextDate}
              onInput={(e) => setNextDate(e.currentTarget.value)}
              onChange={(e) => setNextDate(e.target.value)}
            />
          </div>
          {error && (
            <div className="error-banner" role="alert">
              {error}
            </div>
          )}
        </div>
        <div className="modal-foot">
          <span className="form-assurance">
            This review is added to the plan’s permanent history.
          </span>
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button type="submit" disabled={busy}>
            {busy ? "Saving…" : "Record review"}
            <Icon name="check" size={17} />
          </Button>
        </div>
      </form>
    </Modal>
  );
}

export default function CarePlanning({
  clientId,
  data,
  canWrite,
  create,
  edit,
  action,
  mutate,
}: WorkflowProps & { clientId: string }) {
  const plans = data.treatment_plans.filter((p) => p.client_id === clientId);
  const [selectedId, setSelectedId] = useState("");
  const [review, setReview] = useState(false);
  const selected =
    plans.find((p) => p.id === selectedId) ||
    plans.find((p) => p.status === "active") ||
    plans[0];
  const goals = selected
    ? data.goals.filter((g) => g.plan_id === selected.id)
    : [];
  const reviews = selected
    ? data.plan_reviews
        .filter((r) => r.plan_id === selected.id)
        .sort((a, b) =>
          value(b, "created_at").localeCompare(value(a, "created_at")),
        )
    : [];
  const latest = reviews[0];
  const goalProgress = (
    Array.isArray(latest?.goal_progress) ? latest.goal_progress : []
  ) as { goal_id: string; status: string; observation: string }[];
  const addPlan = () =>
    create("treatment_plans", {
      client_id: clientId,
      episode_id: data.episodes.find(
        (e) => e.client_id === clientId && e.status === "active",
      )?.id,
    });
  if (!plans.length)
    return (
      <Panel
        title="Treatment planning"
        action={
          canWrite ? (
            <Button variant="secondary small" onClick={addPlan}>
              <Icon name="plus" size={16} /> New plan
            </Button>
          ) : undefined
        }
      >
        <Empty
          icon="leaf"
          title="Turn priorities into a shared plan"
          description="Start with the client’s strengths and needs. Add measurable goals, sign the plan, and capture progress at each review."
          action={
            canWrite ? (
              <Button variant="secondary" onClick={addPlan}>
                Create treatment plan
              </Button>
            ) : undefined
          }
        />
      </Panel>
    );
  return (
    <>
      <div className="care-planning">
        <aside className="plan-list">
          <div className="plan-list-head">
            <h3>Treatment plans</h3>
            {canWrite && (
              <button
                className="icon-button"
                aria-label="Create treatment plan"
                onClick={addPlan}
              >
                <Icon name="plus" size={17} />
              </button>
            )}
          </div>
          {plans.map((p) => (
            <button
              className={`plan-list-item ${p.id === selected?.id ? "selected" : ""}`}
              key={p.id}
              onClick={() => setSelectedId(p.id)}
            >
              <strong>{value(p, "title")}</strong>
              <Badge status={p.status} />
              <small>Created {date(p.created_at)}</small>
            </button>
          ))}
        </aside>
        {selected && (
          <div className="plan-detail">
            <Panel
              title={value(selected, "title")}
              subtitle={`Next review: ${selected.next_review_date ? date(selected.next_review_date) : "Not scheduled"}`}
              action={<Badge status={selected.status} />}
            >
              <div className="plan-summary">
                <div>
                  <span className="eyebrow">PRESENTING NEEDS</span>
                  <p>{value(selected, "presenting_needs")}</p>
                </div>
                <div>
                  <span className="eyebrow">STRENGTHS & RESOURCES</span>
                  <p>{value(selected, "strengths")}</p>
                </div>
              </div>
              {Boolean(selected.supersedes_plan_id) && (
                <div className="quiet-note">
                  <Icon name="notes" size={17} /> Revision of plan{" "}
                  {value(selected, "supersedes_plan_id").slice(0, 8)}. The
                  earlier record remains in history.
                </div>
              )}
              {canWrite && (
                <div className="plan-actions">
                  {selected.status === "draft" ? (
                    <>
                      <Button
                        variant="secondary small"
                        onClick={() => edit("treatment_plans", selected)}
                      >
                        Edit plan
                      </Button>
                      <Button
                        variant="small"
                        onClick={() =>
                          action(
                            `/api/treatment_plans/${selected.id}/activate`,
                            "Sign and activate treatment plan",
                            "Confirm that you have reviewed the client’s plan and its measurable goals. Signing preserves the plan and goal definitions. A future revision creates a separate draft.",
                            [],
                            {},
                            "Sign and activate",
                          )
                        }
                        disabled={!goals.length}
                      >
                        <Icon name="check" size={16} /> Sign & activate
                      </Button>
                    </>
                  ) : selected.status === "active" ? (
                    <>
                      <Button
                        variant="secondary small"
                        onClick={() =>
                          action(
                            `/api/treatment_plans/${selected.id}/revise`,
                            "Revise treatment plan",
                            "Copy the signed plan and its goals into a new draft. The current plan stays active until the revision is signed.",
                            [
                              {
                                key: "reason",
                                label: "Reason for revision",
                                type: "textarea",
                                required: true,
                              },
                            ],
                            {},
                            "Create revision",
                          )
                        }
                      >
                        Create revision
                      </Button>
                      <Button
                        variant="secondary small"
                        onClick={() =>
                          action(
                            `/api/treatment_plans/${selected.id}/complete`,
                            "Complete treatment plan",
                            "Document why this plan is complete. Its original content and review history will be retained.",
                            [
                              {
                                key: "reason",
                                label: "Completion reason",
                                type: "textarea",
                                required: true,
                              },
                            ],
                            {},
                            "Complete plan",
                          )
                        }
                      >
                        Complete plan
                      </Button>
                      <Button variant="small" onClick={() => setReview(true)}>
                        Record review <Icon name="arrow" size={16} />
                      </Button>
                    </>
                  ) : (
                    <span className="muted">
                      This plan is preserved as a historical record.
                    </span>
                  )}
                </div>
              )}
            </Panel>
            <Panel
              title="Goals that make progress visible"
              subtitle="Signed goal definitions stay intact; outcomes are recorded in reviews."
              action={
                canWrite && selected.status === "draft" ? (
                  <Button
                    variant="text small"
                    onClick={() => create("goals", { plan_id: selected.id })}
                  >
                    <Icon name="plus" size={16} /> Add goal
                  </Button>
                ) : undefined
              }
            >
              {goals.length ? (
                <div className="goal-list">
                  {goals.map((goal, i) => {
                    const result = goalProgress.find(
                      (g) => g.goal_id === goal.id,
                    );
                    return (
                      <article className="goal-card" key={goal.id}>
                        <div className="goal-card-title">
                          <span className="goal-index">
                            {String(i + 1).padStart(2, "0")}
                          </span>
                          <h3>{value(goal, "description")}</h3>
                          {result && <Badge status={result.status} />}
                        </div>
                        <div className="goal-grid">
                          <div>
                            <span>Measure</span>
                            <p>{value(goal, "measure")}</p>
                          </div>
                          <div>
                            <span>Starting point</span>
                            <p>{value(goal, "baseline")}</p>
                          </div>
                          <div>
                            <span>Target</span>
                            <p>{value(goal, "target")}</p>
                          </div>
                          <div>
                            <span>Target date</span>
                            <p>
                              {goal.target_date
                                ? date(goal.target_date)
                                : "Not set"}
                            </p>
                          </div>
                        </div>
                        {result && (
                          <div className="goal-observation">
                            <strong>Latest observation</strong>
                            <p>{result.observation}</p>
                          </div>
                        )}
                        {canWrite && selected.status === "draft" && (
                          <Button
                            variant="text small"
                            onClick={() => edit("goals", goal)}
                          >
                            Edit goal <Icon name="arrow" size={15} />
                          </Button>
                        )}
                      </article>
                    );
                  })}
                </div>
              ) : (
                <Empty
                  icon="leaf"
                  title="Make the next step measurable"
                  description="Add at least one goal, including a starting point, measure, and target, before signing the plan."
                />
              )}
            </Panel>
            <Panel
              title="Progress reviews"
              subtitle="Each review adds to the story without replacing what came before."
            >
              {reviews.length ? (
                <div className="plan-reviews">
                  {reviews.map((r) => (
                    <article key={r.id}>
                      <div className="review-date">
                        <Icon name="activity" size={17} />
                        <strong>{date(r.created_at, true)}</strong>
                      </div>
                      <p>{value(r, "summary")}</p>
                      <small>
                        Next review:{" "}
                        {r.next_review_date
                          ? date(r.next_review_date)
                          : "Not specified"}
                      </small>
                      {Array.isArray(r.goal_progress) && (
                        <div className="review-progress">
                          {(
                            r.goal_progress as {
                              goal_id: string;
                              status: string;
                              observation: string;
                            }[]
                          ).map((g) => (
                            <div key={g.goal_id}>
                              <Badge status={g.status} />
                              <span>{g.observation}</span>
                            </div>
                          ))}
                        </div>
                      )}
                    </article>
                  ))}
                </div>
              ) : (
                <Empty
                  icon="activity"
                  title="Progress has a place here"
                  description="Once the plan is active, record a review of each goal and the next review date."
                />
              )}
            </Panel>
          </div>
        )}
      </div>
      {review && selected && (
        <PlanReview
          plan={selected}
          goals={goals}
          onClose={() => setReview(false)}
          onSave={async (body) => {
            await mutate("/api/plan_reviews", body);
            setReview(false);
          }}
        />
      )}
    </>
  );
}
