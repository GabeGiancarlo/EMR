import { useState } from "react";
import type { Bootstrap, Resource, Row } from "./api";
import { label } from "./api";
import {
  AUTH_WINDOW_DAYS,
  LEDGER_DEFINITIONS,
  LEVEL_COLORS,
  SERVICE_COLORS,
  UNAVAILABLE_EXECUTIVE_METRICS,
  actorFacilityZone,
  allowedWorkspaceModes,
  calendarDateInZone,
  claimBalance,
  currentCensus,
  currentStaffing,
  defaultFlowRange,
  defaultWorkspaceMode,
  expiringAuthorizations,
  flowLedger,
  formatCents,
  noteActionsAllowed,
  operationalWorkload,
  todaysGroupCount,
  todaysSessions,
  upcomingPlanReviews,
  visibleRows,
  writeAllowed,
} from "./workspace";
import type { MoneyState, WorkspaceMode } from "./workspace";
import "./workspace.css";

export type WorkspaceView =
  | "overview"
  | "clients"
  | "schedule"
  | "notes"
  | "billing"
  | "tasks"
  | "team"
  | "settings";

export type RoleWorkspaceProps = {
  data: Bootstrap;
  mode: WorkspaceMode;
  onNavigate: (view: WorkspaceView, filter?: string) => void;
  onOpenClient: (row: Row) => void;
  onCreate: (resource: Resource, initial?: Record<string, unknown>) => void;
  onOpenClaim: (row: Row) => void;
};

function text(row: Row | undefined, key: string): string {
  const value = row?.[key];
  return value == null ? "" : String(value);
}

function patientName(row: Row): string {
  return `${text(row, "first_name")} ${text(row, "last_name")}`.trim() || "Unnamed patient";
}

function MoneyFigure({ figure }: { figure: MoneyState }) {
  if (figure.state === "value") {
    return (
      <>
        <strong>{formatCents(figure.cents)}</strong>
      </>
    );
  }
  return (
    <>
      <strong>{figure.state === "unavailable" ? "Not available" : "Not calculated"}</strong>
      <small className="ws-muted">{figure.reason}</small>
    </>
  );
}

function PatientLink({
  data,
  id,
  onOpenClient,
}: {
  data: Bootstrap;
  id: unknown;
  onOpenClient: (row: Row) => void;
}) {
  const clients = visibleRows(data, "clients");
  if (!clients) return <span className="ws-muted">Patient record not included</span>;
  const row = clients.find((client) => client.id === id);
  if (!row) return <span className="ws-muted">Patient unavailable</span>;
  return (
    <button type="button" className="ws-link" onClick={() => onOpenClient(row)}>
      {patientName(row)}
    </button>
  );
}

function Gated({
  available,
  empty,
  children,
  unavailable,
  emptyTitle = "Nothing here yet",
  emptyDescription = "Records will appear here as your team works.",
}: {
  available: boolean;
  empty: boolean;
  unavailable: string;
  children: React.ReactNode;
  emptyTitle?: string;
  emptyDescription?: string;
}) {
  if (!available) {
    return (
      <div className="ws-unavailable">
        <strong>Not included</strong>
        <p>{unavailable}</p>
      </div>
    );
  }
  if (empty) {
    return (
      <div className="ws-panel">
        <strong>{emptyTitle}</strong>
        <p className="ws-muted">{emptyDescription}</p>
      </div>
    );
  }
  return <>{children}</>;
}

function ClinicalHome(props: RoleWorkspaceProps & { now: Date; today: string | null }) {
  const { data, onNavigate, onOpenClient, onCreate, now, today } = props;
  const schedule = todaysSessions(data, now);
  const groups = todaysGroupCount(data, now);
  const notes = visibleRows(data, "notes");
  const drafts = notes?.filter((note) => note.status === "draft") ?? [];
  const reviews = upcomingPlanReviews(data, today);
  const tasks = visibleRows(data, "tasks");
  const openTasks = tasks?.filter((task) => task.status !== "done") ?? [];
  const zone = actorFacilityZone(data);
  return (
    <div className="ws-clinical">
      <section className="ws-schedule" aria-labelledby="ws-today-heading">
        <span className="ws-kicker">Facility calendar</span>
        <div className="ws-split">
          <h2 id="ws-today-heading">Today’s schedule</h2>
          <button type="button" className="ws-action quiet" onClick={() => onNavigate("schedule", "today")}>
            Open calendar
          </button>
        </div>
        <p>Today’s care, shown in each facility’s local time.</p>
        <Gated
          available={schedule.available}
          empty={schedule.rows.length === 0 && schedule.undated.length === 0}
          emptyTitle="Your schedule is clear today"
          emptyDescription="Schedule care or open the calendar to review another date."
          unavailable="Sessions are not included for this access. No schedule was invented."
        >
          {schedule.rows.map((item) => (
            <article className="ws-session" key={item.row.id}>
              <span
                className="ws-accent-bar"
                style={{ background: SERVICE_COLORS[text(item.row, "kind")] || "var(--ws-accent)" }}
              />
              <div>
                <PatientLink data={data} id={item.row.client_id} onOpenClient={onOpenClient} />
                <small className="ws-muted">
                  {label(text(item.row, "kind") || "session")} · {item.localTime || "Time unavailable"} · {item.timeZone}
                </small>
              </div>
              <strong>{label(text(item.row, "status") || "not set")}</strong>
            </article>
          ))}
          {schedule.undated.length > 0 && (
            <p>{schedule.undated.length} loaded session{schedule.undated.length === 1 ? "" : "s"} could not be placed on a facility calendar and {schedule.undated.length === 1 ? "is" : "are"} not counted as away.</p>
          )}
        </Gated>
        {groups === null ? (
          <p className="ws-muted">Group encounters are not included for this access.</p>
        ) : (
          <p>{groups === 0 ? "No group encounters today." : `${groups} group encounter${groups === 1 ? "" : "s"} today.`}</p>
        )}
        <div className="ws-actions" aria-label="Clinical quick actions">
          {noteActionsAllowed(data) && (
            <button type="button" className="ws-action" onClick={() => onCreate("notes")}>
              New note
            </button>
          )}
          {writeAllowed(data, "sessions") && (
            <button type="button" className="ws-action quiet" onClick={() => onCreate("sessions")}>
              Schedule session
            </button>
          )}
          {writeAllowed(data, "tasks") && (
            <button type="button" className="ws-action quiet" onClick={() => onCreate("tasks")}>
              New task
            </button>
          )}
          {writeAllowed(data, "clients") && (
            <button type="button" className="ws-action quiet" onClick={() => onCreate("clients")}>
              New client
            </button>
          )}
        </div>
      </section>
      <aside className="ws-rail">
        <section aria-labelledby="ws-notes-heading">
          <div className="ws-split">
            <h2 id="ws-notes-heading">Unsigned notes</h2>
            {notes && (
              <button type="button" className="ws-link" onClick={() => onNavigate("notes", "draft")}>
                {drafts.length} draft{drafts.length === 1 ? "" : "s"}
              </button>
            )}
          </div>
          <Gated
            available={notes !== null}
            empty={drafts.length === 0}
            emptyTitle="No notes awaiting signature"
            emptyDescription="New drafts will appear here for review."
            unavailable="Clinical notes are not included for this role. The count is not shown as zero."
          >
            {drafts.slice(0, 6).map((note) => (
              <article className="ws-row" key={note.id}>
                <button className="ws-link" type="button" onClick={() => onNavigate("notes", `record:${note.id}`)}><strong>{text(note, "title") || "Untitled note"}</strong></button>
                <PatientLink data={data} id={note.client_id} onOpenClient={onOpenClient} />
              </article>
            ))}
            {drafts.length > 6 && <p>Showing 6 of {drafts.length} loaded drafts.</p>}
          </Gated>
        </section>
        <section aria-labelledby="ws-review-heading">
          <h2 id="ws-review-heading">Next plan reviews</h2>
          <Gated
            available={reviews.available}
            empty={reviews.dated.length === 0 && reviews.undated === 0}
            emptyTitle="No active plan reviews due"
            emptyDescription="Review dates from active treatment plans appear here."
            unavailable="Treatment plans are not included for this access."
          >
            {reviews.dated.slice(0, 6).map((plan) => {
              const due = text(plan, "next_review_date");
              const overdue = Boolean(today && due < today);
              return (
                <article className="ws-row" key={plan.id}>
                  <strong>{text(plan, "title") || "Treatment plan"}</strong>
                  <span>{overdue ? "Overdue" : "Scheduled"} {due}</span>
                  <PatientLink data={data} id={plan.client_id} onOpenClient={onOpenClient} />
                </article>
              );
            })}
            {reviews.undated > 0 && <p>{reviews.undated} loaded plan{reviews.undated === 1 ? "" : "s"} have no review date and are not treated as due today.</p>}
          </Gated>
        </section>
        <section aria-labelledby="ws-task-heading">
          <div className="ws-split">
            <h2 id="ws-task-heading">Task queue</h2>
            {tasks && (
              <button type="button" className="ws-link" onClick={() => onNavigate("tasks", "open")}>
                Open tasks
              </button>
            )}
          </div>
          <Gated
            available={tasks !== null}
            empty={openTasks.length === 0}
            unavailable="Tasks are not included for this access."
          >
            {openTasks.slice(0, 6).map((task) => (
              <button
                type="button"
                className="ws-row"
                key={task.id}
                onClick={() => onNavigate("tasks", `record:${task.id}`)}
              >
                <strong>{text(task, "title") || "Untitled task"}</strong>
                <small className="ws-muted">
                  {label(text(task, "priority") || "normal")} · {text(task, "due_date") || "No due date"}
                </small>
              </button>
            ))}
          </Gated>
        </section>
      </aside>
    </div>
  );
}

function FlowControls({
  from,
  to,
  onChange,
}: {
  from: string;
  to: string;
  onChange: (next: { from: string; to: string }) => void;
}) {
  return (
    <div className="ws-flow-controls">
      <label>
        From
        <input
          type="date"
          value={from}
          onChange={(event) => onChange({ from: event.target.value, to })}
        />
      </label>
      <label>
        Through
        <input
          type="date"
          value={to}
          onChange={(event) => onChange({ from, to: event.target.value })}
        />
      </label>
      <p>These dates change charges, cash, and adjustments only. Outstanding stays current.</p>
    </div>
  );
}

function BillingHome(props: RoleWorkspaceProps & { now: Date; today: string | null }) {
  const { data, onNavigate, onOpenClient, onCreate, onOpenClaim, today } = props;
  const zone = actorFacilityZone(data);
  const defaults = today ? defaultFlowRange(today) : null;
  const [range, setRange] = useState<{ actor: string; from: string; to: string } | null>(null);
  const actor = String(data.actor?.id ?? "");
  const activeRange =
    range && range.actor === actor
      ? range
      : { actor, from: defaults?.from ?? "", to: defaults?.to ?? "" };
  const ledger = flowLedger(data, activeRange.from, activeRange.to);
  const claims = visibleRows(data, "claims");
  const auths = expiringAuthorizations(data, today);
  const statuses = ["draft", "ready", "submitted", "denied", "paid", "voided"];
  const denied = claims?.filter((claim) => claim.status === "denied") ?? [];
  const balances = claims?.filter((claim) =>
    ["submitted", "denied", "partially_paid"].includes(text(claim, "status")),
  ) ?? [];
  return (
    <div className="ws-billing">
      <section className="ws-ledger" aria-labelledby="ws-ledger-heading">
        <div>
          <span className="ws-kicker">Billing activity · {zone}</span>
          <h2 id="ws-ledger-heading">Charges, cash, and adjustments</h2>
          <FlowControls
            from={activeRange.from}
            to={activeRange.to}
            onChange={(next) => setRange({ actor, ...next })}
          />
          <div className="ws-tape">
            <article>
              <div>
                <h3>Charges</h3>
                <details><summary>Definition</summary><p>{LEDGER_DEFINITIONS.charges}</p></details>
              </div>
              <MoneyFigure figure={ledger.charges} />
            </article>
            <article>
              <div>
                <h3>Cash</h3>
                <details><summary>Definition</summary><p>{LEDGER_DEFINITIONS.cash}</p></details>
              </div>
              <MoneyFigure figure={ledger.cash} />
            </article>
            <article>
              <div>
                <h3>Adjustments</h3>
                <details><summary>Definition</summary><p>{LEDGER_DEFINITIONS.adjustments}</p></details>
              </div>
              <MoneyFigure figure={ledger.adjustments} />
            </article>
          </div>
        </div>
        <aside className="ws-stock" aria-labelledby="ws-outstanding-heading">
          <span className="ws-current">Current</span>
          <h2 id="ws-outstanding-heading">Outstanding</h2>
          <details><summary>Definition</summary><p>{LEDGER_DEFINITIONS.outstanding}</p></details>
          <MoneyFigure figure={ledger.outstanding} />
          <button type="button" className="ws-action" onClick={() => onNavigate("billing", "balance")}>
            Open current balances
          </button>
        </aside>
      </section>
      <section className="ws-panel" aria-labelledby="ws-status-heading">
        <h2 id="ws-status-heading">Claim status</h2>
        <Gated
          available={claims !== null}
          empty={(claims?.length ?? 0) === 0}
          unavailable="Claims are not included for this access. Status counts are not zeroed."
        >
          <ul className="ws-bars">
            {statuses.map((status) => {
              const count = claims?.filter((claim) => claim.status === status).length ?? 0;
              const max = Math.max(...statuses.map((item) => claims?.filter((claim) => claim.status === item).length ?? 0), 1);
              return (
                <li key={status}>
                  <button type="button" onClick={() => onNavigate("billing", status)}>
                    <span>{label(status)}</span>
                    <span className="ws-track" aria-hidden="true">
                      <span className="ws-fill" style={{ width: `${(count / max) * 100}%`, background: status === "denied" ? "#b45c4e" : "var(--ws-accent)" }} />
                    </span>
                    <strong>{count}</strong>
                  </button>
                </li>
              );
            })}
          </ul>
        </Gated>
      </section>
      <div className="ws-billing-grid">
        <section className="ws-panel" aria-labelledby="ws-denial-heading">
          <div className="ws-split">
            <h2 id="ws-denial-heading">Denial queue</h2>
            <button type="button" className="ws-link" onClick={() => onNavigate("billing", "denied")}>
              All denied claims
            </button>
          </div>
          <Gated
            available={claims !== null}
            empty={denied.length === 0}
            unavailable="Denied claims are not included for this access."
          >
            {denied.slice(0, 6).map((claim) => (
              <article className="ws-row" key={claim.id}>
                <PatientLink data={data} id={claim.client_id} onOpenClient={onOpenClient} />
                <span>{text(claim, "service_code") || "Service code unavailable"}</span>
                <button type="button" className="ws-action quiet" onClick={() => onOpenClaim(claim)}>
                  Open claim
                </button>
              </article>
            ))}
          </Gated>
        </section>
        <section className="ws-panel" aria-labelledby="ws-auth-heading">
          <div className="ws-split">
            <h2 id="ws-auth-heading">Authorizations ending within {AUTH_WINDOW_DAYS} days</h2>
            <button type="button" className="ws-link" onClick={() => onNavigate("billing", "authorizations")}>
              Authorizations
            </button>
          </div>
          <p className="ws-current">Current snapshot, not filtered by the flow dates.</p>
          <Gated
            available={auths.available}
            empty={auths.rows.length === 0 && auths.undated === 0}
            unavailable="Authorizations are not included for this access."
          >
            {auths.rows.slice(0, 6).map((auth) => (
              <article className="ws-row" key={auth.id}>
                <strong>{text(auth, "authorization_number") || "Authorization"}</strong>
                <span>Through {text(auth, "ends_on")} · {text(auth, "service_code")}</span>
                <PatientLink data={data} id={auth.client_id} onOpenClient={onOpenClient} />
              </article>
            ))}
            {auths.undated > 0 && <p>{auths.undated} loaded authorization{auths.undated === 1 ? "" : "s"} have no end date and are not marked expiring.</p>}
          </Gated>
        </section>
      </div>
      <section className="ws-panel" aria-labelledby="ws-balance-heading">
        <h2 id="ws-balance-heading">Current balances</h2>
        <Gated
          available={claims !== null}
          empty={balances.length === 0}
          unavailable="Claim balances are not included for this access."
        >
          {balances.slice(0, 8).map((claim) => {
            const balance = claimBalance(data, claim);
            return (
              <article className="ws-row" key={claim.id}>
                <PatientLink data={data} id={claim.client_id} onOpenClient={onOpenClient} />
                <span>{label(text(claim, "status"))} · {text(claim, "service_code")}</span>
                <MoneyFigure figure={balance} />
                <button type="button" className="ws-link" onClick={() => onOpenClaim(claim)}>
                  Open claim record
                </button>
              </article>
            );
          })}
        </Gated>
        <div className="ws-actions">
          {writeAllowed(data, "claims") && (
            <button type="button" className="ws-action" onClick={() => onCreate("claims")}>
              New claim
            </button>
          )}
          {writeAllowed(data, "payments") && (
            <button type="button" className="ws-action quiet" onClick={() => onCreate("payments")}>
              Record payment
            </button>
          )}
          <button type="button" className="ws-action quiet" onClick={() => onNavigate("billing", "payments")}>
            Payment ledger
          </button>
        </div>
      </section>
    </div>
  );
}

function ExecutiveHome(props: RoleWorkspaceProps & { now: Date; today: string | null }) {
  const { data, onNavigate, today } = props;
  const census = currentCensus(data);
  const staffing = currentStaffing(data);
  const workload = operationalWorkload(data, today);
  const zone = actorFacilityZone(data);
  const defaults = today ? defaultFlowRange(today) : null;
  const [range, setRange] = useState<{ actor: string; from: string; to: string } | null>(null);
  const actor = String(data.actor?.id ?? "");
  const activeRange =
    range && range.actor === actor
      ? range
      : { actor, from: defaults?.from ?? "", to: defaults?.to ?? "" };
  const ledger = flowLedger(data, activeRange.from, activeRange.to);
  const maxRole = Math.max(...staffing.activeByRole.map((item) => item.count), 1);
  return (
    <div className="ws-executive">
      <section className="ws-panel" aria-labelledby="ws-census-heading">
        <span className="ws-kicker">Current active care episodes</span>
        <div className="ws-split">
          <h2 id="ws-census-heading">Census by facility and level</h2>
          <button type="button" className="ws-link" onClick={() => onNavigate("clients", "active")}>
            Active clients
          </button>
        </div>
        {!census.available ? (
          <div className="ws-unavailable">
            <strong>Not included</strong>
            <p>Episodes are not included for this access. Census is not shown as zero.</p>
          </div>
        ) : census.totalActive === 0 ? (
          <p>No active care episodes. Census will appear as clients enter care.</p>
        ) : (
          census.facilities.map((facility) => (
            <div className="ws-band" key={facility.id}>
              <h3>{facility.name} · {facility.total} current</h3>
              <div className="ws-stack">
                {facility.levels.map((level) => (
                  <button
                    type="button"
                    key={level.level}
                    style={{ flex: level.count, background: LEVEL_COLORS[level.level] || "var(--ws-accent)" }}
                    onClick={() => onNavigate("clients", `census:${facility.id}:${level.level}`)}
                  >
                    {label(level.level)} {level.count}
                  </button>
                ))}
              </div>
            </div>
          ))
        )}
        {census.available && census.unclassified > 0 && (
          <p>{census.unclassified} loaded episode{census.unclassified === 1 ? "" : "s"} have no status and are not counted as active or inactive.</p>
        )}
      </section>
      <div className="ws-exec-grid">
        <section className="ws-panel" aria-labelledby="ws-staff-heading">
          <div className="ws-split">
            <h2 id="ws-staff-heading">Active staffing</h2>
            <button type="button" className="ws-link" onClick={() => onNavigate("team")}>
              Team
            </button>
          </div>
          {!staffing.available ? (
            <p>Staff records are not included. Active roles are not shown as zero.</p>
          ) : (
            <>
              <p className="ws-current">Current · {staffing.activeTotal} active</p>
              <ul className="ws-bars">
                {staffing.activeByRole.map((item) => (
                  <li key={item.role}>
                    <button type="button" onClick={() => onNavigate("team", `role:${item.role}`)}>
                      <span>{label(item.role)}</span>
                      <span className="ws-track" aria-hidden="true">
                        <span className="ws-fill" style={{ width: `${(item.count / maxRole) * 100}%`, background: "var(--ws-accent)" }} />
                      </span>
                      <strong>{item.count}</strong>
                    </button>
                  </li>
                ))}
              </ul>
              {staffing.unclassified > 0 && <p>{staffing.unclassified} staff record{staffing.unclassified === 1 ? "" : "s"} have no active flag and are excluded from both active and inactive totals.</p>}
            </>
          )}
        </section>
        <section className="ws-panel" aria-labelledby="ws-work-heading">
          <h2 id="ws-work-heading">Operational workload</h2>
          <p className="ws-current">Open work across all dates.</p>
          <button type="button" className="ws-row" onClick={() => onNavigate("tasks", "unfinished")}>
            <strong>{workload.tasks.available ? workload.tasks.open : "Not available"}</strong>
            <span>Open and in-progress tasks</span>
          </button>
          <p>
            Overdue: {workload.tasks.overdue === null ? "Not calculated without a facility date or task access." : workload.tasks.overdue}
          </p>
          <button type="button" className="ws-row" disabled={!workload.sessions.available} onClick={() => onNavigate("schedule", "pending")}>
            <strong>{workload.sessions.available ? workload.sessions.scheduled : "Not available"}</strong>
            <span>Client appointments awaiting attendance</span>
          </button>
          <p>
            Draft notes: {workload.drafts.available ? workload.drafts.count : "Not available"}.
            {" "}
            {workload.drafts.available ? "Review documentation in the clinical workspace." : "Clinical document access is required."}
          </p>
        </section>
      </div>
      <section className="ws-ledger" aria-labelledby="ws-actuals-heading">
        <div>
          <span className="ws-kicker">Financial actuals · {zone}</span>
          <h2 id="ws-actuals-heading">Financial activity</h2>
          <FlowControls
            from={activeRange.from}
            to={activeRange.to}
            onChange={(next) => setRange({ actor, ...next })}
          />
          <div className="ws-tape">
            <article>
              <h3>Charges</h3>
              <MoneyFigure figure={ledger.charges} />
            </article>
            <article>
              <h3>Cash</h3>
              <MoneyFigure figure={ledger.cash} />
            </article>
            <article>
              <h3>Adjustments</h3>
              <MoneyFigure figure={ledger.adjustments} />
            </article>
          </div>
          <button type="button" className="ws-link" onClick={() => onNavigate("billing", "claims")}>
            Open billing records
          </button>
        </div>
        <aside className="ws-stock">
          <span className="ws-current">Current outstanding</span>
          <MoneyFigure figure={ledger.outstanding} />
          <details><summary>Definition</summary><p>{LEDGER_DEFINITIONS.outstanding}</p></details>
        </aside>
      </section>
      <section className="ws-unavailable" aria-labelledby="ws-missing-metrics">
        <h2 id="ws-missing-metrics">Not in this workspace</h2>
        <p>{UNAVAILABLE_EXECUTIVE_METRICS}</p>
      </section>
    </div>
  );
}

export default function RoleWorkspace(props: RoleWorkspaceProps) {
  const allowed = allowedWorkspaceModes(props.data);
  const mode = allowed.includes(props.mode) ? props.mode : defaultWorkspaceMode(props.data);
  const now = new Date();
  const today = calendarDateInZone(now, actorFacilityZone(props.data));
  const home = { ...props, now, today };
  return (
    <div className={`ws-root workspace-mode-${mode}`} data-mode={mode}>
      {mode === "clinical" ? (
        <ClinicalHome {...home} />
      ) : mode === "billing" ? (
        <BillingHome {...home} />
      ) : (
        <ExecutiveHome {...home} />
      )}
    </div>
  );
}
