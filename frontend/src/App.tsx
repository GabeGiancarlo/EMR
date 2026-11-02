import Setup from "./Setup";
import NoteComposer from "./NoteComposer";
import CarePlanning from "./CarePlanning";
import GroupEncounters from "./GroupEncounters";
import StructuredForms from "./StructuredForms";
import ExportReports from "./ExportReports";
import {
  ClaimCorrections,
  PaymentCorrection,
  ReversalHistory,
} from "./FinancialActions";
import ClaimReadiness from "./ClaimReadiness";
import ThemeControl from "./ThemeControl";
import { useCallback, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import {
  actorStorageKey,
  clientName,
  date,
  initials,
  label,
  money,
  numeric,
  request,
  value,
  withOriginalUpdatedAt,
} from "./api";
import type { Bootstrap, Resource, Row } from "./api";
import { definitions, RecordForm } from "./forms";
import type { Field } from "./forms";
import { Badge, Brand, Button, Empty, Icon, Modal, Panel, Tabs } from "./ui";
import type { IconName } from "./ui";

type View =
  | "overview"
  | "clients"
  | "schedule"
  | "notes"
  | "billing"
  | "tasks"
  | "team"
  | "settings";
type Dialog = {
  title: string;
  description?: string;
  resource?: Resource;
  record?: Row;
  initial?: Record<string, unknown>;
  fields?: Field[];
  path?: string;
  submitLabel?: string;
  kind?: "note" | "viewnote";
};
type Column = { label: string; render: (r: Row) => ReactNode };
const nav: { id: View; label: string; icon: IconName; section?: string }[] = [
  { id: "overview", label: "Overview", icon: "overview", section: "WORKSPACE" },
  { id: "clients", label: "Clients", icon: "clients" },
  { id: "schedule", label: "Schedule", icon: "schedule" },
  { id: "notes", label: "Documentation", icon: "notes" },
  { id: "billing", label: "Billing & revenue", icon: "billing" },
  { id: "tasks", label: "Tasks", icon: "tasks" },
  { id: "team", label: "Team & access", icon: "team", section: "ORGANIZATION" },
  { id: "settings", label: "Workspace settings", icon: "settings" },
];
const titles: Record<View, { title: string; description: string }> = {
  overview: {
    title: "A clear view of care.",
    description:
      "Your people, their progress, and the work that connects it all.",
  },
  clients: {
    title: "Every person. One connected record.",
    description:
      "Follow each client from the first conversation through continuing care.",
  },
  schedule: {
    title: "Make room for meaningful care.",
    description: "Sessions, people, and documentation stay connected.",
  },
  notes: {
    title: "Care deserves a clear record.",
    description:
      "Thoughtful documentation, reusable forms, and an accountable history.",
  },
  billing: {
    title: "From care delivered to care sustained.",
    description: "Connect coverage, documentation, claims, and payments.",
  },
  tasks: {
    title: "Keep the next step in sight.",
    description: "A shared place for follow-through across your organization.",
  },
  team: {
    title: "The people behind the care.",
    description:
      "Clear responsibilities, thoughtful access, and smooth transitions.",
  },
  settings: {
    title: "A workspace that fits your practice.",
    description:
      "Shape your locations, forms, and connections around the way you care.",
  },
};
function Table({
  columns,
  rows,
  onRow,
  empty,
}: {
  columns: Column[];
  rows: Row[];
  onRow?: (r: Row) => void;
  empty?: ReactNode;
}) {
  if (!rows.length)
    return (
      <>
        {empty || (
          <Empty
            title="Nothing here yet"
            description="Records will appear here as your team starts working."
          />
        )}
      </>
    );
  return (
    <div className="table-wrap">
      <table>
        <thead>
          <tr>
            {columns.map((c, i) => (
              <th key={i} scope="col">
                {c.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr
              key={r.id}
              className={onRow ? "clickable" : ""}
              onClick={onRow ? () => onRow(r) : undefined}
              onKeyDown={
                onRow
                  ? (e) => {
                      if (e.key === "Enter") onRow(r);
                    }
                  : undefined
              }
              tabIndex={onRow ? 0 : undefined}
            >
              {columns.map((c, i) => (
                <td key={i}>{c.render(r)}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
function Person({
  name,
  sub,
  small = false,
}: {
  name: string;
  sub?: string;
  small?: boolean;
}) {
  return (
    <div className="person">
      <span className={`avatar ${small ? "small" : ""}`}>
        {initials(name) || "N"}
      </span>
      <div>
        <strong>{name}</strong>
        {sub && <small>{sub}</small>}
      </div>
    </div>
  );
}
function Metric({
  icon,
  title,
  number,
  caption,
  color = "green",
}: {
  icon: IconName;
  title: string;
  number: ReactNode;
  caption: string;
  color?: string;
}) {
  return (
    <div className="metric">
      <div className="metric-top">
        <span>{title}</span>
        <span className={`metric-icon ${color}`}>
          <Icon name={icon} size={19} />
        </span>
      </div>
      <strong className="metric-number">{number}</strong>
      <span className="metric-caption">{caption}</span>
    </div>
  );
}

export default function App() {
  const [data, setData] = useState<Bootstrap | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [toast, setToast] = useState("");
  const [refreshing, setRefreshing] = useState(false);
  const refreshInFlight = useRef(false);
  const [view, setView] = useState<View>("overview");
  const [clientId, setClientId] = useState("");
  const [chartTab, setChartTab] = useState("overview");
  const [billingTab, setBillingTab] = useState("claims");
  const [settingsTab, setSettingsTab] = useState("templates");
  const [teamTab, setTeamTab] = useState("members");
  const [scheduleTab, setScheduleTab] = useState("agenda");
  const [documentationTab, setDocumentationTab] = useState("notes");
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("all");
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [readinessClaim, setReadinessClaim] = useState<Row | null>(null);
  const [mobile, setMobile] = useState(false);
  const [actor, setActor] = useState(
    localStorage.getItem(actorStorageKey) || "",
  );
  const [weekOffset, setWeekOffset] = useState(0);
  const requestVersion = useRef(0);
  const reload = useCallback(async () => {
    const version = ++requestVersion.current;
    setError("");
    try {
      const result = await request<Bootstrap>("/api/bootstrap");
      if (version !== requestVersion.current) return false;
      setData({
        ...result,
        treatment_plans: result.treatment_plans || [],
        goals: result.goals || [],
        plan_reviews: result.plan_reviews || [],
        groups: result.groups || [],
        group_attendance: result.group_attendance || [],
        form_responses: result.form_responses || [],
        payment_reversals: result.payment_reversals || [],
      });
      return true;
    } catch (e) {
      if (version !== requestVersion.current) return false;
      setError(
        e instanceof Error
          ? e.message
          : "Nexus could not reach the local service.",
      );
      return false;
    } finally {
      if (version === requestVersion.current) setLoading(false);
    }
  }, []);
  const refreshWorkspace = async () => {
    if (refreshInFlight.current) return;
    refreshInFlight.current = true;
    setRefreshing(true);
    setToast("");
    const refreshed = await reload();
    if (refreshed) setToast("Workspace refreshed.");
    refreshInFlight.current = false;
    setRefreshing(false);
  };
  useEffect(() => {
    void reload();
  }, [reload, actor]);
  useEffect(() => {
    if (toast) {
      const timer = setTimeout(() => setToast(""), 4500);
      return () => clearTimeout(timer);
    }
  }, [toast]);
  const changeActor = (id: string) => {
    ++requestVersion.current;
    if (id) localStorage.setItem(actorStorageKey, id);
    else localStorage.removeItem(actorStorageKey);
    setActor(id);
    setData(null);
    setLoading(true);
    setDialog(null);
    setReadinessClaim(null);
    setClientId("");
    setView("overview");
    setQuery("");
    setFilter("all");
    setMobile(false);
    setToast("");
    if (id === actor) void reload();
  };
  const navigate = (next: View) => {
    setView(next);
    setClientId("");
    setQuery("");
    setFilter("all");
    setMobile(false);
    setReadinessClaim(null);
  };
  const role = value(data?.actor, "role");
  const admin = role === "admin";
  const can = (resource: Resource, fallback: boolean) =>
    data?.permissions?.[resource]?.write ?? fallback;
  const canRead = (resource: Resource, fallback = true) =>
    data?.permissions?.[resource]?.read ?? fallback;
  const clinical = can(
    "notes",
    ["admin", "clinical_lead", "clinician"].includes(role),
  );
  const canClients = can(
    "clients",
    ["admin", "clinical_lead", "clinician", "support"].includes(role),
  );
  const canSessions = can("sessions", canClients);
  const billing = can(
    billingTab as Resource,
    ["admin", "billing"].includes(role),
  );
  const canTemplates = can(
    "templates",
    ["admin", "clinical_lead"].includes(role),
  );
  const client = (id: unknown) => data?.clients.find((c) => c.id === id);
  const staff = (id: unknown) => data?.staff.find((s) => s.id === id);
  const facility = (id: unknown) => data?.facilities.find((f) => f.id === id);
  const selectedClient = data?.clients.find((c) => c.id === clientId);
  const create = (resource: Resource, initial: Record<string, unknown> = {}) =>
    setDialog({
      resource,
      title: `New ${definitions[resource].title.toLowerCase()}`,
      description: definitions[resource].description,
      initial: {
        ...(data?.facilities.length === 1
          ? { facility_id: data.facilities[0].id }
          : {}),
        ...initial,
      },
      ...(resource === "notes" ? { kind: "note" as const } : {}),
    });
  const edit = (resource: Resource, record: Row) =>
    setDialog({
      resource,
      record,
      title: `Edit ${definitions[resource].title.toLowerCase()}`,
      description: definitions[resource].description,
      initial: record,
      fields: definitions[resource].fields.filter(
        (f) =>
          ![
            "client_id",
            "episode_id",
            "session_id",
            "policy_id",
            "authorization_id",
            "claim_id",
            "group_id",
            "plan_id",
          ].includes(f.key) && !(resource === "staff" && f.key === "active"),
      ),
    });
  const action = (
    path: string,
    title: string,
    description: string,
    fields: Field[] = [],
    initial: Record<string, unknown> = {},
    submitLabel = "Confirm",
    record?: Row,
  ) =>
    setDialog({
      path,
      title,
      description,
      fields,
      initial,
      submitLabel,
      record,
    });
  async function mutate(path: string, body: unknown, method = "POST") {
    await request(path, method, body);
    setDialog(null);
    await reload();
    setToast("Saved. Your workspace is up to date.");
  }
  async function quick(path: string, body: unknown, method = "PATCH") {
    try {
      await mutate(path, body, method);
    } catch (e) {
      setError(
        e instanceof Error ? e.message : "This change could not be saved.",
      );
    }
  }
  function openChart(row: Row) {
    setClientId(row.id);
    setChartTab("overview");
    setView("clients");
    setQuery("");
  }
  const matching = (rows: Row[]) =>
    rows.filter(
      (r) =>
        !query ||
        Object.values(r).some(
          (v) =>
            typeof v === "string" &&
            v.toLowerCase().includes(query.toLowerCase()),
        ) ||
        clientName(client(r.client_id))
          .toLowerCase()
          .includes(query.toLowerCase()),
    );
  const clientCell = (r: Row) => (
    <Person
      name={clientName(client(r.client_id))}
      sub={value(r, "service_code") || undefined}
      small
    />
  );
  const toolbar = (placeholder: string, extra?: ReactNode) => (
    <div className="toolbar">
      <div className="search-field">
        <Icon name="search" size={18} />
        <input
          aria-label={placeholder}
          placeholder={placeholder}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        {query && (
          <button
            className="icon-button"
            onClick={() => setQuery("")}
            aria-label="Clear search"
          >
            <Icon name="close" size={15} />
          </button>
        )}
      </div>
      {extra}
    </div>
  );
  const taskColumns: Column[] = [
    {
      label: "Task",
      render: (r) => (
        <div>
          <strong>{value(r, "title")}</strong>
          <small>{value(r, "description")}</small>
        </div>
      ),
    },
    {
      label: "Assigned to",
      render: (r) => value(staff(r.assignee_id), "name") || "Unassigned",
    },
    { label: "Priority", render: (r) => <Badge status={r.priority} /> },
    { label: "Due", render: (r) => date(r.due_date) },
    { label: "Status", render: (r) => <Badge status={r.status} /> },
    {
      label: "Actions",
      render: (r) => (
        <div className="row-actions">
          <Button variant="text small" onClick={() => edit("tasks", r)}>
            Edit
          </Button>
          {r.status !== "done" && (
            <Button
              variant="text small"
              onClick={() =>
                void quick(
                  `/api/tasks/${r.id}`,
                  withOriginalUpdatedAt(r, { status: "done" }),
                )
              }
            >
              Complete
            </Button>
          )}
        </div>
      ),
    },
  ];
  const noteColumns: Column[] = [
    {
      label: "Note",
      render: (r) => (
        <div>
          <strong>{value(r, "title")}</strong>
          <small>{date(r.created_at)}</small>
        </div>
      ),
    },
    { label: "Client", render: clientCell },
    {
      label: "Author",
      render: (r) => value(staff(r.author_id), "name") || "Care team",
    },
    { label: "Status", render: (r) => <Badge status={r.status} /> },
    {
      label: "",
      render: (r) => (
        <Button
          variant="text small"
          onClick={() =>
            setDialog({ title: value(r, "title"), record: r, kind: "viewnote" })
          }
        >
          Open note <Icon name="arrow" size={15} />
        </Button>
      ),
    },
  ];

  if (loading && !data)
    return (
      <div className="loading-page">
        <div className="entry-appearance">
          <ThemeControl />
        </div>
        <Brand />
        <div className="spinner" />
        <p>Connecting your workspace…</p>
      </div>
    );
  if (!data)
    return (
      <div className="loading-page">
        <div className="entry-appearance">
          <ThemeControl />
        </div>
        <Brand />
        <div className="connection-card">
          <Icon name="link" size={32} />
          <h1>Let’s get connected.</h1>
          <p>
            The Nexus local service is not available yet. Start the database and
            API, then reconnect.
          </p>
          <div className="error-banner" role="alert">
            {error}
          </div>
          <Button
            onClick={() => {
              setLoading(true);
              void reload();
            }}
          >
            Reconnect <Icon name="arrow" size={17} />
          </Button>
        </div>
      </div>
    );
  if (data.setup_required)
    return (
      <>
        <div className="entry-appearance">
          <ThemeControl />
        </div>
        <Setup onDone={changeActor} />
      </>
    );
  if (!data.actor)
    return (
      <div className="loading-page">
        <div className="entry-appearance">
          <ThemeControl />
        </div>
        <Brand />
        <div className="connection-card">
          <span className="eyebrow">LOCAL WORKSPACE</span>
          <h1>Welcome back.</h1>
          <p>
            Select the local operator whose permissions you want to use. This
            selection is not authentication.
          </p>
          <div className="operator-list">
            {data.staff
              .filter((s) => s.active !== false)
              .map((s) => (
                <button key={s.id} onClick={() => changeActor(s.id)}>
                  <Person
                    name={value(s, "name")}
                    sub={label(value(s, "role"))}
                  />
                  <Icon name="arrow" size={18} />
                </button>
              ))}
          </div>
          {error && (
            <div className="error-banner" role="alert">
              {error}
            </div>
          )}
        </div>
      </div>
    );

  const orgName = value(data.organizations[0], "name") || "Your organization";
  const metric = data.metrics;
  const sessionsByDate = [...data.sessions].sort((a, b) =>
    value(a, "starts_at").localeCompare(value(b, "starts_at")),
  );
  const upcoming = sessionsByDate
    .filter(
      (s) =>
        s.status === "scheduled" &&
        new Date(value(s, "starts_at")).getTime() >= Date.now(),
    )
    .slice(0, 4);
  const allNotes = data.notes;
  const dueTasks = data.tasks.filter((t) => t.status !== "done");
  const primaryAction: Partial<Record<View, ReactNode>> = {
    clients: canClients ? (
      <Button onClick={() => create("clients")}>
        <Icon name="plus" size={17} /> New client
      </Button>
    ) : null,
    schedule: can(
      scheduleTab === "groups" ? "groups" : "sessions",
      canSessions,
    ) ? (
      <Button
        onClick={() => create(scheduleTab === "groups" ? "groups" : "sessions")}
      >
        <Icon name="plus" size={17} />{" "}
        {scheduleTab === "groups" ? "Schedule group" : "Schedule session"}
      </Button>
    ) : null,
    notes:
      clinical && documentationTab === "notes" ? (
        <Button onClick={() => create("notes")}>
          <Icon name="plus" size={17} /> New note
        </Button>
      ) : null,
    billing: billing ? (
      <Button onClick={() => create(billingTab as Resource)}>
        <Icon name="plus" size={17} /> New{" "}
        {definitions[billingTab as Resource]?.title.toLowerCase()}
      </Button>
    ) : null,
    tasks: (
      <Button onClick={() => create("tasks")}>
        <Icon name="plus" size={17} /> New task
      </Button>
    ),
    team: admin ? (
      <Button
        onClick={() =>
          create(teamTab === "exceptions" ? "exceptions" : "staff")
        }
      >
        <Icon name="plus" size={17} />{" "}
        {teamTab === "exceptions" ? "Access exception" : "Add team member"}
      </Button>
    ) : null,
    settings:
      settingsTab === "templates" && canTemplates ? (
        <Button onClick={() => create("templates")}>
          <Icon name="plus" size={17} /> New template
        </Button>
      ) : settingsTab === "facilities" && admin ? (
        <Button onClick={() => create("facilities")}>
          <Icon name="plus" size={17} /> Add facility
        </Button>
      ) : null,
  };

  function renderOverview() {
    return (
      <>
        <div className="metric-grid">
          <Metric
            icon="clients"
            title="Active clients"
            number={metric.active_clients}
            caption={`${data!.episodes.filter((e) => e.status === "active").length} active care episodes`}
          />
          <Metric
            icon="schedule"
            title="Scheduled sessions"
            number={metric.scheduled_sessions}
            caption="Across your care team"
            color="blue"
          />
          <Metric
            icon="notes"
            title="Notes awaiting signature"
            number={metric.draft_notes}
            caption="Drafts in the documentation queue"
            color="sand"
          />
          <Metric
            icon="billing"
            title="Outstanding balance"
            number={money(metric.outstanding_cents)}
            caption="Submitted balances less payments and adjustments"
            color="rose"
          />
        </div>
        <div className="overview-grid">
          <Panel
            title="Your care pathway"
            subtitle="One connected journey, from arrival to continuing care."
            className="pathway-panel"
          >
            <div className="pathway">
              {[
                {
                  name: "Intake",
                  count: data!.clients.filter((c) =>
                    ["inquiry", "intake"].includes(value(c, "status")),
                  ).length,
                  icon: "clients",
                  view: "clients",
                },
                {
                  name: "In care",
                  count: metric.active_clients,
                  icon: "leaf",
                  view: "clients",
                },
                {
                  name: "Documentation",
                  count: metric.draft_notes,
                  icon: "notes",
                  view: "notes",
                },
                {
                  name: "Billing readiness",
                  count: data!.claims.filter((c) => c.status === "ready")
                    .length,
                  icon: "billing",
                  view: "billing",
                },
              ].map((step, i) => (
                <button
                  key={step.name}
                  onClick={() => navigate(step.view as View)}
                  className="pathway-step"
                >
                  <span className="pathway-num">0{i + 1}</span>
                  <span className={`pathway-icon tone-${i}`}>
                    <Icon name={step.icon as IconName} size={24} />
                  </span>
                  <strong>{step.name}</strong>
                  <span>
                    {step.count}{" "}
                    {i === 0
                      ? "clients"
                      : i === 1
                        ? "clients"
                        : i === 2
                          ? "drafts"
                          : "ready claims"}
                  </span>
                  {i < 3 && (
                    <span className="pathway-arrow">
                      <Icon name="arrow" size={17} />
                    </span>
                  )}
                </button>
              ))}
            </div>
          </Panel>
          <Panel className="care-note">
            <span className="eyebrow">A LITTLE MORE CLARITY</span>
            <Icon name="leaf" size={34} />
            <h2>Good care is a team effort.</h2>
            <p>
              Keep handoffs visible. Give every next step an owner, a date, and
              a place in the client’s story.
            </p>
            <Button variant="text" onClick={() => navigate("tasks")}>
              See your team’s tasks <Icon name="arrow" size={17} />
            </Button>
          </Panel>
          <Panel
            title="Coming up"
            subtitle="Your next scheduled sessions"
            action={
              <Button variant="text small" onClick={() => navigate("schedule")}>
                View schedule <Icon name="arrow" size={15} />
              </Button>
            }
          >
            {upcoming.length ? (
              <div className="session-list">
                {upcoming.map((s) => (
                  <div className="session-row" key={s.id}>
                    <div className="session-date">
                      <strong>
                        {new Date(value(s, "starts_at")).getDate()}
                      </strong>
                      <span>
                        {new Date(value(s, "starts_at")).toLocaleDateString(
                          "en-US",
                          { month: "short" },
                        )}
                      </span>
                    </div>
                    <div className="grow">
                      <strong>{clientName(client(s.client_id))}</strong>
                      <small>
                        {label(value(s, "kind"))} ·{" "}
                        {value(staff(s.clinician_id), "name")}
                      </small>
                    </div>
                    <div className="align-right">
                      <span>
                        {new Date(value(s, "starts_at")).toLocaleTimeString(
                          "en-US",
                          { hour: "numeric", minute: "2-digit" },
                        )}
                      </span>
                      <small>{value(s, "location")}</small>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <Empty
                icon="schedule"
                title="A little space in the schedule"
                description="Your upcoming sessions will appear here when care is scheduled."
                action={
                  clinical ? (
                    <Button
                      variant="secondary small"
                      onClick={() => create("sessions")}
                    >
                      Schedule a session
                    </Button>
                  ) : undefined
                }
              />
            )}
          </Panel>
          <Panel
            title="Needs attention"
            subtitle="The next steps that move care forward"
            action={<span className="count-pill">{dueTasks.length}</span>}
          >
            {dueTasks.length ? (
              <div className="attention-list">
                {dueTasks.slice(0, 5).map((t) => (
                  <button
                    key={t.id}
                    className="attention-row"
                    onClick={() => edit("tasks", t)}
                  >
                    <span className={`priority-dot ${value(t, "priority")}`} />
                    <div className="grow">
                      <strong>{value(t, "title")}</strong>
                      <small>
                        {value(staff(t.assignee_id), "name") || "Unassigned"} ·{" "}
                        {date(t.due_date)}
                      </small>
                    </div>
                    <Icon name="chevron" size={16} />
                  </button>
                ))}
              </div>
            ) : (
              <Empty
                icon="check"
                title="A clear next step starts here"
                description="Add tasks for admissions, documentation, coverage, or care coordination."
                action={
                  <Button
                    variant="secondary small"
                    onClick={() => create("tasks")}
                  >
                    Add a task
                  </Button>
                }
              />
            )}
          </Panel>
        </div>
        {!data!.clients.length && (
          <div className="start-strip">
            <span className="start-icon">
              <Icon name="clients" size={24} />
            </span>
            <div>
              <h3>Your workspace is ready for its first client.</h3>
              <p>
                Start with an intake record, then connect an episode, coverage,
                and care team.
              </p>
            </div>
            {canClients && (
              <Button onClick={() => create("clients")}>
                Start an intake <Icon name="arrow" size={17} />
              </Button>
            )}
          </div>
        )}
      </>
    );
  }

  function renderClients() {
    if (selectedClient) return renderChart(selectedClient);
    const rows = matching(data!.clients).filter(
      (c) => filter === "all" || c.status === filter,
    );
    return (
      <Panel>
        {toolbar(
          "Search by name, email, or record…",
          <select
            aria-label="Filter client status"
            className="compact-select"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          >
            <option value="all">All client stages</option>
            {["inquiry", "intake", "active", "discharged"].map((s) => (
              <option key={s} value={s}>
                {label(s)}
              </option>
            ))}
          </select>,
        )}
        <Table
          rows={rows}
          columns={[
            {
              label: "Client",
              render: (c) => (
                <button
                  className="client-link"
                  onClick={() => openChart(c)}
                  aria-label={`Open chart for ${clientName(c)}`}
                >
                  <Person
                    name={clientName(c)}
                    sub={
                      value(c, "preferred_name")
                        ? `Goes by ${value(c, "preferred_name")}`
                        : `Record ${c.id.slice(0, 8).toUpperCase()}`
                    }
                  />
                </button>
              ),
            },
            { label: "Date of birth", render: (c) => date(c.birth_date) },
            { label: "Stage", render: (c) => <Badge status={c.status} /> },
            {
              label: "Care team",
              render: (c) =>
                value(staff(c.primary_clinician_id), "name") || (
                  <span className="muted">Not assigned</span>
                ),
            },
            {
              label: "Facility",
              render: (c) =>
                value(facility(c.facility_id), "name") || "Not assigned",
            },
            { label: "", render: () => <Icon name="chevron" size={17} /> },
          ]}
          empty={
            <Empty
              icon="clients"
              title={
                query
                  ? "No matching clients"
                  : "Every care journey begins with a conversation"
              }
              description={
                query
                  ? "Try a different search or stage."
                  : "Create the first intake record to start connecting care, documentation, and coverage."
              }
              action={
                !query && canClients ? (
                  <Button onClick={() => create("clients")}>
                    <Icon name="plus" size={17} /> New client
                  </Button>
                ) : undefined
              }
            />
          }
        />
      </Panel>
    );
  }

  function renderChart(c: Row) {
    const canEpisodes = can("episodes", canClients);
    const canConsents = can("consents", canClients);
    const canPolicies = can("policies", ["admin", "billing"].includes(role));
    const episodes = data!.episodes.filter((e) => e.client_id === c.id);
    const notes = data!.notes.filter((n) => n.client_id === c.id);
    const tasks = data!.tasks.filter((t) => t.client_id === c.id);
    const policies = data!.policies.filter((p) => p.client_id === c.id);
    const consents = data!.consents.filter((p) => p.client_id === c.id);
    const sessions = data!.sessions.filter((s) => s.client_id === c.id);
    return (
      <>
        <button className="back-link" onClick={() => setClientId("")}>
          <Icon name="back" size={16} /> All clients
        </button>
        <div className="chart-banner">
          <Person
            name={clientName(c)}
            sub={
              value(c, "preferred_name")
                ? `Preferred name: ${value(c, "preferred_name")}`
                : "Client record"
            }
          />
          <div className="chart-key">
            <span>Date of birth</span>
            <strong>{date(c.birth_date)}</strong>
          </div>
          <div className="chart-key">
            <span>Primary clinician</span>
            <strong>
              {value(staff(c.primary_clinician_id), "name") || "Not assigned"}
            </strong>
          </div>
          <div className="chart-key">
            <span>Current stage</span>
            <Badge status={c.status} />
          </div>
          {canClients && (
            <Button
              variant="secondary small"
              onClick={() => edit("clients", c)}
            >
              Edit details
            </Button>
          )}
        </div>
        <Tabs
          value={chartTab}
          onChange={setChartTab}
          tabs={[
            { id: "overview", label: "Overview" },
            { id: "episodes", label: "Care episodes", count: episodes.length },
            ...((data!.permissions?.treatment_plans?.read ?? clinical)
              ? [
                  {
                    id: "plan",
                    label: "Treatment plan",
                    count: data!.treatment_plans.filter(
                      (p) => p.client_id === c.id,
                    ).length,
                  },
                ]
              : []),
            { id: "notes", label: "Documentation", count: notes.length },
            ...(canRead("form_responses", clinical)
              ? [
                  {
                    id: "forms",
                    label: "Forms",
                    count: data!.form_responses.filter(
                      (r) => r.client_id === c.id,
                    ).length,
                  },
                ]
              : []),
            { id: "coverage", label: "Coverage", count: policies.length },
            { id: "tasks", label: "Tasks", count: tasks.length },
            { id: "consents", label: "Consents", count: consents.length },
          ]}
        />
        {chartTab === "plan" && (
          <CarePlanning
            clientId={c.id}
            data={data!}
            canWrite={can("treatment_plans", clinical)}
            create={create}
            edit={edit}
            action={action}
            mutate={mutate}
          />
        )}
        {chartTab === "overview" && (
          <div className="chart-grid">
            <Panel title="Client information">
              <dl className="detail-list">
                <div>
                  <dt>Email</dt>
                  <dd>{value(c, "email") || "Not provided"}</dd>
                </div>
                <div>
                  <dt>Phone</dt>
                  <dd>{value(c, "phone") || "Not provided"}</dd>
                </div>
                <div>
                  <dt>Facility</dt>
                  <dd>
                    {value(facility(c.facility_id), "name") || "Not assigned"}
                  </dd>
                </div>
                <div>
                  <dt>Emergency contact</dt>
                  <dd>{value(c, "emergency_contact") || "Not provided"}</dd>
                </div>
                <div>
                  <dt>Created</dt>
                  <dd>{date(c.created_at)}</dd>
                </div>
              </dl>
            </Panel>
            <Panel
              title="Connected care"
              subtitle="Complete the foundations for this client’s journey"
            >
              <div className="checklist">
                {[
                  {
                    done: episodes.length > 0,
                    title: "Open a care episode",
                    sub: "Set the level of care and admission date.",
                    fn: () =>
                      create("episodes", {
                        client_id: c.id,
                        facility_id: c.facility_id,
                      }),
                    allowed: canClients,
                  },
                  {
                    done: !!c.primary_clinician_id,
                    title: "Assign a primary clinician",
                    sub: "Give care coordination a clear owner.",
                    fn: () => edit("clients", c),
                    allowed: canClients,
                  },
                  {
                    done: consents.length > 0,
                    title: "Record consent",
                    sub: "Document the purpose, scope, and dates.",
                    fn: () => create("consents", { client_id: c.id }),
                    allowed: canConsents,
                  },
                  {
                    done: policies.length > 0,
                    title: "Connect coverage",
                    sub: "Add insurance and authorization details.",
                    fn: () => create("policies", { client_id: c.id }),
                    allowed: canPolicies,
                  },
                ].map((step) => (
                  <button
                    key={step.title}
                    disabled={!step.allowed}
                    onClick={step.fn}
                  >
                    <span
                      className={`check-circle ${step.done ? "complete" : ""}`}
                    >
                      {step.done ? (
                        <Icon name="check" size={15} />
                      ) : (
                        <Icon name="plus" size={14} />
                      )}
                    </span>
                    <div>
                      <strong>{step.title}</strong>
                      <small>{step.sub}</small>
                    </div>
                    <Icon name="chevron" size={15} />
                  </button>
                ))}
              </div>
            </Panel>
            <Panel
              title="Sessions"
              action={
                clinical ? (
                  <Button
                    variant="text small"
                    onClick={() =>
                      create("sessions", {
                        client_id: c.id,
                        episode_id: episodes.find((e) => e.status === "active")
                          ?.id,
                      })
                    }
                  >
                    Schedule <Icon name="plus" size={15} />
                  </Button>
                ) : undefined
              }
            >
              <Table
                rows={sessions}
                columns={[
                  {
                    label: "Session",
                    render: (s) => (
                      <div>
                        <strong>{label(value(s, "kind"))}</strong>
                        <small>{date(s.starts_at, true)}</small>
                      </div>
                    ),
                  },
                  {
                    label: "Clinician",
                    render: (s) => value(staff(s.clinician_id), "name"),
                  },
                  {
                    label: "Status",
                    render: (s) => <Badge status={s.status} />,
                  },
                ]}
                empty={
                  <Empty
                    icon="schedule"
                    title="Care is ready to be scheduled"
                    description="Link the first session to this client’s care episode."
                  />
                }
              />
            </Panel>
            <Panel title="Care record at a glance">
              <div className="chart-counts">
                <div>
                  <strong>
                    {episodes.filter((e) => e.status === "active").length}
                  </strong>
                  <span>Active episodes</span>
                </div>
                <div>
                  <strong>
                    {notes.filter((n) => n.status === "signed").length}
                  </strong>
                  <span>Signed notes</span>
                </div>
                <div>
                  <strong>
                    {tasks.filter((t) => t.status !== "done").length}
                  </strong>
                  <span>Open tasks</span>
                </div>
              </div>
              <div className="quiet-note">
                <Icon name="shield" size={18} />
                <span>
                  Record access and changes are captured in your organization’s
                  audit history.
                </span>
              </div>
            </Panel>
          </div>
        )}
        {chartTab === "episodes" && (
          <Panel
            title="Episodes of care"
            action={
              canEpisodes ? (
                <Button
                  variant="secondary small"
                  onClick={() =>
                    create("episodes", {
                      client_id: c.id,
                      facility_id: c.facility_id,
                    })
                  }
                >
                  <Icon name="plus" size={16} /> Add episode
                </Button>
              ) : undefined
            }
          >
            <Table
              rows={episodes}
              columns={[
                {
                  label: "Level of care",
                  render: (e) => (
                    <strong>{label(value(e, "level_of_care"))}</strong>
                  ),
                },
                { label: "Admission", render: (e) => date(e.start_date) },
                { label: "Discharge", render: (e) => date(e.end_date) },
                { label: "Status", render: (e) => <Badge status={e.status} /> },
                {
                  label: "Actions",
                  render: (e) =>
                    canEpisodes && e.status !== "discharged" ? (
                      <Button
                        variant="text small"
                        onClick={() =>
                          action(
                            `/api/episodes/${e.id}/discharge`,
                            "Discharge care episode",
                            "Close this episode with a documented reason. Other active episodes remain open.",
                            [
                              {
                                key: "end_date",
                                label: "Discharge date",
                                type: "date",
                                required: true,
                              },
                              {
                                key: "discharge_reason",
                                label: "Discharge reason and transition plan",
                                type: "textarea",
                                required: true,
                              },
                            ],
                            {},
                            "Discharge episode",
                          )
                        }
                      >
                        Discharge
                      </Button>
                    ) : (
                      value(e, "discharge_reason")
                    ),
                },
              ]}
              empty={
                <Empty
                  title="A new chapter in care"
                  description="Add a care episode to define admission, level of care, and discharge."
                />
              }
            />
          </Panel>
        )}
        {chartTab === "forms" && (
          <StructuredForms
            clientId={c.id}
            data={data!}
            canWrite={can("form_responses", clinical)}
            create={create}
            edit={edit}
            action={action}
            mutate={mutate}
            onAuthorRules={
              canTemplates
                ? () => {
                    setClientId("");
                    setSettingsTab("templates");
                    setView("settings");
                  }
                : undefined
            }
          />
        )}
        {chartTab === "notes" && (
          <Panel
            title="Clinical documentation"
            action={
              clinical ? (
                <Button
                  variant="secondary small"
                  onClick={() => create("notes", { client_id: c.id })}
                >
                  <Icon name="plus" size={16} /> New note
                </Button>
              ) : undefined
            }
          >
            <Table
              rows={notes}
              columns={noteColumns}
              empty={
                <Empty
                  icon="notes"
                  title="The story of care starts here"
                  description="Create a note from your organization’s forms or write a free-form narrative."
                />
              }
            />
          </Panel>
        )}
        {chartTab === "coverage" && (
          <Panel
            title="Insurance coverage"
            action={
              canPolicies ? (
                <Button
                  variant="secondary small"
                  onClick={() => create("policies", { client_id: c.id })}
                >
                  <Icon name="plus" size={16} /> Add policy
                </Button>
              ) : undefined
            }
          >
            <Table
              rows={policies}
              columns={[
                {
                  label: "Payer",
                  render: (p) => <strong>{value(p, "payer_name")}</strong>,
                },
                { label: "Member ID", render: (p) => value(p, "member_id") },
                {
                  label: "Coverage period",
                  render: (p) =>
                    `${date(p.effective_date)} — ${p.termination_date ? date(p.termination_date) : "Ongoing"}`,
                },
                { label: "Status", render: (p) => <Badge status={p.status} /> },
                {
                  label: "",
                  render: (p) =>
                    canPolicies ? (
                      <Button
                        variant="text small"
                        onClick={() => edit("policies", p)}
                      >
                        Edit
                      </Button>
                    ) : null,
                },
              ]}
              empty={
                <Empty
                  icon="billing"
                  title="Connect coverage to care"
                  description="A billing team member can add insurance policies and track authorizations."
                />
              }
            />
          </Panel>
        )}
        {chartTab === "tasks" && (
          <Panel
            title="Client tasks"
            action={
              <Button
                variant="secondary small"
                onClick={() => create("tasks", { client_id: c.id })}
              >
                <Icon name="plus" size={16} /> Add task
              </Button>
            }
          >
            <Table rows={tasks} columns={taskColumns} />
          </Panel>
        )}
        {chartTab === "consents" && (
          <Panel
            title="Consent attestations"
            action={
              canConsents ? (
                <Button
                  variant="secondary small"
                  onClick={() => create("consents", { client_id: c.id })}
                >
                  <Icon name="plus" size={16} /> Record consent
                </Button>
              ) : undefined
            }
          >
            <Table
              rows={consents}
              columns={[
                {
                  label: "Consent",
                  render: (r) => (
                    <div>
                      <strong>{value(r, "kind")}</strong>
                      <small>{value(r, "scope")}</small>
                    </div>
                  ),
                },
                { label: "Granted", render: (r) => date(r.granted_on) },
                {
                  label: "Expires",
                  render: (r) =>
                    r.expires_on ? date(r.expires_on) : "Not specified",
                },
                {
                  label: "Status",
                  render: (r) => (
                    <Badge status={r.revoked_on ? "revoked" : "active"} />
                  ),
                },
                {
                  label: "",
                  render: (r) =>
                    !r.revoked_on && canConsents ? (
                      <Button
                        variant="text small danger"
                        onClick={() =>
                          action(
                            `/api/consents/${r.id}`,
                            "Revoke consent",
                            "Record when this consent was revoked.",
                            [
                              {
                                key: "revoked_on",
                                label: "Revocation date",
                                type: "date",
                                required: true,
                              },
                            ],
                            {},
                            "Record revocation",
                            r,
                          )
                        }
                      >
                        Revoke
                      </Button>
                    ) : null,
                },
              ]}
              empty={
                <Empty
                  icon="shield"
                  title="Make the scope of consent visible"
                  description="Record the client’s attestation and preserve the original signed documentation."
                />
              }
            />
          </Panel>
        )}
      </>
    );
  }

  function renderSchedule() {
    return (
      <>
        <Tabs
          value={scheduleTab}
          onChange={setScheduleTab}
          tabs={[
            { id: "agenda", label: "Individual agenda" },
            {
              id: "groups",
              label: "Group encounters",
              count: data!.groups.length,
            },
          ]}
        />
        {scheduleTab === "groups" ? (
          <GroupEncounters
            data={data!}
            canWrite={can("groups", canSessions)}
            create={create}
            edit={edit}
            action={action}
            mutate={mutate}
          />
        ) : (
          renderScheduleAgenda()
        )}
      </>
    );
  }
  function renderScheduleAgenda() {
    const start = new Date();
    start.setDate(
      start.getDate() - ((start.getDay() + 6) % 7) + weekOffset * 7,
    );
    start.setHours(0, 0, 0, 0);
    const days = Array.from({ length: 7 }, (_, i) => {
      const d = new Date(start);
      d.setDate(d.getDate() + i);
      return d;
    });
    const end = new Date(start);
    end.setDate(end.getDate() + 7);
    const rows = matching(sessionsByDate).filter((s) => {
      const d = new Date(value(s, "starts_at"));
      return d >= start && d < end;
    });
    return (
      <>
        <Panel className="calendar-panel">
          <div className="calendar-head">
            <div>
              <span className="eyebrow">WEEKLY AGENDA</span>
              <h2>
                {start.toLocaleDateString("en-US", {
                  month: "long",
                  day: "numeric",
                })}{" "}
                –{" "}
                {days[6].toLocaleDateString("en-US", {
                  month: "long",
                  day: "numeric",
                  year: "numeric",
                })}
              </h2>
            </div>
            <div className="row-actions">
              <Button
                variant="secondary small"
                onClick={() => setWeekOffset(weekOffset - 1)}
              >
                <Icon name="back" size={16} />
                <span className="sr-only">Previous week</span>
              </Button>
              <Button
                variant="secondary small"
                onClick={() => setWeekOffset(0)}
              >
                Today
              </Button>
              <Button
                variant="secondary small"
                onClick={() => setWeekOffset(weekOffset + 1)}
              >
                <Icon name="arrow" size={16} />
                <span className="sr-only">Next week</span>
              </Button>
            </div>
          </div>
          <div className="week-strip">
            {days.map((d) => {
              const count = rows.filter(
                (s) =>
                  new Date(value(s, "starts_at")).toDateString() ===
                  d.toDateString(),
              ).length;
              return (
                <div
                  key={d.toISOString()}
                  className={
                    d.toDateString() === new Date().toDateString()
                      ? "today"
                      : ""
                  }
                >
                  <span>
                    {d.toLocaleDateString("en-US", { weekday: "short" })}
                  </span>
                  <strong>{d.getDate()}</strong>
                  <small>
                    {count ? `${count} session${count > 1 ? "s" : ""}` : "Open"}
                  </small>
                </div>
              );
            })}
          </div>
        </Panel>
        <Panel>
          {toolbar("Search this week’s sessions…")}
          <Table
            rows={rows}
            columns={[
              { label: "Client", render: clientCell },
              {
                label: "When",
                render: (r) => (
                  <div>
                    <strong>{date(r.starts_at, true)}</strong>
                    <small>
                      Until{" "}
                      {new Date(value(r, "ends_at")).toLocaleTimeString(
                        "en-US",
                        { hour: "numeric", minute: "2-digit" },
                      )}
                    </small>
                  </div>
                ),
              },
              {
                label: "Session",
                render: (r) => (
                  <div>
                    {label(value(r, "kind"))}
                    <small>{value(r, "location")}</small>
                  </div>
                ),
              },
              {
                label: "Clinician",
                render: (r) => value(staff(r.clinician_id), "name"),
              },
              { label: "Status", render: (r) => <Badge status={r.status} /> },
              {
                label: "Actions",
                render: (r) =>
                  canSessions ? (
                    <div className="row-actions">
                      <Button
                        variant="text small"
                        onClick={() => edit("sessions", r)}
                      >
                        Edit
                      </Button>
                      {r.status === "scheduled" && (
                        <Button
                          variant="text small"
                          onClick={() =>
                            void quick(
                              `/api/sessions/${r.id}`,
                              withOriginalUpdatedAt(r, { status: "completed" }),
                            )
                          }
                        >
                          Complete
                        </Button>
                      )}
                      {r.status === "completed" && (
                        <Button
                          variant="text small"
                          onClick={() =>
                            create("notes", {
                              client_id: r.client_id,
                              session_id: r.id,
                            })
                          }
                        >
                          Write note
                        </Button>
                      )}
                    </div>
                  ) : null,
              },
            ]}
            empty={
              <Empty
                icon="schedule"
                title="A fresh week of possibilities"
                description="Schedule a session and connect it to the client, clinician, and care episode."
                action={
                  canSessions ? (
                    <Button
                      variant="secondary"
                      onClick={() => create("sessions")}
                    >
                      Schedule session
                    </Button>
                  ) : undefined
                }
              />
            }
          />
        </Panel>
      </>
    );
  }

  function renderNotes() {
    if (!canRead("notes", clinical) && !canRead("form_responses", clinical))
      return (
        <Panel>
          <Empty
            icon="shield"
            title="Clinical documentation has a defined audience"
            description="Your current role does not include clinical notes or form responses. Your organization can grant a documented exception when appropriate."
          />
        </Panel>
      );
    return (
      <>
        <Tabs
          value={documentationTab}
          onChange={setDocumentationTab}
          tabs={[
            { id: "notes", label: "Clinical notes", count: data!.notes.length },
            {
              id: "forms",
              label: "Structured forms",
              count: data!.form_responses.length,
            },
          ]}
        />
        {documentationTab === "forms" ? (
          <StructuredForms
            data={data!}
            canWrite={can("form_responses", clinical)}
            create={create}
            edit={edit}
            action={action}
            mutate={mutate}
          />
        ) : (
          renderClinicalNotes()
        )}
      </>
    );
  }
  function renderClinicalNotes() {
    return (
      <>
        <div className="section-summary">
          <div>
            <Icon name="notes" />
            <strong>
              {data!.notes.filter((n) => n.status === "draft").length}
            </strong>
            <span>Drafts to review</span>
          </div>
          <div>
            <Icon name="check" />
            <strong>
              {data!.notes.filter((n) => n.status === "signed").length}
            </strong>
            <span>Signed records</span>
          </div>
          <div>
            <Icon name="leaf" />
            <strong>
              {data!.templates.filter((t) => t.active !== false).length}
            </strong>
            <span>Available form templates</span>
          </div>
        </div>
        <Panel>
          {toolbar(
            "Search documentation…",
            <select
              className="compact-select"
              aria-label="Filter notes"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
            >
              <option value="all">All notes</option>
              <option value="draft">Drafts</option>
              <option value="signed">Signed</option>
            </select>,
          )}
          <Table
            rows={matching(allNotes).filter(
              (n) => filter === "all" || n.status === filter,
            )}
            columns={noteColumns}
            empty={
              <Empty
                icon="notes"
                title="Documentation with room to think"
                description="Create a clinical note, use a custom form, and sign it after your review. Signed records retain their history."
                action={
                  clinical ? (
                    <Button variant="secondary" onClick={() => create("notes")}>
                      Create a note
                    </Button>
                  ) : undefined
                }
              />
            }
          />
        </Panel>
        <div className="info-strip">
          <Icon name="shield" size={20} />
          <p>
            <strong>Human review stays at the center.</strong> Notes remain
            drafts until signed. Structured forms support local audio
            transcription and reviewed source excerpts.
          </p>
          {canTemplates && (
            <Button
              variant="text small"
              onClick={() => {
                navigate("settings");
                setSettingsTab("templates");
              }}
            >
              Manage forms <Icon name="arrow" size={16} />
            </Button>
          )}
        </div>
      </>
    );
  }

  function renderBilling() {
    if (
      !canRead(
        "claims",
        ["admin", "billing", "executive", "clinical_lead"].includes(role),
      )
    )
      return (
        <Panel>
          <Empty
            icon="shield"
            title="Financial records follow your role"
            description="Your current role does not include billing records. Clinical work remains connected to billing through sessions and signed documentation."
          />
        </Panel>
      );
    const rows = matching(data![billingTab as Resource]);
    const cols: Record<string, Column[]> = {
      claims: [
        { label: "Client / service", render: clientCell },
        {
          label: "Charge",
          render: (r) => <strong>{money(numeric(r, "amount_cents"))}</strong>,
        },
        { label: "Status", render: (r) => <Badge status={r.status} /> },
        {
          label: "Reference",
          render: (r) => value(r, "external_reference") || "Not submitted",
        },
        {
          label: "Actions",
          render: (r) => {
            const reviewable = r.status === "draft" || r.status === "ready";
            if (!reviewable && !billing) return null;
            return (
              <div className="row-actions">
                {reviewable && (
                  <Button
                    variant="text small"
                    onClick={() => setReadinessClaim(r)}
                  >
                    Review readiness
                  </Button>
                )}
                {billing && r.status === "ready" && (
                  <Button
                    variant="text small"
                    onClick={() =>
                      action(
                        `/api/claims/${r.id}/record-submission`,
                        "Record external submission",
                        "Record a claim you have already submitted through your approved billing channel. Nexus does not transmit it.",
                        [
                          {
                            key: "external_reference",
                            label: "External submission reference",
                            required: true,
                          },
                        ],
                        {},
                        "Record submission",
                      )
                    }
                  >
                    Record submission
                  </Button>
                )}
                {r.status === "submitted" && (
                  <Button
                    variant="text small"
                    onClick={() =>
                      action(
                        `/api/claims/${r.id}/deny`,
                        "Record claim denial",
                        "Capture the payer’s reason so the billing team can follow up.",
                        [
                          {
                            key: "reason",
                            label: "Denial reason",
                            type: "textarea",
                            required: true,
                          },
                        ],
                        {},
                        "Record denial",
                      )
                    }
                  >
                    Record denial
                  </Button>
                )}
                {["submitted", "denied", "partially_paid"].includes(
                  value(r, "status"),
                ) && (
                  <Button
                    variant="text small"
                    onClick={() => create("payments", { claim_id: r.id })}
                  >
                    Payment
                  </Button>
                )}
                {billing && (
                  <ClaimCorrections
                    claim={r}
                    action={action}
                    edit={() => edit("claims", r)}
                  />
                )}
              </div>
            );
          },
        },
      ],
      policies: [
        { label: "Client", render: clientCell },
        {
          label: "Payer",
          render: (r) => <strong>{value(r, "payer_name")}</strong>,
        },
        { label: "Member ID", render: (r) => value(r, "member_id") },
        { label: "Effective", render: (r) => date(r.effective_date) },
        { label: "Status", render: (r) => <Badge status={r.status} /> },
        {
          label: "",
          render: (r) =>
            billing ? (
              <Button variant="text small" onClick={() => edit("policies", r)}>
                Edit
              </Button>
            ) : null,
        },
      ],
      authorizations: [
        { label: "Client", render: clientCell },
        {
          label: "Authorization",
          render: (r) => (
            <div>
              <strong>{value(r, "authorization_number")}</strong>
              <small>Service {value(r, "service_code")}</small>
            </div>
          ),
        },
        {
          label: "Approved units",
          render: (r) => numeric(r, "units_approved"),
        },
        { label: "Valid through", render: (r) => date(r.ends_on) },
        { label: "Status", render: (r) => <Badge status={r.status} /> },
        {
          label: "",
          render: (r) =>
            billing ? (
              <Button
                variant="text small"
                onClick={() => edit("authorizations", r)}
              >
                Edit
              </Button>
            ) : null,
        },
      ],
      payments: [
        {
          label: "Claim",
          render: (r) => {
            const claim = data!.claims.find((c) => c.id === r.claim_id);
            return (
              <div>
                <strong>{clientName(client(claim?.client_id))}</strong>
                <small>
                  {value(claim, "service_code")} ·{" "}
                  {value(r, "claim_id").slice(0, 8)}
                </small>
              </div>
            );
          },
        },
        {
          label: "Amount",
          render: (r) => <strong>{money(numeric(r, "amount_cents"))}</strong>,
        },
        { label: "Received", render: (r) => date(r.received_on) },
        { label: "Source", render: (r) => label(value(r, "source")) },
        { label: "Reference", render: (r) => value(r, "reference") },
        {
          label: "Entry status",
          render: (r) => (
            <PaymentCorrection
              payment={r}
              reversal={data!.payment_reversals.find(
                (reversal) => reversal.payment_id === r.id,
              )}
              action={action}
              canWrite={can("payments", ["admin", "billing"].includes(role))}
            />
          ),
        },
      ],
    };
    return (
      <>
        <div className="metric-grid">
          <Metric
            icon="billing"
            title="Recorded charges"
            number={money(
              data!.claims
                .filter((c) => c.status !== "voided")
                .reduce((n, c) => n + numeric(c, "amount_cents"), 0),
            )}
            caption="Recorded charges, excluding voided claims"
          />
          <Metric
            icon="check"
            title="Cash received"
            number={money(
              metric.cash_received_cents ??
                data!.payments
                  .filter((p) => p.source !== "adjustment")
                  .reduce((n, p) => n + numeric(p, "amount_cents"), 0),
            )}
            caption="Net receipts; excludes adjustments"
            color="blue"
          />
          <Metric
            icon="clock"
            title="Outstanding"
            number={money(metric.outstanding_cents)}
            caption="Submitted balances less payments and adjustments"
            color="sand"
          />
          <Metric
            icon="activity"
            title="Ready for submission"
            number={data!.claims.filter((c) => c.status === "ready").length}
            caption="Validated internally, not transmitted"
            color="rose"
          />
        </div>
        <Tabs
          value={billingTab}
          onChange={(t) => {
            setBillingTab(t);
            setQuery("");
          }}
          tabs={[
            { id: "claims", label: "Claims", count: data!.claims.length },
            { id: "policies", label: "Coverage", count: data!.policies.length },
            {
              id: "authorizations",
              label: "Authorizations",
              count: data!.authorizations.length,
            },
            { id: "payments", label: "Payments", count: data!.payments.length },
          ]}
        />
        <Panel>
          {toolbar(`Search ${billingTab}…`)}
          <Table
            rows={rows}
            columns={cols[billingTab]}
            empty={
              <Empty
                icon="billing"
                title={
                  billingTab === "claims"
                    ? "Turn documented care into a clear next step"
                    : `Your ${billingTab} will appear here`
                }
                description={
                  billingTab === "claims"
                    ? "Start with coverage, a completed session, and a signed note. Nexus connects those records when checking claim readiness."
                    : "Add real records to connect the financial and clinical picture."
                }
                action={
                  billing ? (
                    <Button
                      variant="secondary"
                      onClick={() => create(billingTab as Resource)}
                    >
                      Add{" "}
                      {definitions[billingTab as Resource].title.toLowerCase()}
                    </Button>
                  ) : undefined
                }
              />
            }
          />
        </Panel>
        {billingTab === "payments" && (
          <ReversalHistory data={data!} reversals={data!.payment_reversals} />
        )}
        <ExportReports role={role} />
        <div className="info-strip">
          <Icon name="link" size={20} />
          <p>
            <strong>External submission stays explicit.</strong> Claim
            transmission, eligibility checks, and remittance import require a
            configured billing connection. Record activity performed through
            your approved channel here.
          </p>
        </div>
      </>
    );
  }

  function renderTasks() {
    return (
      <>
        <div className="section-summary">
          <div>
            <Icon name="tasks" />
            <strong>{dueTasks.length}</strong>
            <span>Open & in progress</span>
          </div>
          <div>
            <Icon name="clock" />
            <strong>
              {
                dueTasks.filter(
                  (t) =>
                    t.due_date &&
                    String(t.due_date) < new Date().toLocaleDateString("en-CA"),
                ).length
              }
            </strong>
            <span>Past due</span>
          </div>
          <div>
            <Icon name="check" />
            <strong>
              {data!.tasks.filter((t) => t.status === "done").length}
            </strong>
            <span>Completed</span>
          </div>
        </div>
        <Panel>
          {toolbar(
            "Search tasks…",
            <select
              aria-label="Filter tasks"
              className="compact-select"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
            >
              <option value="all">All tasks</option>
              <option value="open">Open</option>
              <option value="in_progress">In progress</option>
              <option value="done">Completed</option>
            </select>,
          )}
          <Table
            rows={matching(data!.tasks).filter(
              (t) => filter === "all" || t.status === filter,
            )}
            columns={taskColumns}
            empty={
              <Empty
                icon="tasks"
                title="Good handoffs begin with a clear next step"
                description="Create a task, connect a client when relevant, and give it an owner and due date."
                action={
                  <Button variant="secondary" onClick={() => create("tasks")}>
                    Add the first task
                  </Button>
                }
              />
            }
          />
        </Panel>
      </>
    );
  }

  function renderTeam() {
    return (
      <>
        <Tabs
          value={teamTab}
          onChange={setTeamTab}
          tabs={[
            { id: "members", label: "Team members", count: data!.staff.length },
            { id: "permissions", label: "Role overview" },
            {
              id: "exceptions",
              label: "Access exceptions",
              count: data!.exceptions.length,
            },
          ]}
        />
        {teamTab === "members" && (
          <Panel>
            {toolbar("Search your team…")}
            <Table
              rows={matching(data!.staff)}
              columns={[
                {
                  label: "Team member",
                  render: (r) => (
                    <Person name={value(r, "name")} sub={value(r, "email")} />
                  ),
                },
                { label: "Role", render: (r) => <Badge status={r.role} /> },
                {
                  label: "Facility",
                  render: (r) =>
                    value(facility(r.facility_id), "name") ||
                    "Organization-wide",
                },
                {
                  label: "Status",
                  render: (r) => (
                    <Badge status={r.active ? "active" : "inactive"} />
                  ),
                },
                {
                  label: "Actions",
                  render: (r) =>
                    admin ? (
                      <div className="row-actions">
                        <Button
                          variant="text small"
                          onClick={() => edit("staff", r)}
                        >
                          Edit
                        </Button>
                        {Boolean(r.active) && r.id !== data!.actor?.id && (
                          <Button
                            variant="text small danger"
                            onClick={() =>
                              action(
                                `/api/staff/${r.id}/offboard`,
                                "Offboard team member",
                                "Deactivate access and transfer open tasks, future sessions, and client assignments to another active team member.",
                                [
                                  {
                                    key: "transfer_to",
                                    label: "Transfer responsibilities to",
                                    type: "select",
                                    ref: "staff",
                                    required: true,
                                  },
                                ],
                                {},
                                "Transfer and offboard",
                              )
                            }
                          >
                            Offboard
                          </Button>
                        )}
                      </div>
                    ) : null,
                },
              ]}
            />
          </Panel>
        )}
        {teamTab === "permissions" && (
          <Panel
            title="Access follows responsibility"
            subtitle="The API enforces resource policies. Local operator selection is for development; sign-in is deferred."
          >
            <div className="role-cards">
              {[
                {
                  role: "Admin",
                  desc: "Organization setup, staff access, clinical and financial records, and exceptions.",
                  icon: "settings",
                },
                {
                  role: "Clinical lead",
                  desc: "Clinical coordination, documentation oversight, and shared form definitions.",
                  icon: "leaf",
                },
                {
                  role: "Clinician",
                  desc: "Client care, sessions, clinical notes, and follow-through.",
                  icon: "notes",
                },
                {
                  role: "Billing",
                  desc: "Coverage, authorizations, claims, and payments. Clinical narrative is not included.",
                  icon: "billing",
                },
                {
                  role: "Executive",
                  desc: "Organization visibility and financial summaries. Clinical narrative is not included.",
                  icon: "overview",
                },
                {
                  role: "Support",
                  desc: "Client coordination and operational follow-through with limited clinical access.",
                  icon: "clients",
                },
              ].map((r) => (
                <div key={r.role}>
                  <span className="role-icon">
                    <Icon name={r.icon as IconName} />
                  </span>
                  <h3>{r.role}</h3>
                  <p>{r.desc}</p>
                </div>
              ))}
            </div>
            <div className="quiet-note">
              <Icon name="shield" size={18} />
              <span>
                Temporary exceptions require an administrator, a documented
                reason, and an expiry. They do not replace an identity provider.
              </span>
            </div>
          </Panel>
        )}
        {teamTab === "exceptions" && (
          <Panel title="Time-bound access exceptions">
            <Table
              rows={data!.exceptions}
              columns={[
                {
                  label: "Team member",
                  render: (r) => value(staff(r.staff_id), "name"),
                },
                {
                  label: "Resource",
                  render: (r) => label(value(r, "resource")),
                },
                { label: "Reason", render: (r) => value(r, "reason") },
                { label: "Expires", render: (r) => date(r.expires_at, true) },
                {
                  label: "Status",
                  render: (r) => (
                    <Badge
                      status={
                        r.revoked
                          ? "revoked"
                          : new Date(value(r, "expires_at")) < new Date()
                            ? "expired"
                            : "active"
                      }
                    />
                  ),
                },
                {
                  label: "",
                  render: (r) =>
                    admin && !r.revoked ? (
                      <Button
                        variant="text small danger"
                        onClick={() =>
                          void quick(
                            `/api/exceptions/${r.id}`,
                            withOriginalUpdatedAt(r, { revoked: true }),
                          )
                        }
                      >
                        Revoke
                      </Button>
                    ) : null,
                },
              ]}
              empty={
                <Empty
                  icon="shield"
                  title="No access exceptions"
                  description="Keep access aligned with roles. When an exception is necessary, document why and when it should end."
                />
              }
            />
          </Panel>
        )}
      </>
    );
  }

  function renderSettings() {
    return (
      <>
        <Tabs
          value={settingsTab}
          onChange={setSettingsTab}
          tabs={[
            { id: "templates", label: "Forms & templates" },
            { id: "facilities", label: "Facilities" },
            { id: "integrations", label: "Connections" },
            { id: "audit", label: "Audit history" },
          ]}
        />
        {settingsTab === "templates" && (
          <Panel
            title="Your organization’s forms"
            subtitle="Author reusable prompts, then use them when documenting care."
          >
            {data!.templates.length ? (
              <div className="template-grid">
                {data!.templates.map((t) => (
                  <article className="template-card" key={t.id}>
                    <div>
                      <span className="template-icon">
                        <Icon name="notes" size={24} />
                      </span>
                      <Badge status={t.active ? "active" : "inactive"} />
                    </div>
                    <span className="eyebrow">{label(value(t, "kind"))}</span>
                    <h3>{value(t, "name")}</h3>
                    <p>
                      {value(t, "description") ||
                        "Custom form created by your organization."}
                    </p>
                    <p>
                      {label(value(t, "applies_to_stage") || "any")} stage ·{" "}
                      {label(value(t, "applies_to_care_level") || "any")} care
                      level ·{" "}
                      {t.required_for_stage
                        ? "Required by your organization"
                        : "Suggested"}
                    </p>
                    <div className="template-card-foot">
                      <span>
                        {Array.isArray(t.fields) ? t.fields.length : 0} fields
                      </span>
                      {canTemplates && (
                        <Button
                          variant="text small"
                          onClick={() => edit("templates", t)}
                        >
                          Edit template <Icon name="arrow" size={15} />
                        </Button>
                      )}
                    </div>
                  </article>
                ))}
              </div>
            ) : (
              <Empty
                icon="notes"
                title="Forms that follow your way of caring"
                description="Build an intake, progress, assessment, discharge, or consent form. Your clinical team defines the content; Nexus keeps it organized."
                action={
                  canTemplates ? (
                    <Button
                      variant="secondary"
                      onClick={() => create("templates")}
                    >
                      Create a form template
                    </Button>
                  ) : undefined
                }
              />
            )}
          </Panel>
        )}
        {settingsTab === "facilities" && (
          <Panel title="Care locations">
            <Table
              rows={data!.facilities}
              columns={[
                {
                  label: "Facility",
                  render: (r) => <strong>{value(r, "name")}</strong>,
                },
                { label: "Timezone", render: (r) => value(r, "timezone") },
                {
                  label: "Status",
                  render: (r) => (
                    <Badge status={r.active ? "active" : "inactive"} />
                  ),
                },
                {
                  label: "",
                  render: (r) =>
                    admin ? (
                      <Button
                        variant="text small"
                        onClick={() => edit("facilities", r)}
                      >
                        Edit location
                      </Button>
                    ) : null,
                },
              ]}
            />
          </Panel>
        )}
        {settingsTab === "integrations" && (
          <>
            <Panel
              title="A deliberately small set of connections"
              subtitle="Review local capabilities and the external services that still need setup."
            >
              <div className="integration-grid">
                {[
                  {
                    icon: "shield",
                    title: "Identity & single sign-on",
                    desc: "Connect your chosen identity provider before moving beyond local development. Authentication setup is deferred.",
                  },
                  {
                    icon: "billing",
                    title: "Claims & remittance",
                    desc: "Payer eligibility, electronic submission, and remittance require a validated billing interface. Internal claim readiness and manual payment entry are available.",
                  },
                  {
                    icon: "notes",
                    title: "Speech-to-text",
                    status: "optional_local",
                    desc: "Structured forms support local WAV transcription with recording consent and manual review. The optional recognizer must be installed on the API machine. Live session recording is not connected.",
                  },
                  {
                    icon: "link",
                    title: "Accounting & migrations",
                    desc: "Historical data import, exports, and accounting reconciliation need explicit field mapping and review. No automatic QuickBooks or Kipu synchronization is active.",
                  },
                ].map((i) => (
                  <article key={i.title}>
                    <span className="integration-icon">
                      <Icon name={i.icon as IconName} size={25} />
                    </span>
                    <Badge status={i.status || "not_connected"} />
                    <h3>{i.title}</h3>
                    <p>{i.desc}</p>
                  </article>
                ))}
              </div>
            </Panel>
            <div className="info-strip">
              <Icon name="shield" size={20} />
              <p>
                Keep real patient information out of this local prototype until
                identity, deployment, data handling, and the organization’s
                required controls have been validated.
              </p>
            </div>
          </>
        )}
        {settingsTab === "audit" && (
          <Panel
            title="Accountable by design"
            subtitle="A chronological record of workspace access and changes. Clinical narrative is not duplicated here."
          >
            <Table
              rows={[...data!.audit].reverse()}
              columns={[
                {
                  label: "When",
                  render: (r) => date(r.created_at || r.occurred_at, true),
                },
                {
                  label: "Actor",
                  render: (r) =>
                    value(staff(r.actor_id), "name") ||
                    value(r, "actor_name") ||
                    value(r, "actor_id") ||
                    "Setup",
                },
                { label: "Action", render: (r) => label(value(r, "action")) },
                {
                  label: "Resource",
                  render: (r) =>
                    label(value(r, "resource") || value(r, "entity_type")),
                },
                {
                  label: "Record",
                  render: (r) => (
                    <code>
                      {(value(r, "record_id") || value(r, "entity_id")).slice(
                        0,
                        12,
                      ) || "—"}
                    </code>
                  ),
                },
              ]}
              empty={
                <Empty
                  icon="activity"
                  title="Activity will leave a clear trail"
                  description="The server records permitted reads, writes, and workflow actions."
                />
              }
            />
          </Panel>
        )}
      </>
    );
  }

  return (
    <div className="app-shell" onKeyDown={(event) => {
      if (event.key === "Escape" && mobile) setMobile(false);
    }}>
      <aside id="workspace-navigation" className={`sidebar ${mobile ? "mobile-open" : ""}`}>
        <Brand />
        <div className="organization">
          <span className="org-symbol">{initials(orgName)}</span>
          <div>
            <strong>{orgName}</strong>
            <span>Recovery workspace</span>
          </div>
        </div>
        <nav aria-label="Primary navigation">
          {nav.map((item) => (
            <div key={item.id}>
              {item.section && (
                <div className="nav-section">{item.section}</div>
              )}
              <button
                className={`nav-item ${view === item.id ? "active" : ""}`}
                onClick={() => navigate(item.id)}
              >
                <Icon name={item.icon} size={19} />
                <span>{item.label}</span>
                {item.id === "tasks" && metric.open_tasks > 0 && (
                  <span className="nav-count">{metric.open_tasks}</span>
                )}
                {view === item.id && <span className="nav-active-dot" />}
              </button>
            </div>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="local-notice">
            <span className="status-dot" />
            <div>
              <strong>Local development</strong>
              <span>Authentication deferred</span>
            </div>
          </div>
          <div className="operator">
            <Person name={value(data.actor, "name")} sub={label(role)} small />
            <button
              className="button secondary small exit-workspace"
              title="Return to local operator selection"
              onClick={() => changeActor("")}
            >
              <Icon name="back" size={15} /> Exit workspace
            </button>
          </div>
        </div>
      </aside>
      {mobile && (
        <button
          className="sidebar-scrim"
          aria-label="Close navigation"
          onClick={() => setMobile(false)}
        />
      )}
      <div className="main-shell">
        <header className="topbar">
          <button
            className="icon-button mobile-toggle"
            aria-label="Open navigation"
            aria-expanded={mobile}
            aria-controls="workspace-navigation"
            onClick={() => setMobile(true)}
          >
            <Icon name="menu" />
          </button>
          <div className="breadcrumb">
            <span>Workspace</span>
            <Icon name="chevron" size={13} />
            <strong>{nav.find((n) => n.id === view)?.label}</strong>
          </div>
          <div className="topbar-right">
            <span className="today-label">
              <Icon name="sun" size={17} />
              {new Date().toLocaleDateString("en-US", {
                weekday: "short",
                month: "short",
                day: "numeric",
              })}
            </span>
            <span className="topbar-divider" />
            <ThemeControl />
            <button
              className="button secondary small refresh-workspace"
              aria-label="Refresh workspace"
              title="Refresh workspace"
              aria-busy={refreshing}
              disabled={refreshing}
              onClick={() => void refreshWorkspace()}
            >
              <Icon name="refresh" size={17} />
              <span>{refreshing ? "Refreshing…" : "Refresh"}</span>
            </button>
          </div>
        </header>
        <main>
          <div className="page-heading">
            <div>
              <span className="eyebrow">
                {view === "overview"
                  ? `WELCOME BACK, ${value(data.actor, "name").split(" ")[0].toUpperCase()}`
                  : view === "clients" && selectedClient
                    ? "CLIENT CHART"
                    : "NEXUS / " +
                      nav.find((n) => n.id === view)?.label.toUpperCase()}
              </span>
              <h1>
                {view === "clients" && selectedClient
                  ? "One person. A connected team."
                  : titles[view].title}
              </h1>
              <p>{titles[view].description}</p>
            </div>
            {view === "overview" ? (
              <Button variant="secondary" onClick={() => navigate("clients")}>
                <Icon name="clients" size={17} /> View clients
              </Button>
            ) : (
              primaryAction[view]
            )}
          </div>
          {error && (
            <div className="error-banner global" role="alert">
              <span>{error}</span>
              <button
                className="icon-button"
                aria-label="Dismiss error"
                onClick={() => setError("")}
              >
                <Icon name="close" size={16} />
              </button>
            </div>
          )}
          {view === "overview"
            ? renderOverview()
            : view === "clients"
              ? renderClients()
              : view === "schedule"
                ? renderSchedule()
                : view === "notes"
                  ? renderNotes()
                  : view === "billing"
                    ? renderBilling()
                    : view === "tasks"
                      ? renderTasks()
                      : view === "team"
                        ? renderTeam()
                        : renderSettings()}
          <footer className="workspace-footer">
            <span>
              <span className="footer-dot" /> Connected care, considered.
            </span>
            <span>Nexus · Local workspace</span>
          </footer>
        </main>
      </div>
      {toast && (
        <div className="toast" role="status">
          <span>
            <Icon name="check" size={16} />
          </span>
          {toast}
        </div>
      )}
      {readinessClaim && (
        <ClaimReadiness
          claim={readinessClaim}
          canWrite={can("claims", ["admin", "billing"].includes(role))}
          onClose={() => setReadinessClaim(null)}
          onMarkedReady={async () => {
            setReadinessClaim(null);
            await reload();
            setToast(
              "Marked internally ready. The claim has not been transmitted.",
            );
          }}
        />
      )}
      {dialog && (
        <Modal
          title={dialog.title}
          description={dialog.description}
          onClose={() => setDialog(null)}
          wide={
            dialog.resource === "notes" ||
            dialog.resource === "templates" ||
            dialog.kind === "viewnote"
          }
        >
          {dialog.kind === "note" ? (
            <NoteComposer
              data={data}
              initial={dialog.initial}
              onClose={() => setDialog(null)}
              onSave={(body) => mutate("/api/notes", body)}
            />
          ) : dialog.kind === "viewnote" && dialog.record ? (
            <>
              <div className="note-preview">
                <div className="note-meta">
                  <Badge status={dialog.record.status} />
                  <span>{clientName(client(dialog.record.client_id))}</span>
                  <span>{date(dialog.record.created_at, true)}</span>
                </div>
                {Boolean(dialog.record.amends_note_id) && (
                  <div className="quiet-note">
                    Amendment to record{" "}
                    {value(dialog.record, "amends_note_id").slice(0, 8)}
                  </div>
                )}
                <div className="note-body">
                  {value(dialog.record, "body") ||
                    "Clinical narrative is not available with your current role."}
                </div>
                {Boolean(dialog.record.signed_at) && (
                  <div className="signature">
                    <Icon name="shield" size={18} /> Signed{" "}
                    {date(dialog.record.signed_at, true)} · Original content
                    preserved
                  </div>
                )}
              </div>
              <div className="modal-foot">
                <Button variant="secondary" onClick={() => setDialog(null)}>
                  Close
                </Button>
                {clinical && dialog.record.status === "draft" && (
                  <>
                    <Button
                      variant="secondary"
                      onClick={() => edit("notes", dialog.record!)}
                    >
                      Edit draft
                    </Button>
                    <Button
                      disabled={
                        Boolean(dialog.record.session_id) &&
                        data.sessions.find(
                          (s) => s.id === dialog.record!.session_id,
                        )?.status !== "completed"
                      }
                      onClick={() =>
                        action(
                          `/api/notes/${dialog.record!.id}/sign`,
                          "Sign clinical note",
                          "Confirm that you have reviewed this documentation. The signed original cannot be edited; future corrections are recorded as amendments.",
                          [],
                          {},
                          "Sign note",
                        )
                      }
                    >
                      <Icon name="check" size={17} />{" "}
                      {dialog.record.session_id &&
                      data.sessions.find(
                        (s) => s.id === dialog.record!.session_id,
                      )?.status !== "completed"
                        ? "Complete session before signing"
                        : "Sign note"}
                    </Button>
                  </>
                )}
                {clinical && dialog.record.status === "signed" && (
                  <Button
                    onClick={() =>
                      action(
                        `/api/notes/${dialog.record!.id}/amend`,
                        "Amend clinical note",
                        "Preserve the signed original and create a new draft amendment for review.",
                        [
                          {
                            key: "reason",
                            label: "Reason for amendment",
                            type: "textarea",
                            required: true,
                          },
                          {
                            key: "body",
                            label: "Amended narrative",
                            type: "textarea",
                            required: true,
                          },
                        ],
                        { body: dialog.record!.body },
                        "Create draft amendment",
                      )
                    }
                  >
                    Create amendment
                  </Button>
                )}
              </div>
            </>
          ) : (
            <RecordForm
              key={`${dialog.resource || dialog.path}-${dialog.record?.id || "new"}`}
              resource={dialog.resource}
              data={data}
              initial={dialog.initial}
              fields={dialog.fields}
              submitLabel={dialog.submitLabel}
              onCancel={() => setDialog(null)}
              onSubmit={async (body) => {
                const patching =
                  (!dialog.path && Boolean(dialog.record)) ||
                  Boolean(dialog.path?.startsWith("/api/consents/"));
                const payload = patching
                  ? withOriginalUpdatedAt(dialog.record, body)
                  : body;
                try {
                  await mutate(
                    dialog.path ||
                      `/api/${dialog.resource}${dialog.record ? `/${dialog.record.id}` : ""}`,
                    payload,
                    patching ? "PATCH" : "POST",
                  );
                } catch (error) {
                  setError(
                    error instanceof Error
                      ? error.message
                      : "This change could not be saved.",
                  );
                  throw error;
                }
              }}
            />
          )}
        </Modal>
      )}
    </div>
  );
}
