import { Choice, FieldInput } from "./nexus-ui/FieldControls";
import { useEffect, useMemo, useRef, useState } from "react";
import type { Bootstrap, Row } from "./api";
import { clientName, date, label, value } from "./api";
import { NxIcon } from "./nexus-ui/Icon";
import { zoneLabel } from "./dateText";
import { readStored, writeStored } from "./safeStorage";
import { Avatar } from "./nexus-ui/Avatar";
import { Badge, Button, Modal } from "./ui";
import "./calendar.css";
import {
  ALL,
  addDays,
  dayAppointments,
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
  sessionLockReason,
  visibleParticipantNames,
  weekDates,
  weekdayIndex,
  validCalendarDay,
} from "./calendar.ts";
import type { Schedulable } from "./calendar.ts";
import { facilityWallTime, wallTimeCandidates, occurrenceLabel } from "./schedulingTime";

export type CalendarWorkspaceProps = {
  data: Bootstrap;
  canWrite: boolean;
  onCreateSession: (initial: Record<string, unknown>) => void;
  onCreateGroup: (initial: Record<string, unknown>) => void;
  onEditSession: (row: Row) => void;
  onOpenGroup: (row: Row) => void;
  onTelehealth: (kind:"sessions"|"groups",row:Row)=>void;
  onOpenClient: (row: Row) => void;
  onOpenTasks: (row?: Row) => void;
  focusSessionId?: string;
  startView?: Mode;
  initialDay?: string;
};

type Mode = "day" | "week" | "month" | "agenda";
type RichEvent = Schedulable & {
  key: string;
  row: Row;
  lockReason?: string;
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

function EventChip({ event, day, timeZone, onOpen }: {
  event: RichEvent; day: string; timeZone: string; onOpen: (event: RichEvent) => void;
}) {
  const when = sliceText(event.startsAt, event.endsAt, day, timeZone);
  const kind = event.source === "group" ? "Group" : label(event.type);
  const lock = event.lockReason ? ` Locked. ${event.lockReason}` : "";
  return <button type="button" className={`${serviceClass(event.type, event.source)} state-${event.status}`}
    aria-label={`${kind}, ${event.title}, ${when}. ${label(event.status)}.${lock} Open details`}
    title={event.lockReason} onClick={() => onOpen(event)}>
    <span className="cal-chip-kicker">{kind}{event.status && event.status !== "scheduled" && ` · ${label(event.status)}`}</span>
    <strong>{event.title}</strong>
    <small>{when}</small>
    {event.lockReason && <small className="cal-lock-label"><NxIcon name="lock" size={12}/> <span>Locked</span></small>}
  </button>;
}

export default function CalendarWorkspace({
  data,
  canWrite,
  onCreateSession,
  onCreateGroup,
  onEditSession,
  onOpenGroup,
  onTelehealth,
  onOpenClient,
  onOpenTasks,
  focusSessionId,
  startView = "week", initialDay,
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
  const [colorBy, setColorBy] = useState<"service" | "attendance">(() =>
    readStored("local","nexus.calendar.colorBy") === "attendance" ? "attendance" : "service"
  );
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
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const tick = window.setInterval(() => setNow(new Date()), 30000);
    return () => window.clearInterval(tick);
  }, []);
  const today = facilityYmd(now, timeZone);
  const [jumpNow, setJumpNow] = useState(0);
  const [anchor, setAnchor] = useState(today);
  const [selectedDay, setSelectedDay] = useState(today);
  const [jumpDate, setJumpDate] = useState(today);
  useEffect(() => setJumpDate(selectedDay), [selectedDay]);
  const [repeatedSlot, setRepeatedSlot] = useState<{kind:"session"|"group"; day:string; hour:number; minute:number; candidates:string[]} | null>(null);
  const [openKey, setOpenKey] = useState<string | null>(null);
  const dialogRef = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const nextFacility = defaultFacilityId(actorFacility, facilityChoices);
    setFacilityId(nextFacility);
    const nextZone = safeTimeZone(facilityChoices.find((facility) => facility.id === nextFacility)?.timezone);
    setDisplayTimeZone(nextZone);
    const nextToday = facilityYmd(new Date(), nextZone);
    setAnchor(initialDay || nextToday);
    setSelectedDay(initialDay || nextToday);
    // Reset only when the operator changes; facilityChoices is derived from that bootstrap.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [actorId,initialDay]);

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
    const signedNotes = new Set(data.notes.filter(note => note.status === "signed").map(note => note.session_id));
    const advancedClaims = new Set(data.claims.filter(claim => claim.status !== "draft").map(claim => claim.session_id));
    const sessions: RichEvent[] = (data.sessions ?? []).flatMap((row) => {
      const startsAt = value(row, "starts_at");
      const endsAt = endOf(row);
      if (!startsAt || !endsAt) return [];
      const client = clients.get(String(row.client_id ?? ""));
      const title = client ? names[client.id] || label(value(row, "kind")) || "Session" : label(value(row, "kind")) || "Session";
      return [{
        id: row.id,
        key: `session:${row.id}`,
        lockReason: row.edit_locked === true
          ? sessionLockReason(signedNotes.has(row.id), advancedClaims.has(row.id)) : undefined,
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
        title: value(row, "title") || "Group session",
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

  const range = useMemo(() => mode === "month" ? monthGrid(anchor) : mode === "day" ? [anchor] : weekDates(mode === "week" || mode === "agenda" ? anchor : selectedDay), [mode, anchor, selectedDay]);
  const visible = useMemo(
    () =>
      selectCalendarItems(events, {
        facilityId,
        clinicianId,
        type,
        status,
      }, { days: range, timeZone }),
    [events, facilityId, clinicianId, type, status, range, timeZone],
  );
  const ranged = visible;
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
    if (!validCalendarDay(nextAnchor) || !validCalendarDay(nextDay)) return;
    setAnchor(nextAnchor);
    setSelectedDay(nextDay);
  }
  function goToday() {
    const current = facilityYmd(new Date(), timeZone);
    setAnchor(current);
    setSelectedDay(current);
    setJumpDate(current);
    setJumpNow(tick => tick + 1);
  }
  function schedule(kind: "session" | "group", day = selectedDay, hour = 9, minute = 0, chosen?:string) {
    const wall = day + "T" + String(hour).padStart(2,"0") + ":" + String(minute).padStart(2,"0");
    const candidates = wallTimeCandidates(wall,timeZone);
    if (!candidates.length) return;
    if (candidates.length > 1 && !chosen) {
      setRepeatedSlot({kind,day,hour,minute,candidates});
      return;
    }
    const initial = createInitial({
      selectedInstant: chosen,
      kind,
      day,
      hour, minute,
      facilityId: createFacility === ALL ? "" : createFacility,
      clinicianId: clinicianId !== ALL ? clinicianId : schedulingClinician,
      timeZone,
    });
    if (!initial.facility_id) delete initial.facility_id;
    if (!initial.clinician_id) delete initial.clinician_id;
    if (kind === "group") onCreateGroup(initial);
    else onCreateSession(initial);
  }
  function openDay(day: string) {
    setSelectedDay(day); setAnchor(day); setMode("day");
  }
  const canAdd = !!(canWrite && data.permissions?.sessions?.write);
  function open(event: RichEvent) {
    setOpenKey(event.key);
  }

  const rangeLabel =
    mode === "day"
      ? formatYmdLong(anchor)
      : mode === "month"
        ? new Intl.DateTimeFormat("en-US", {month:"long", year:"numeric",timeZone:"UTC"}).format(new Date(`${anchor}T12:00:00Z`))
        : new Intl.DateTimeFormat("en-US", { month:"short", day:"numeric", year:"numeric", timeZone:"UTC" }).formatRange(new Date(range[0]+"T12:00Z"),new Date(range[range.length-1]+"T12:00Z"));
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
    <div className="calendar-workspace" data-calendar-root data-color-by={colorBy}>
      <header className="cal-toolbar">
        <div>
          <h2>{rangeLabel}</h2>
          <p className="cal-tz">
            Times are {zoneLabel(timeZone)}
            {facilityId === ALL ? " for all facilities" : selectedFacility ? ` · ${selectedFacility.name}` : ""}.

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
            <NxIcon name="chevron-left" size={16}/><span className="sr-only">Previous {mode}</span>
          </button>
          <button type="button" className="button secondary small" onClick={goToday}>Today</button>
          <button type="button" className="button secondary small" onClick={() => shift(1)}>
            <NxIcon name="chevron-right" size={16}/><span className="sr-only">Next {mode}</span>
          </button>
          <div>
            <FieldInput
              aria-label="Choose date"
              className="cal-date"
              type="date"
              value={jumpDate}
              onChange={(event) => {
                const next = event.target.value;
                setJumpDate(next);
                if (!validCalendarDay(next)) return;
                setSelectedDay(next);
                setAnchor(next);
              }}
            />
          </div>
          {canWrite && (
            <div className="cal-create">
              {data.permissions?.sessions?.write && <Button onClick={() => schedule("session")}><NxIcon name="add" size={16}/>Schedule session</Button>}
              {data.permissions?.groups?.write && <Button variant="secondary" onClick={() => schedule("group")}>Schedule group</Button>}
            </div>
          )}
        </div>
      </header>
      <details className="cal-options"><summary>Filters and display
        <span>{[facilityId !== ALL ? selectedFacility?.name : "All facilities", clinicianId !== ALL && value(data.staff.find(row => row.id === clinicianId), "name"), type !== ALL && label(type), status !== ALL && label(status)].filter(Boolean).join(" · ")}</span>
      </summary>
      <fieldset className="cal-filters">
        <legend className="sr-only">Filter schedule</legend>
        <label>
          Facility
          <Choice aria-label="Facility" value={facilityId} onChange={(event) => setFacilityId(event.target.value)}>
            <option value={ALL}>All facilities</option>
            {facilityChoices.map((facility) => (
              <option key={facility.id} value={facility.id}>
                {facility.name}{facility.active ? "" : " (inactive)"}
              </option>
            ))}
          </Choice>
        </label>
        <label>
          Clinician
          <Choice aria-label="Clinician" value={clinicianId} onChange={(event) => setClinicianId(event.target.value)}>
            <option value={ALL}>All clinicians</option>
            {clinicianOptions.map((staff) => (
              <option key={staff.id} value={staff.id}>{value(staff, "name") || "Unnamed team member"}</option>
            ))}
          </Choice>
        </label>
        <label>
          Type
          <Choice aria-label="Type" value={type} onChange={(event) => setType(event.target.value)}>
            <option value={ALL}>All types</option>
            {typeOptions.map((item) => <option key={item} value={item}>{label(item)}</option>)}
          </Choice>
        </label>
        <label>
          Status
          <Choice aria-label="Status" value={status} onChange={(event) => setStatus(event.target.value)}>
            <option value={ALL}>All statuses</option>
            {statusOptions.map((item) => <option key={item} value={item}>{label(item)}</option>)}
          </Choice>
        </label>
        {facilityId === ALL && zoneChoices.length > 0 && (
          <label>
            Display timezone
            <Choice aria-label="Display timezone" value={timeZone} onChange={(event) => setDisplayTimeZone(event.target.value)}>
              {zoneChoices.map((zone) => <option key={zone} value={zone}>{zone}</option>)}
            </Choice>
          </label>
        )}
        <label>Card color
          <Choice aria-label="Card color" value={colorBy} onChange={e => {
            const next = e.target.value === "attendance" ? "attendance" : "service";
            setColorBy(next);
            writeStored("local", "nexus.calendar.colorBy", next);
          }}><option value="service">Service type</option><option value="attendance">Attendance</option></Choice>
        </label>
      </fieldset></details>
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
                    <h4><button type="button" className="cal-day-link" onClick={()=>openDay(day)}>{formatYmdLong(day)}</button></h4>
                    {items.length ? items.map((event) => <EventChip key={event.key} event={event} day={day} timeZone={timeZone} onOpen={open} />) : <p className="cal-empty">Open</p>}
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
                      style={{gridColumnStart: weekdayIndex(day) + 1}}
                      className={`cal-cell ${day === today ? "is-today" : ""} ${day === selectedDay ? "is-selected" : ""} ${day.slice(0, 7) === anchor.slice(0, 7) ? "" : "is-outside"}`}
                    >
                      <button type="button" className="cal-month-space" aria-label={`${canAdd ? "Schedule session on" : "Open"} ${formatYmdLong(day)}`}
                        onClick={()=>canAdd ? schedule("session",day) : openDay(day)} />
                      <button
                        type="button"
                        className="cal-daynum"
                        aria-pressed={day === selectedDay}
                        aria-current={day === today ? "date" : undefined}
                        onClick={() => openDay(day)}
                      >
                        {Number(day.slice(8))}
                        <span className="sr-only">{formatYmdLong(day)}</span>
                      </button>
                      {items.slice(0, extra ? 2 : 3).map((event) => <EventChip key={event.key} event={event} day={day} timeZone={timeZone} onOpen={open} />)}
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
            <TimeGrid days={mode === "day" ? [anchor] : range} events={ranged} timeZone={timeZone} now={now}
              canAdd={canAdd} jumpNow={jumpNow} onDay={openDay} onOpen={open}
              onCreate={(day, minute) => schedule("session", day, Math.floor(minute/60), minute%60)} />
          )}
          <ul className="cal-legend">
            {[...new Set(ranged.map(event => colorBy === "attendance" ? event.status : event.type))].map(item =>
              <li key={item} className={colorBy === "attendance" ? "cal-chip state-" + item : serviceClass(item, "session")}>{label(item)}</li>
            )}
          </ul>
        </section>
      </div>
      {repeatedSlot && <Modal icon="clock" title="Which of the two times?" description={`This time occurs twice in ${timeZone} on ${formatYmdLong(repeatedSlot.day)}.`} onClose={()=>setRepeatedSlot(null)}>
        <div className="cal-dialog-actions">{repeatedSlot.candidates.map((instant,index) =>
          <Button key={instant} variant="secondary" onClick={()=>{
            const {kind,day,hour,minute} = repeatedSlot;
            setRepeatedSlot(null);
            schedule(kind,day,hour,minute,instant);
          }}>{occurrenceLabel(instant,timeZone,index)}</Button>)}</div>
      </Modal>}
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
            <header className={`cal-event-head state-${openEvent.status} ${serviceClass(openEvent.type,openEvent.source).split(" ").at(-1)}`}>
              <div className="cal-event-type"><span className="cal-service-dot"/>{label(openEvent.type)} {openEvent.source === "group" ? "group" : "session"}</div>
              <button type="button" className="icon-button cal-detail-close" aria-label="Close session" onClick={()=>setOpenKey(null)}><NxIcon name="close" size={20}/></button>
              <div className="cal-event-person">{openEvent.source === "group" ? <span className="group-avatar"><NxIcon name="group" size={28}/></span> :
                <Avatar patient={openEvent.participantRows[0]} id={String(openEvent.row.client_id || openEvent.id)} size="lg"/>}
                <div><h3 id="cal-dialog-title">{openEvent.title}</h3><Badge status={openEvent.status || "scheduled"}/></div></div>
            </header>
            <dl className="cal-facts">
              <dt>When</dt>
              <dd>
                {formatYmdLong(facilityYmd(new Date(openEvent.startsAt), timeZone))} · {formatFacilityTime(new Date(openEvent.startsAt), timeZone)}–{formatFacilityTime(new Date(openEvent.endsAt), timeZone)} {zoneLabel(timeZone)}
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
                  <dt>Participants in your access</dt>
                  <dd>
                    {openEvent.participantNames.length ? openEvent.participantNames.join(", ") : "No participant names are available in your access."}
                    {openEvent.hiddenParticipants > 0 ? " Some participants are outside your chart access." : ""}
                  </dd>
                </>
              )}
            </dl>
            {openEvent.lockReason && <div className="cal-lock-notice">
              <strong><NxIcon name="lock" size={16}/> Session locked</strong>
              <p>{openEvent.lockReason}</p>
            </div>}
            <div className="cal-dialog-actions">
              {["admin","clinical_lead","clinician"].includes(value(data.actor,"role")) && (value(data.actor,"role")!=="clinician"||openEvent.clinicianId===data.actor?.id) && <Button onClick={()=>{onTelehealth(openEvent.source==="group"||parentGroup?"groups":"sessions",parentGroup||openEvent.row);setOpenKey(null);}}>{openEvent.status==="scheduled"?"Open telehealth":"Visit records"}</Button>}
              {openEvent.source === "session" && !openEvent.row.group_id && !parentGroup && !openEvent.lockReason && canWrite && data.permissions?.sessions?.write && (
                <Button variant="secondary" onClick={() => { onEditSession(openEvent.row); setOpenKey(null); }}>Edit session</Button>
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

function clockLabel(minute: number) {
  const hour = Math.floor(minute/60);
  return `${hour % 12 || 12}:${String(minute%60).padStart(2,"0")} ${hour < 12 ? "AM" : "PM"}`;
}

function TimeGrid({ days, events, timeZone, now, canAdd, jumpNow, onDay, onCreate, onOpen }: {
  days: string[]; events: RichEvent[]; timeZone: string; now: Date; canAdd: boolean; jumpNow: number;
  onDay: (day: string)=>void; onCreate: (day: string, minute: number)=>void; onOpen: (event: RichEvent)=>void;
}) {
  const scroll = useRef<HTMLDivElement>(null);
  const [cursor,setCursor] = useState<{day:string; minute:number} | null>(null);
  const today = facilityYmd(now,timeZone);
  const wall = facilityWallTime(now.toISOString(),timeZone);
  const minuteNow = +wall.slice(11,13)*60 + +wall.slice(14,16);
  const firstDay = days[0];
  const dayCount = days.length;
  useEffect(() => {
    const view = scroll.current;
    if (!view) return;
    const hourSize = parseFloat(getComputedStyle(view).getPropertyValue("--cal-hour-size"));
    const start = jumpNow && days.includes(today) ? Math.max(0,minuteNow/60-1.5) : 7;
    view.scrollTop = Math.max(0, start * hourSize);
  },[firstDay, dayCount, jumpNow]);
  const layout = useMemo(()=>days.map(day=>dayAppointments(events,day,timeZone)),[events,firstDay,dayCount,timeZone]);
  const available = (day: string, minute: number) => wallTimeCandidates(
    day+"T"+String(Math.floor(minute/60)).padStart(2,"0")+":"+String(minute%60).padStart(2,"0"),timeZone).length>0;
  function minuteAt(e: React.PointerEvent<HTMLButtonElement> | React.MouseEvent<HTMLButtonElement>) {
    const r=e.currentTarget.getBoundingClientRect();
    return Math.max(0,Math.min(1410, Math.floor((e.clientY-r.top)/r.height*48)*30));
  }
  return <div className="time-scroll" ref={scroll}>
    <div className={`time-board ${days.length===1?"is-day":""}`} style={{gridTemplateColumns:`54px repeat(${days.length},minmax(0,1fr))`}}>
      <div className="time-corner"><NxIcon name="clock" size={15}/></div>
      {days.map(day=><button type="button" key={day} className="time-date" aria-label={formatYmdLong(day)} aria-current={day===today?"date":undefined}
        onClick={()=>onDay(day)}><span>{new Intl.DateTimeFormat("en",{weekday:"short",timeZone:"UTC"}).format(new Date(day+"T12:00Z"))}</span><strong>{+day.slice(8)}</strong></button>)}
      <div className="time-rail">{HOURS.map(h=><span key={h}>{h%12||12} {h<12?"AM":"PM"}</span>)}</div>
      {days.map((day,index)=>{
        const here = cursor?.day===day;
        const minute = here ? cursor.minute : 540;
        const valid = here && available(day,minute);
        return <div className="time-column" key={day} data-today={day===today}>
          {canAdd && <button type="button" className="time-create" aria-label={`Schedule session on ${formatYmdLong(day)} at ${clockLabel(minute)}. Use up and down arrows to choose a time.`}
            onFocus={()=>setCursor(prev=>prev?.day===day?prev:{day,minute:540})}
            onBlur={()=>setCursor(null)} onPointerLeave={()=>setCursor(null)}
            onPointerMove={e=>{const n=minuteAt(e); if(!here || minute!==n)setCursor({day,minute:n});}}
            onKeyDown={e=>{
              if(e.key==="ArrowUp"||e.key==="ArrowDown"){e.preventDefault();const n=Math.min(1410,Math.max(0,minute+(e.key==="ArrowUp"?-30:30)));setCursor({day,minute:n});
                const view=scroll.current;if(view){const y=n/1440*e.currentTarget.offsetHeight;if(y<view.scrollTop || y>view.scrollTop+view.clientHeight-100)view.scrollTop=Math.max(0,y-100);}}
            }}
            onClick={e=>{const n=e.detail ? minuteAt(e) : minute;if(available(day,n))onCreate(day,n);}}>
            {here && <span className="time-preview" style={{top:`${minute/1440*100}%`}}>{valid ? "+ "+clockLabel(minute) : "Time unavailable"}</span>}
          </button>}
          {layout[index].map(({event,start,end,lane,lanes})=><div key={event.key} className="time-event" data-short={end-start<44}
            style={{top:`${start/1440*100}%`,height:`${(end-start)/1440*100}%`,left:`${lane/lanes*100}%`,width:`${100/lanes}%`}}>
            <EventChip event={event} day={day} timeZone={timeZone} onOpen={onOpen}/>
          </div>)}
          {day===today && <div className="time-now" aria-label={`Current time ${clockLabel(minuteNow)}`} style={{top:`${minuteNow/1440*100}%`}}><span>{clockLabel(minuteNow)}</span></div>}
        </div>;
      })}
    </div>
  </div>;
}
