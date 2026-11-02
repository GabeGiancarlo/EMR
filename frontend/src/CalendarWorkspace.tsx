import { useEffect, useMemo, useRef, useState } from "react";
import type { Bootstrap, Row } from "./api";
import { clientName, date, label, value } from "./api";
import { Badge, Button } from "./ui";
import "./calendar.css";
import {
  ALL,
  addDays,
  addMonths,
  createInitial,
  crossesMidnight,
  defaultFacilityId,
  facilityYmd,
  formatFacilityTime,
  formatYmdLong,
  intervalOverlapsDay,
  monthGrid,
  safeTimeZone,
  selectCalendarItems,
  serviceClass,
  visibleParticipantNames,
  weekDates,
} from "./calendar.ts";
import type { Schedulable } from "./calendar.ts";
import { wallTimeCandidates } from "./schedulingTime";

export type CalendarWorkspaceProps = {
  data: Bootstrap;
  canWrite: boolean;
  onCreateSession: (initial: Record<string, unknown>) => void;
  onCreateGroup: (initial: Record<string, unknown>) => void;
  onEditSession: (row: Row) => void;
  onOpenGroup: (row: Row) => void;
  onOpenClient: (row: Row) => void;
  onOpenTasks: (row?: Row) => void;
  focusSessionId?: string;
  startView?: Mode;
};

type Mode = "day" | "week" | "month" | "agenda";
type RichEvent = Schedulable & {
  key: string;
  row: Row;
  title: string;
  location: string;
  participantNames: string[];
  hiddenParticipants: number;
  participantRows: Row[];
};

const HOURS = Array.from({ length: 24 }, (_, index) => index);
const MODES: Mode[] = ["day", "week", "month", "agenda"];

function endOf(row: Row): string {
  const end = value(row, "ends_at");
  if (end) return end;
  const start = new Date(value(row, "starts_at"));
  return Number.isNaN(start.getTime()) ? "" : new Date(start.getTime() + 3_600_000).toISOString();
}

function sliceText(startIso: string, endIso: string, day: string, timeZone: string): string {
  const startDay = facilityYmd(new Date(startIso), timeZone);
  const lastDay = facilityYmd(new Date(new Date(endIso).getTime() - 1), timeZone);
  const startLabel = formatFacilityTime(new Date(startIso), timeZone);
  const endLabel = formatFacilityTime(new Date(endIso), timeZone);
  if (startDay === day && lastDay === day) return `${startLabel}–${endLabel}`;
  if (startDay === day) return `${startLabel} onward`;
  if (lastDay === day) return `Until ${endLabel}`;
  return "All day";
}

export default function CalendarWorkspace({
  data,
  canWrite,
  onCreateSession,
  onCreateGroup,
  onEditSession,
  onOpenGroup,
  onOpenClient,
  onOpenTasks,
  focusSessionId,
  startView = "month",
}: CalendarWorkspaceProps) {
  const facilities = data.facilities ?? [];
  const facilityChoices = facilities.map((facility) => ({
    id: facility.id,
    active: facility.active !== false,
    name: value(facility, "name") || "Unnamed facility",
    timezone: value(facility, "timezone"),
  }));
  const actorId = data.actor?.id ?? "";
  const actorFacility = value(data.actor, "facility_id");
  const [facilityId, setFacilityId] = useState(() =>
    defaultFacilityId(actorFacility, facilityChoices),
  );
  const [clinicianId, setClinicianId] = useState(ALL);
  const [type, setType] = useState(ALL);
  const [status, setStatus] = useState(ALL);
  const [mode, setMode] = useState<Mode>(startView);
  const [displayTimeZone, setDisplayTimeZone] = useState(() =>
    safeTimeZone(facilityChoices.find((facility) => facility.id === defaultFacilityId(actorFacility, facilityChoices))?.timezone),
  );
  const selectedFacility = facilityChoices.find((facility) => facility.id === facilityId);
  const zoneChoices = [...new Set(facilityChoices.map((facility) => safeTimeZone(facility.timezone)))];
  const timeZone =
    facilityId === ALL
      ? safeTimeZone(zoneChoices.includes(displayTimeZone) ? displayTimeZone : zoneChoices[0])
      : safeTimeZone(selectedFacility?.timezone);
  const today = facilityYmd(new Date(), timeZone);
  const [anchor, setAnchor] = useState(today);
  const [selectedDay, setSelectedDay] = useState(today);
  const [openKey, setOpenKey] = useState<string | null>(null);
  const dialogRef = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const nextFacility = defaultFacilityId(actorFacility, facilityChoices);
    setFacilityId(nextFacility);
    const nextZone = safeTimeZone(facilityChoices.find((facility) => facility.id === nextFacility)?.timezone);
    setDisplayTimeZone(nextZone);
    const nextToday = facilityYmd(new Date(), nextZone);
    setAnchor(nextToday);
    setSelectedDay(nextToday);
    // Reset only when the operator changes; facilityChoices is derived from that bootstrap.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [actorId]);

  const events = useMemo(() => {
    const names: Record<string, string> = {};
    const clients = new Map(data.clients.map((client) => [client.id, client]));
    for (const client of data.clients) {
      const name = clientName(client);
      if (name && name !== "Client unavailable") names[client.id] = name;
    }
    const links = new Map<string, string[]>();
    for (const session of data.sessions ?? []) {
      const group = value(session, "group_id");
      if (group) links.set(group, [...(links.get(group) ?? []), session.id]);
    }
    const attendanceByGroup = new Map<string, Row[]>();
    for (const row of data.group_attendance ?? []) {
      const groupId = value(row, "group_id");
      const sessionId = value(row, "session_id");
      if (groupId && sessionId) {
        const list = links.get(groupId) ?? [];
        list.push(sessionId);
        links.set(groupId, list);
      }
      if (groupId) {
        const list = attendanceByGroup.get(groupId) ?? [];
        list.push(row);
        attendanceByGroup.set(groupId, list);
      }
    }
    const sessionFacility = (row: Row) => {
      const episode = data.episodes.find((item) => item.id === row.episode_id);
      if (episode && value(episode, "facility_id")) return value(episode, "facility_id");
      const client = clients.get(String(row.client_id ?? ""));
      return client ? value(client, "facility_id") : "";
    };
    const sessions: RichEvent[] = (data.sessions ?? []).flatMap((row) => {
      const startsAt = value(row, "starts_at");
      const endsAt = endOf(row);
      if (!startsAt || !endsAt) return [];
      const client = clients.get(String(row.client_id ?? ""));
      const title = client ? names[client.id] || label(value(row, "kind")) || "Session" : label(value(row, "kind")) || "Session";
      return [{
        id: row.id,
        key: `session:${row.id}`,
        source: "session" as const,
        facilityId: sessionFacility(row),
        clinicianId: value(row, "clinician_id"),
        type: value(row, "kind") || "session",
        status: value(row, "status"),
        startsAt,
        endsAt,
        linkedSessionIds: [],
        row,
        title,
        location: value(row, "location"),
        participantNames: client && names[client.id] ? [names[client.id]] : [],
        hiddenParticipants: client ? 0 : 1,
        participantRows: client ? [client] : [],
      }];
    });
    const groups: RichEvent[] = (data.groups ?? []).flatMap((row) => {
      const startsAt = value(row, "starts_at");
      const endsAt = endOf(row);
      if (!startsAt || !endsAt) return [];
      const attendance = attendanceByGroup.get(row.id) ?? [];
      const disclosure = visibleParticipantNames(
        attendance.map((item) => value(item, "client_id")).filter(Boolean),
        names,
      );
      const participantRows = attendance
        .map((item) => clients.get(value(item, "client_id")))
        .filter((item): item is Row => Boolean(item));
      return [{
        id: row.id,
        key: `group:${row.id}`,
        source: "group" as const,
        facilityId: value(row, "facility_id"),
        clinicianId: value(row, "clinician_id"),
        type: value(row, "kind") || "group",
        status: value(row, "status"),
        startsAt,
        endsAt,
        linkedSessionIds: links.get(row.id) ?? [],
        row,
        title: value(row, "title") || "Group encounter",
        location: value(row, "location"),
        participantNames: disclosure.names,
        hiddenParticipants: disclosure.hiddenCount,
        participantRows,
      }];
    });
    return [...groups, ...sessions];
  }, [data]);

  useEffect(() => {
    if (!focusSessionId) return;
    const session = events.find(event => event.source === "session" && event.id === focusSessionId);
    if (!session) return;
    const group = events.find(event => event.source === "group" && event.linkedSessionIds?.includes(session.id));
    const event = group || session;
    setFacilityId(event.facilityId || ALL); setClinicianId(ALL); setType(ALL); setStatus(ALL);
    const zone = safeTimeZone(facilityChoices.find(facility => facility.id === event.facilityId)?.timezone);
    const day = facilityYmd(new Date(event.startsAt), zone);
    setAnchor(day); setSelectedDay(day); setOpenKey(event.key);
  }, [focusSessionId]);

  const visible = useMemo(
    () =>
      selectCalendarItems(events, {
        facilityId,
        clinicianId,
        type,
        status,
      }),
    [events, facilityId, clinicianId, type, status],
  );
  const range = mode === "month" ? monthGrid(anchor) : mode === "day" ? [anchor] : weekDates(mode === "week" || mode === "agenda" ? anchor : selectedDay);
  const ranged = visible.filter((event) =>
    range.some((day) => intervalOverlapsDay(event.startsAt, event.endsAt, day, timeZone)),
  );
  const openEvent = visible.find((event) => event.key === openKey) ?? null;

  useEffect(() => {
    if (openKey && !openEvent) setOpenKey(null);
  }, [openKey, openEvent]);
  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    if (openEvent && !dialog.open) dialog.showModal();
    if (!openEvent && dialog.open) dialog.close();
  }, [openEvent]);

  const clinicianOptions = data.staff.filter((staff) => (staff.active !== false && ["admin", "clinical_lead", "clinician"].includes(value(staff, "role"))) || events.some((event) => event.clinicianId === staff.id));
  const typeOptions = [...new Set(events.map((event) => event.type).filter(Boolean))].sort();
  const statusOptions = [...new Set(events.map((event) => event.status).filter(Boolean))].sort();
  const createFacility = facilityId !== ALL ? facilityId : defaultFacilityId(actorFacility, facilityChoices);
  const schedulingClinician = ["clinician", "clinical_lead", "admin"].includes(value(data.actor, "role"))
    ? actorId
    : "";

  function shift(direction: number) {
    const nextAnchor = mode === "month" ? addMonths(anchor, direction) : addDays(anchor, mode === "day" ? direction : direction * 7);
    const nextDay = mode === "month" ? addMonths(selectedDay, direction) : addDays(selectedDay, mode === "day" ? direction : direction * 7);
    setAnchor(nextAnchor);
    setSelectedDay(nextDay);
  }
  function goToday() {
    const current = facilityYmd(new Date(), timeZone);
    setAnchor(current);
    setSelectedDay(current);
  }
  function schedule(kind: "session" | "group", day = selectedDay, hour = 9) {
    const initial = createInitial({
      kind,
      day,
      hour,
      facilityId: createFacility === ALL ? "" : createFacility,
      clinicianId: clinicianId !== ALL ? clinicianId : schedulingClinician,
      timeZone: safeTimeZone(facilityChoices.find(facility => facility.id === createFacility)?.timezone),
    });
    if (!initial.facility_id) delete initial.facility_id;
    if (!initial.clinician_id) delete initial.clinician_id;
    if (kind === "group") onCreateGroup(initial);
    else onCreateSession(initial);
  }
  function open(event: RichEvent) {
    setOpenKey(event.key);
  }

  const rangeLabel =
    mode === "day"
      ? formatYmdLong(anchor)
      : mode === "month"
        ? new Intl.DateTimeFormat("en-US", {month:"long", year:"numeric",timeZone:"UTC"}).format(new Date(`${anchor}T12:00:00Z`))
        : `${formatYmdLong(range[0])} – ${formatYmdLong(range[range.length - 1])}`;
  const tasks = (data.tasks ?? [])
    .filter((task) => value(task, "status") !== "done")
    .filter((task) => {
      if (clinicianId !== ALL && value(task, "assignee_id") !== clinicianId) return false;
      if (facilityId === ALL) return true;
      const client = data.clients.find((item) => item.id === task.client_id);
      const assignee = data.staff.find((item) => item.id === task.assignee_id);
      const taskFacility = client ? value(client, "facility_id") : value(assignee, "facility_id");
      return !taskFacility || taskFacility === facilityId;
    })
    .sort((a, b) => value(a, "due_date").localeCompare(value(b, "due_date")));

  function Chip({ event, day }: { event: RichEvent; day: string }) {
    const when = sliceText(event.startsAt, event.endsAt, day, timeZone);
    const kicker = event.source === "group" ? "Group" : label(event.type);
    return (
      <button
        type="button"
        className={serviceClass(event.type, event.source)}
        aria-label={`${kicker}, ${event.title}, ${when}. Open details`}
        onClick={() => open(event)}
      >
        <span className="cal-chip-kicker">{kicker}</span>
        <strong>{event.title}</strong>
        <small>{when}</small>
      </button>
    );
  }

  function dayEvents(day: string) {
    return ranged.filter((event) => intervalOverlapsDay(event.startsAt, event.endsAt, day, timeZone));
  }

  const parentGroup = openEvent?.source === "session"
    ? data.groups.find((group) =>
        (data.group_attendance ?? []).some(
          (row) => row.group_id === group.id && value(row, "session_id") === openEvent.id,
        ),
      )
    : undefined;

  return (
    <div className="calendar-workspace" data-calendar-root>
      <header className="cal-toolbar">
        <div>
          <span className="cal-kicker">CLINICIAN SCHEDULE</span>
          <h2>{rangeLabel}</h2>
          <p className="cal-tz">
            Times shown in {timeZone}
            {facilityId === ALL ? " for all facilities" : selectedFacility ? ` · ${selectedFacility.name}` : ""}.
            Scheduling date {formatYmdLong(selectedDay)}.
          </p>
          <div className="cal-views" role="group" aria-label="Calendar view">
            {MODES.map((item) => (
              <button
                type="button"
                className="button secondary small"
                key={item}
                aria-pressed={mode === item}
                onClick={() => { setMode(item); setAnchor(selectedDay); }}
              >
                {label(item)}
              </button>
            ))}
          </div>
        </div>
        <div className="cal-nav">
          <button type="button" className="button secondary small" onClick={() => shift(-1)}>
            Previous <span className="sr-only">{mode}</span>
          </button>
          <button type="button" className="button secondary small" onClick={goToday}>Today</button>
          <button type="button" className="button secondary small" onClick={() => shift(1)}>
            Next <span className="sr-only">{mode}</span>
          </button>
          <label>
            <span className="sr-only">Choose date</span>
            <input
              className="cal-date"
              type="date"
              value={selectedDay}
              onChange={(event) => {
                const next = event.target.value;
                if (!/^\d{4}-\d{2}-\d{2}$/.test(next)) return;
                setSelectedDay(next);
                setAnchor(next);
              }}
            />
          </label>
          {canWrite && (
            <div className="cal-create">
              {data.permissions?.sessions?.write && <Button onClick={() => schedule("session")}>Schedule session</Button>}
              {data.permissions?.groups?.write && <Button variant="secondary" onClick={() => schedule("group")}>Schedule group</Button>}
            </div>
          )}
        </div>
      </header>
      <fieldset className="cal-filters">
        <legend className="sr-only">Filter schedule</legend>
        <label>
          Facility
          <select aria-label="Facility" value={facilityId} onChange={(event) => setFacilityId(event.target.value)}>
            <option value={ALL}>All facilities</option>
            {facilityChoices.map((facility) => (
              <option key={facility.id} value={facility.id}>
                {facility.name}{facility.active ? "" : " (inactive)"}
              </option>
            ))}
          </select>
        </label>
        <label>
          Clinician
          <select aria-label="Clinician" value={clinicianId} onChange={(event) => setClinicianId(event.target.value)}>
            <option value={ALL}>All clinicians</option>
            {clinicianOptions.map((staff) => (
              <option key={staff.id} value={staff.id}>{value(staff, "name") || "Unnamed team member"}</option>
            ))}
          </select>
        </label>
        <label>
          Type
          <select aria-label="Type" value={type} onChange={(event) => setType(event.target.value)}>
            <option value={ALL}>All types</option>
            {typeOptions.map((item) => <option key={item} value={item}>{label(item)}</option>)}
          </select>
        </label>
        <label>
          Status
          <select aria-label="Status" value={status} onChange={(event) => setStatus(event.target.value)}>
            <option value={ALL}>All statuses</option>
            {statusOptions.map((item) => <option key={item} value={item}>{label(item)}</option>)}
          </select>
        </label>
        {facilityId === ALL && zoneChoices.length > 0 && (
          <label>
            Display timezone
            <select aria-label="Display timezone" value={timeZone} onChange={(event) => setDisplayTimeZone(event.target.value)}>
              {zoneChoices.map((zone) => <option key={zone} value={zone}>{zone}</option>)}
            </select>
          </label>
        )}
      </fieldset>
      <div className="cal-layout">
        <aside className="cal-tasks" aria-label="Task queue">
          <h3>Task queue</h3>
          <p>Follow-through for the selected facility and clinician.</p>
          {tasks.length ? (
            <div className="cal-task-list">
              {tasks.slice(0, 8).map((task) => (
                <button type="button" className="cal-task" key={task.id} onClick={() => onOpenTasks(task)}>
                  <span className={`cal-priority ${value(task, "priority") || "normal"}`} aria-hidden="true" />
                  <span>
                    <strong>{value(task, "title") || "Untitled task"}</strong>
                    <small>{task.due_date ? date(task.due_date) : "No due date"} · {label(value(task, "status") || "open")}</small>
                  </span>
                </button>
              ))}
            </div>
          ) : (
            <p className="cal-empty">No open tasks in this view. The calendar stays available.</p>
          )}
          <Button variant="secondary small" onClick={() => onOpenTasks()}>Open tasks</Button>
        </aside>
        <section className="cal-board" aria-label="Calendar">
          {!ranged.length && (
            <p className="cal-empty">No sessions or groups in this range. Move the date or clear a filter to keep looking.</p>
          )}
          {mode === "agenda" ? (
            <div className="cal-agenda">
              {range.map((day) => {
                const items = dayEvents(day);
                return (
                  <article key={day}>
                    <h4>{formatYmdLong(day)}</h4>
                    {items.length ? items.map((event) => <Chip key={event.key} event={event} day={day} />) : <p className="cal-empty">Open</p>}
                  </article>
                );
              })}
            </div>
          ) : mode === "month" ? (
            <>
              <div className="cal-dow" aria-hidden="true">
                {["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"].map((day) => <span key={day}>{day}</span>)}
              </div>
              <div className="cal-month" role="group" aria-label="Month dates">
                {range.map((day) => {
                  const items = dayEvents(day);
                  const extra = Math.max(0, items.length - 3);
                  return (
                    <div
                      key={day}
                      className={`cal-cell ${day === today ? "is-today" : ""} ${day === selectedDay ? "is-selected" : ""} ${day.slice(0, 7) === anchor.slice(0, 7) ? "" : "is-outside"}`}
                    >
                      <button
                        type="button"
                        className="cal-daynum"
                        aria-pressed={day === selectedDay}
                        aria-current={day === today ? "date" : undefined}
                        onClick={() => setSelectedDay(day)}
                      >
                        {Number(day.slice(8))}
                        <span className="sr-only">{formatYmdLong(day)}</span>
                      </button>
                      {items.slice(0, extra ? 2 : 3).map((event) => <Chip key={event.key} event={event} day={day} />)}
                      {extra > 0 && (
                        <button
                          type="button"
                          className="cal-more"
                          aria-label={`Show all ${items.length} events on ${formatYmdLong(day)}`}
                          onClick={() => {
                            setSelectedDay(day);
                            setAnchor(day);
                            setMode("day");
                          }}
                        >
                          +{items.length - 2} more
                        </button>
                      )}
                    </div>
                  );
                })}
              </div>
            </>
          ) : (
            <div className="cal-scroll">
              <div className={mode === "day" ? "cal-day-grid" : "cal-week"}>
                <span />
                {(mode === "day" ? [anchor] : range).map((day) => (
                  <button
                    type="button"
                    key={day}
                    className="cal-daynum"
                    aria-pressed={day === selectedDay}
                    onClick={() => {
                      setSelectedDay(day);
                      setAnchor(day);
                    }}
                  >
                    {formatYmdLong(day)}
                  </button>
                ))}
                <span className="cal-hour">Span</span>
                {(mode === "day" ? [anchor] : range).map((day) => (
                  <div key={`${day}-span`} className="cal-allday">
                    {dayEvents(day)
                      .filter((event) => crossesMidnight(event.startsAt, event.endsAt, timeZone) || facilityYmd(new Date(event.startsAt), timeZone) !== day)
                      .map((event) => <Chip key={event.key} event={event} day={day} />)}
                  </div>
                ))}
                {HOURS.map((hour) => (
                  <HourRow
                    key={hour}
                    hour={hour}
                    days={mode === "day" ? [anchor] : range}
                    events={ranged}
                    timeZone={timeZone}
                    canWrite={canWrite && data.permissions?.sessions?.write === true}
                    onAdd={(day) => schedule("session", day, hour)}
                    onOpen={open}
                  />
                ))}
              </div>
            </div>
          )}
          <ul className="cal-legend">
            {[...new Set(ranged.map((event) => event.type))].map((item) => (
              <li key={item} className={serviceClass(item, "session")}>{label(item)}</li>
            ))}
          </ul>
        </section>
      </div>
      <dialog
        ref={dialogRef}
        className="cal-dialog"
        aria-labelledby="cal-dialog-title"
        onCancel={() => setOpenKey(null)}
        onClick={(event) => {
          if (event.target === dialogRef.current) setOpenKey(null);
        }}
      >
        {openEvent && (
          <div className="cal-dialog-body">
            <span className="cal-kicker">{openEvent.source === "group" ? "GROUP" : "SESSION"}</span>
            <h3 id="cal-dialog-title">{openEvent.title}</h3>
            <Badge status={openEvent.status || "scheduled"} />
            <dl className="cal-facts">
              <dt>When</dt>
              <dd>
                {formatYmdLong(facilityYmd(new Date(openEvent.startsAt), timeZone))} · {formatFacilityTime(new Date(openEvent.startsAt), timeZone)}–{formatFacilityTime(new Date(openEvent.endsAt), timeZone)} {timeZone}
                {crossesMidnight(openEvent.startsAt, openEvent.endsAt, timeZone) ? ". Continues across midnight." : ""}
              </dd>
              <dt>Type</dt>
              <dd>{label(openEvent.type)}</dd>
              <dt>Facility</dt>
              <dd>{facilityChoices.find((facility) => facility.id === openEvent.facilityId)?.name || "Not in your access"}</dd>
              <dt>Clinician</dt>
              <dd>{value(data.staff.find((staff) => staff.id === openEvent.clinicianId), "name") || "Not in your access"}</dd>
              {openEvent.location && (
                <>
                  <dt>Location</dt>
                  <dd>{openEvent.location}</dd>
                </>
              )}
              {openEvent.source === "group" && (
                <>
                  <dt>Participants</dt>
                  <dd>
                    {openEvent.participantNames.length ? openEvent.participantNames.join(", ") : "No participant names are available in your access."}
                    {openEvent.hiddenParticipants > 0 ? " Some participants are outside your chart access." : ""}
                  </dd>
                </>
              )}
            </dl>
            <div className="cal-dialog-actions">
              {openEvent.source === "session" && !parentGroup && canWrite && data.permissions?.sessions?.write && (
                <Button onClick={() => { onEditSession(openEvent.row); setOpenKey(null); }}>Edit session</Button>
              )}
              {openEvent.source === "session" && openEvent.participantRows[0] && (
                <Button variant="secondary" onClick={() => { onOpenClient(openEvent.participantRows[0]); setOpenKey(null); }}>Open chart</Button>
              )}
              {openEvent.source === "group" && (
                <Button onClick={() => { onOpenGroup(openEvent.row); setOpenKey(null); }}>Open group</Button>
              )}
              {parentGroup && (
                <Button variant="secondary" onClick={() => { onOpenGroup(parentGroup); setOpenKey(null); }}>Open group</Button>
              )}
              {openEvent.source === "group" && openEvent.participantRows.map((client) => (
                <Button variant="secondary" key={client.id} onClick={() => { onOpenClient(client); setOpenKey(null); }}>
                  Open chart for {clientName(client)}
                </Button>
              ))}
              <Button variant="secondary" onClick={() => setOpenKey(null)}>Close</Button>
            </div>
          </div>
        )}
      </dialog>
    </div>
  );
}

function HourRow({
  hour,
  days,
  events,
  timeZone,
  canWrite,
  onAdd,
  onOpen,
}: {
  hour: number;
  days: string[];
  events: RichEvent[];
  timeZone: string;
  canWrite: boolean;
  onAdd: (day: string) => void;
  onOpen: (event: RichEvent) => void;
}) {
  const labelText = new Intl.DateTimeFormat("en-US", { hour: "numeric", timeZone: "UTC" }).format(new Date(Date.UTC(2026, 0, 1, hour)));
  return (
    <>
      <span className="cal-hour">{labelText}</span>
      {days.map((day) => {
        const items = events.filter((event) => {
          if (crossesMidnight(event.startsAt, event.endsAt, timeZone)) return false;
          if (facilityYmd(new Date(event.startsAt), timeZone) !== day) return false;
          const startHour = Number(new Intl.DateTimeFormat("en-US", {
            timeZone,
            hour: "2-digit",
            hourCycle: "h23",
          }).format(new Date(event.startsAt)));
          return startHour === hour;
        });
        return (
          <div key={`${day}-${hour}`} className="cal-slot">
            {canWrite && (
              <button type="button" className="cal-slot-add" disabled={!wallTimeCandidates(`${day}T${String(hour).padStart(2,"0")}:00`,timeZone).length} aria-label={`Schedule session on ${formatYmdLong(day)} at ${labelText}`} onClick={() => onAdd(day)}>
                Add
              </button>
            )}
            {items.map((event) => (
              <button
                type="button"
                key={event.key}
                className={serviceClass(event.type, event.source)}
                aria-label={`${event.source === "group" ? "Group" : label(event.type)}, ${event.title}, ${sliceText(event.startsAt, event.endsAt, day, timeZone)}. Open details`}
                onClick={() => onOpen(event)}
              >
                <strong>{event.title}</strong>
                <small>{sliceText(event.startsAt, event.endsAt, day, timeZone)}</small>
              </button>
            ))}
          </div>
        );
      })}
    </>
  );
}
