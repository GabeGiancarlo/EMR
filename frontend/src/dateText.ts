/**
 * Dates and times as people type and read them.
 *
 * Records keep ISO values ("2026-11-18", "15:00"). Fields show the brand's forms,
 * "Nov 18, 2026" and "3:00 PM", and accept the ways staff usually type them.
 */
import { addDays, formatYmd, validCalendarDay } from "./calendar.ts";

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/** "Nov 18, 2026" for a stored day; an empty string for anything else. */
export function formatDateText(ymd: string): string {
  if (!validCalendarDay(ymd)) return "";
  const [year, month, day] = ymd.split("-").map(Number);
  return `${MONTHS[month - 1]} ${day}, ${year}`;
}

/** A two-digit year means this century unless that would be more than ten years ahead. */
function fullYear(text: string, thisYear: number): number {
  if (text.length > 2) return Number(text);
  const recent = 2000 + Number(text);
  return recent <= thisYear + 10 ? recent : recent - 100;
}

function build(year: number, month: number, day: number): string | null {
  if (!Number.isInteger(year) || !Number.isInteger(month) || !Number.isInteger(day)) return null;
  if (year < 100 || year > 9999 || month < 1 || month > 12 || day < 1 || day > 31) return null;
  const ymd = formatYmd(year, month, day);
  return validCalendarDay(ymd) ? ymd : null;
}

/**
 * The stored day for typed text, or null when it is not a date.
 * `today` (YYYY-MM-DD) supplies the year when none is typed and anchors "today".
 */
export function parseDateText(input: string, today: string): string | null {
  const text = input.trim().toLowerCase().replace(/\s+/g, " ");
  if (!text) return null;
  const thisYear = Number(today.slice(0, 4));
  if (validCalendarDay(today)) {
    if (text === "today") return today;
    if (text === "tomorrow") return addDays(today, 1);
    if (text === "yesterday") return addDays(today, -1);
  }
  let match = /^(\d{4})-(\d{1,2})-(\d{1,2})$/.exec(text);
  if (match) return build(Number(match[1]), Number(match[2]), Number(match[3]));
  // 11/18/2026, 11-18-26, 11.18.2026, and 11/18 for this year
  match = /^(\d{1,2})[/.-](\d{1,2})(?:[/.-](\d{2}|\d{4}))?$/.exec(text);
  if (match) return build(match[3] ? fullYear(match[3], thisYear) : thisYear, Number(match[1]), Number(match[2]));
  // 11182026 and 111826
  match = /^(\d{2})(\d{2})(\d{4}|\d{2})$/.exec(text);
  if (match) return build(fullYear(match[3], thisYear), Number(match[1]), Number(match[2]));
  const monthIndex = (name: string) => MONTHS.findIndex((month) => name.startsWith(month.toLowerCase()) && "january february march april may june july august september october november december".split(" ").some((full) => full.startsWith(name)));
  // Nov 18, 2026 · November 18 2026 · nov 18
  match = /^([a-z]{3,9})\.? (\d{1,2})(?:st|nd|rd|th)?(?:,? (\d{2}|\d{4}))?$/.exec(text);
  if (match && monthIndex(match[1]) >= 0) return build(match[3] ? fullYear(match[3], thisYear) : thisYear, monthIndex(match[1]) + 1, Number(match[2]));
  // 18 Nov 2026 · 18 November
  match = /^(\d{1,2})(?:st|nd|rd|th)? ([a-z]{3,9})\.?(?:,? (\d{2}|\d{4}))?$/.exec(text);
  if (match && monthIndex(match[2]) >= 0) return build(match[3] ? fullYear(match[3], thisYear) : thisYear, monthIndex(match[2]) + 1, Number(match[1]));
  return null;
}

/** "Eastern Time" for "America/New_York": the brand names the zone when a time could be in doubt. */
export function zoneLabel(zone: string): string {
  try {
    return new Intl.DateTimeFormat("en-US", { timeZone: zone, timeZoneName: "longGeneric" })
      .formatToParts(new Date()).find((part) => part.type === "timeZoneName")?.value || zone;
  } catch {
    return zone.replace(/_/g, " ");
  }
}

/** "3:00 PM" for a stored "15:00"; an empty string for anything else. */
export function formatTimeText(hhmm: string): string {
  const match = /^([01]\d|2[0-3]):([0-5]\d)$/.exec(hhmm);
  if (!match) return "";
  const hour = Number(match[1]);
  return `${hour % 12 || 12}:${match[2]} ${hour < 12 ? "AM" : "PM"}`;
}

/**
 * The stored "HH:MM" for typed text, or null when it is not a time.
 * Without AM or PM, 7 to 11 is morning and 12 to 6 is afternoon: clinic hours.
 * The field shows the result with its AM or PM, so a wrong guess is visible at once.
 */
export function parseTimeText(input: string): string | null {
  const text = input.trim().toLowerCase().replace(/\s+/g, "").replace(/\./g, "");
  if (!text) return null;
  if (text === "noon") return "12:00";
  if (text === "midnight") return "00:00";
  const match = /^(\d{1,2})(?::?([0-5]\d))?(a|am|p|pm)?$/.exec(text);
  if (!match) return null;
  let hour = Number(match[1]);
  const minute = match[2] ?? "00";
  const half = match[3]?.[0];
  if (half) {
    if (hour < 1 || hour > 12) return null;
    hour = (hour % 12) + (half === "p" ? 12 : 0);
  } else if (hour >= 1 && hour <= 6) {
    hour += 12;
  } else if (hour > 23) {
    return null;
  }
  return `${String(hour).padStart(2, "0")}:${minute}`;
}

/** "50 min" between two wall-clock values on the same or the next day; empty when not positive. */
export function durationText(start: string, end: string): string {
  const parse = (value: string) => {
    const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})$/.exec(value);
    return match ? Date.UTC(+match[1], +match[2] - 1, +match[3], +match[4], +match[5]) : NaN;
  };
  const minutes = (parse(end) - parse(start)) / 60000;
  if (!Number.isFinite(minutes) || minutes <= 0 || minutes > 24 * 60) return "";
  return `${minutes} min`;
}

/** Move a wall-clock value by whole minutes, keeping the YYYY-MM-DDTHH:MM form. */
export function shiftWallTime(value: string, minutes: number): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})$/.exec(value);
  if (!match || !Number.isFinite(minutes)) return value;
  const moved = new Date(Date.UTC(+match[1], +match[2] - 1, +match[3], +match[4], +match[5]) + minutes * 60000);
  const ymd = formatYmd(moved.getUTCFullYear(), moved.getUTCMonth() + 1, moved.getUTCDate());
  if (!validCalendarDay(ymd)) return value;
  return `${ymd}T${String(moved.getUTCHours()).padStart(2, "0")}:${String(moved.getUTCMinutes()).padStart(2, "0")}`;
}

/** Whole minutes from one wall-clock value to another; NaN when either is incomplete. */
export function wallMinutesBetween(start: string, end: string): number {
  const parse = (value: string) => {
    const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})$/.exec(value);
    return match ? Date.UTC(+match[1], +match[2] - 1, +match[3], +match[4], +match[5]) : NaN;
  };
  return (parse(end) - parse(start)) / 60000;
}
