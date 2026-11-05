import { wallTimeCandidates } from "./schedulingTime.ts";
export const ALL = "all";

export const SERVICE_CLASS: Record<string, string> = {
  individual: "svc-individual",
  assessment: "svc-assessment",
  family: "svc-family",
  case_management: "svc-case",
  process: "svc-process",
  psychoeducation: "svc-education",
  skills: "svc-skills",
  support: "svc-support",
};

export type CalendarFilter = {
  facilityId: string;
  clinicianId: string;
  type: string;
  status: string;
};

export type Schedulable = {
  id: string;
  source: "session" | "group";
  facilityId: string;
  clinicianId: string;
  type: string;
  status: string;
  startsAt: string;
  endsAt: string;
  linkedSessionIds?: string[];
};

export type ParticipantDisclosure = {
  names: string[];
  hiddenCount: number;
};

const blankFilter = (value: string) => !value || value === ALL;

export function pad2(value: number): string {
  return String(Math.trunc(Math.abs(value))).padStart(2, "0");
}

export function formatYmd(year: number, month: number, day: number): string {
  return `${String(year).padStart(4, "0")}-${pad2(month)}-${pad2(day)}`;
}

export function parseYmd(ymd: string): { year: number; month: number; day: number } {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(ymd);
  if (!match) throw new Error(`Invalid date: ${ymd}`);
  const parts = { year: Number(match[1]), month: Number(match[2]), day: Number(match[3]) };
  if (parts.year < 100 || new Date(`${ymd}T12:00:00Z`).toISOString().slice(0, 10) !== ymd) throw new Error(`Invalid date: ${ymd}`);
  return parts;
}

export function addDays(ymd: string, delta: number): string {
  const { year, month, day } = parseYmd(ymd);
  const utc = new Date(Date.UTC(year, month - 1, day + delta));
  return formatYmd(utc.getUTCFullYear(), utc.getUTCMonth() + 1, utc.getUTCDate());
}

export function addMonths(ymd: string, delta: number): string {
  const { year, month, day } = parseYmd(ymd);
  const first = new Date(Date.UTC(year, month - 1 + delta, 1));
  const last = new Date(Date.UTC(first.getUTCFullYear(), first.getUTCMonth() + 1, 0)).getUTCDate();
  return formatYmd(first.getUTCFullYear(), first.getUTCMonth() + 1, Math.min(day, last));
}

export function weekdayIndex(ymd: string): number {
  const { year, month, day } = parseYmd(ymd);
  return new Date(Date.UTC(year, month - 1, day)).getUTCDay();
}

export function weekDates(anchorYmd: string): string[] {
  const start = addDays(anchorYmd, -weekdayIndex(anchorYmd));
  return Array.from({ length: 7 }, (_, index) => addDays(start, index));
}

export function monthGrid(anchorYmd: string): string[] {
  const { year, month } = parseYmd(anchorYmd);
  const first = formatYmd(year, month, 1);
  const start = addDays(first, -weekdayIndex(first));
  const lastDay = new Date(Date.UTC(year, month, 0)).getUTCDate();
  const last = formatYmd(year, month, lastDay);
  const dates: string[] = [];
  for (let cursor = start; cursor <= last || dates.length % 7 !== 0; cursor = addDays(cursor, 1)) {
    dates.push(cursor);
    if (dates.length > 42) break;
  }
  return dates;
}

export function safeTimeZone(timeZone: string | null | undefined): string {
  if (!timeZone) return "UTC";
  try {
    Intl.DateTimeFormat("en-US", { timeZone }).format(0);
    return timeZone;
  } catch {
    return "UTC";
  }
}

export function facilityDateParts(instant: Date, timeZone: string) {
  const zone = safeTimeZone(timeZone);
  const bag: Record<string, string> = {};
  for (const part of new Intl.DateTimeFormat("en-US", {
    timeZone: zone,
    hourCycle: "h23",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).formatToParts(instant)) {
    if (part.type !== "literal") bag[part.type] = part.value;
  }
  let hour = Number(bag.hour);
  if (hour === 24) hour = 0;
  return {
    year: Number(bag.year),
    month: Number(bag.month),
    day: Number(bag.day),
    hour,
    minute: Number(bag.minute),
    second: Number(bag.second),
  };
}

export function facilityYmd(instant: Date, timeZone: string): string {
  if (Number.isNaN(instant.getTime())) return "";
  const parts = facilityDateParts(instant, timeZone);
  if (!parts.year || !parts.month || !parts.day) return "";
  return formatYmd(parts.year, parts.month, parts.day);
}

export function formatFacilityTime(instant: Date, timeZone: string): string {
  if (Number.isNaN(instant.getTime())) return "";
  return new Intl.DateTimeFormat("en-US", {
    timeZone: safeTimeZone(timeZone),
    hour: "numeric",
    minute: "2-digit",
  }).format(instant);
}

export function formatYmdLong(ymd: string): string {
  const { year, month, day } = parseYmd(ymd);
  return new Intl.DateTimeFormat("en-US", {
    weekday: "short",
    month: "short",
    day: "numeric",
    year: "numeric",
    timeZone: "UTC",
  }).format(new Date(Date.UTC(year, month - 1, day, 12)));
}

export function zonedLocalToUtc(
  ymd: string,
  hour: number,
  minute: number,
  timeZone: string,
): string {
  const zone = safeTimeZone(timeZone);
  parseYmd(ymd);
  const candidates = wallTimeCandidates(`${ymd}T${pad2(hour)}:${pad2(minute)}`, zone);
  if (!candidates.length) throw new Error(`That time does not exist in ${zone}.`);
  return candidates[0];
}

const boundaries = new Map<string, number>();
function dayBoundary(ymd: string, zone: string): number {
  const key = `${ymd}:${zone}`;
  const cached = boundaries.get(key);
  if (cached !== undefined) return cached;
  // Some zones change clocks at midnight. A wholly skipped date has zero duration.
  for (let minute = 0; minute < 1440; minute++) {
    const candidates = wallTimeCandidates(`${ymd}T${pad2(Math.floor(minute / 60))}:${pad2(minute % 60)}`, safeTimeZone(zone));
    if (candidates.length) { const result = Date.parse(candidates[0]); boundaries.set(key, result); return result; }
  }
  const result = dayBoundary(addDays(ymd, 1), zone);
  boundaries.set(key, result);
  return result;
}

export function intervalOverlapsDay(
  startIso: string,
  endIso: string,
  ymd: string,
  timeZone: string,
): boolean {
  const start = new Date(startIso).getTime();
  const end = new Date(endIso).getTime();
  if (!(end > start) || !/^\d{4}-\d{2}-\d{2}$/.test(ymd)) return false;
  const dayStart = dayBoundary(ymd, timeZone);
  const dayEnd = dayBoundary(addDays(ymd, 1), timeZone);
  return dayEnd > dayStart && start < dayEnd && end > dayStart;
}

export function crossesMidnight(startIso: string, endIso: string, timeZone: string): boolean {
  const start = new Date(startIso);
  const end = new Date(endIso);
  if (!(end.getTime() > start.getTime())) return false;
  const startDay = facilityYmd(start, timeZone);
  const lastDay = facilityYmd(new Date(end.getTime() - 1), timeZone);
  return Boolean(startDay && lastDay && startDay !== lastDay);
}

export function defaultFacilityId(
  actorFacilityId: string | null | undefined,
  facilities: { id: string; active?: boolean }[],
): string {
  if (actorFacilityId && facilities.some((facility) => facility.id === actorFacilityId)) {
    return actorFacilityId;
  }
  return facilities.find((facility) => facility.active !== false)?.id ?? facilities[0]?.id ?? ALL;
}

export function selectCalendarItems<T extends Schedulable>(items: T[], filters: CalendarFilter,
  window?: { days: string[]; timeZone: string }): T[] {
  const facility = blankFilter(filters.facilityId) ? "" : filters.facilityId;
  const clinician = blankFilter(filters.clinicianId) ? "" : filters.clinicianId;
  const type = blankFilter(filters.type) ? "" : filters.type;
  const status = blankFilter(filters.status) ? "" : filters.status;
  const matched = items.filter(
    (item) =>
      (!facility || item.facilityId === facility) &&
      (!clinician || item.clinicianId === clinician) &&
      (!type || item.type === type) &&
      (!status || item.status === status) &&
      (!window || window.days.some(day => intervalOverlapsDay(item.startsAt, item.endsAt, day, window.timeZone))),
  );
  const hiddenSessions = new Set<string>();
  for (const item of matched) {
    if (item.source !== "group") continue;
    for (const sessionId of item.linkedSessionIds ?? []) hiddenSessions.add(sessionId);
  }
  return matched
    .filter((item) => item.source !== "session" || !hiddenSessions.has(item.id))
    .sort((a, b) => a.startsAt.localeCompare(b.startsAt) || a.id.localeCompare(b.id));
}

export function visibleParticipantNames(
  attendanceClientIds: string[],
  authorizedNames: Record<string, string>,
): ParticipantDisclosure {
  const names: string[] = [];
  let hiddenCount = 0;
  for (const id of attendanceClientIds) {
    const name = authorizedNames[id];
    if (name) names.push(name);
    else hiddenCount += 1;
  }
  return { names, hiddenCount };
}

export function createInitial(input: {
  kind: "session" | "group";
  day: string;
  hour: number;
  minute?: number;
  durationMinutes?: number;
  facilityId: string;
  clinicianId: string;
  timeZone: string;
}): Record<string, unknown> {
  const minute = input.minute ?? 0;
  const duration = input.durationMinutes ?? 60;
  const start = zonedLocalToUtc(input.day, input.hour, minute, input.timeZone);
  return {
    facility_id: input.facilityId,
    clinician_id: input.clinicianId,
    starts_at: start,
    ends_at: new Date(Date.parse(start) + duration * 60_000).toISOString(),
    _require_time_occurrence: true,
    status: "scheduled",
    kind: input.kind === "group" ? "process" : "individual",
  };
}

export function serviceClass(type: string, source: "session" | "group"): string {
  return `cal-chip ${source === "group" ? "is-group" : "is-session"} ${SERVICE_CLASS[type] ?? "svc-other"}`;
}
