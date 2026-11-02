/** Adult program date helper. No dependencies.
 *
 * Software policy, not legal advice: eligibility requires 18 completed calendar
 * years on the facility-local date. A February 29 birthday is treated as
 * reaching that anniversary on March 1 when the year is not a leap year.
 */

export const ADULT_PROGRAM_HINT =
  "Adults 18+ only. Age is checked using the selected facility’s local date. February 29 birthdays become eligible on March 1 in a non-leap year.";

export function isLeapYear(year: number): boolean {
  return (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
}

export function parseIsoDate(value: string): { year: number; month: number; day: number } | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return null;
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  if (month < 1 || month > 12 || day < 1) return null;
  const lengths = [31, isLeapYear(year) ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  if (day > lengths[month - 1]) return null;
  return { year, month, day };
}

export function formatIsoDate(year: number, month: number, day: number): string {
  const m = String(month).padStart(2, "0");
  const d = String(day).padStart(2, "0");
  return `${year}-${m}-${d}`;
}

function compare(
  a: { year: number; month: number; day: number },
  b: { year: number; month: number; day: number },
): number {
  if (a.year !== b.year) return a.year - b.year;
  if (a.month !== b.month) return a.month - b.month;
  return a.day - b.day;
}

function addDays(
  date: { year: number; month: number; day: number },
  days: number,
): { year: number; month: number; day: number } {
  const utc = Date.UTC(date.year, date.month - 1, date.day) + days * 86400000;
  const next = new Date(utc);
  return { year: next.getUTCFullYear(), month: next.getUTCMonth() + 1, day: next.getUTCDate() };
}

/** Anniversary on which 18 calendar years are completed. Feb 29 -> Mar 1 in non-leap years. */
export function adultAnniversary(birthIso: string): string | null {
  const birth = parseIsoDate(birthIso);
  if (!birth) return null;
  const year = birth.year + 18;
  if (birth.month === 2 && birth.day === 29 && !isLeapYear(year)) {
    return formatIsoDate(year, 3, 1);
  }
  return formatIsoDate(year, birth.month, birth.day);
}

export function hasCompletedAdultYears(birthIso: string, todayIso: string): boolean {
  const today = parseIsoDate(todayIso);
  const anniversary = adultAnniversary(birthIso);
  const mark = anniversary ? parseIsoDate(anniversary) : null;
  if (!today || !mark) return false;
  return compare(today, mark) >= 0;
}

/** Latest YYYY-MM-DD birth date eligible on the facility-local today. */
export function latestEligibleBirthDate(todayIso: string): string | null {
  const today = parseIsoDate(todayIso);
  if (!today) return null;
  const year = today.year - 18;
  let candidate =
    today.month === 2 && today.day === 29 && !isLeapYear(year)
      ? { year, month: 3, day: 1 }
      : { year, month: today.month, day: today.day };
  const iso = formatIsoDate(candidate.year, candidate.month, candidate.day);
  if (hasCompletedAdultYears(iso, todayIso)) return iso;
  const previous = addDays(candidate, -1);
  const previousIso = formatIsoDate(previous.year, previous.month, previous.day);
  return hasCompletedAdultYears(previousIso, todayIso) ? previousIso : null;
}

/** Facility-local calendar date from an IANA zone. Invalid zones fall back to UTC. */
export function facilityLocalToday(timeZone: string | undefined, now = new Date()): string {
  const zone = timeZone && timeZone.trim() ? timeZone.trim() : "UTC";
  try {
    const parts = new Intl.DateTimeFormat("en-CA", {
      timeZone: zone,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
    }).formatToParts(now);
    const year = parts.find((part) => part.type === "year")?.value;
    const month = parts.find((part) => part.type === "month")?.value;
    const day = parts.find((part) => part.type === "day")?.value;
    if (year && month && day && parseIsoDate(`${year}-${month}-${day}`)) {
      return `${year}-${month}-${day}`;
    }
  } catch {
    /* unrecognized zone */
  }
  return now.toISOString().slice(0, 10);
}
