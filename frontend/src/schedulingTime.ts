import type { Bootstrap, Row } from "./api.ts";

const formatters = new Map<string, Intl.DateTimeFormat>();
function formatter(zone: string): Intl.DateTimeFormat {
  let result = formatters.get(zone);
  if (!result) {
    result = new Intl.DateTimeFormat("en-CA", { timeZone: zone, year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", second: "2-digit", hourCycle: "h23" });
    formatters.set(zone, result);
  }
  return result;
}
export function facilityWallTime(instant: string | number, zone: string): string {
  const parsed = new Date(instant);
  if (!Number.isFinite(parsed.getTime())) return "";
  const p = Object.fromEntries(formatter(zone).formatToParts(parsed).map(part => [part.type, part.value]));
  return `${p.year}-${p.month}-${p.day}T${p.hour}:${p.minute}`;
}
/** All valid UTC instants for a minute on a facility's wall clock.
 * A DST gap has none; a repeated minute has two. Never silently normalize either. */
export function wallTimeCandidates(wall: string, zone: string): string[] {
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(wall)) return [];
  const nominal = Date.parse(`${wall}:00Z`);
  if (!Number.isFinite(nominal) || new Date(nominal).toISOString().slice(0, 16) !== wall) return [];
  const offsets = new Set<number>();
  for (let hour = -48; hour <= 48; hour += 6) {
    const probe = nominal + hour * 3_600_000;
    const local = facilityWallTime(probe, zone);
    offsets.add(Date.parse(`${local}:00Z`) - probe);
  }
  return [...offsets].map(offset => nominal - offset)
    .filter(instant => facilityWallTime(instant, zone) === wall)
    .sort((a, b) => a - b).map(instant => new Date(instant).toISOString());
}
export function resolveFacilityTime(wall: string, zone: string, selected?: string, original?: string): string {
  const candidates = wallTimeCandidates(wall, zone);
  if (!candidates.length) throw new Error(`This time does not exist in ${zone}. Choose a valid time outside the daylight-saving clock change.`);
  const resolved = candidates.length === 1 ? candidates[0] : selected && candidates.includes(selected) ? selected : null;
  if (!resolved) throw new Error(`This time occurs twice in ${zone}. Choose the first or second occurrence.`);
  // An unrelated edit must not round an existing appointment's seconds away.
  const originalMillis = original ? Date.parse(original) : NaN;
  return Number.isFinite(originalMillis) && Math.floor(originalMillis / 60_000) * 60_000 === Date.parse(resolved)
    ? new Date(originalMillis).toISOString() : resolved;
}
export function occurrenceLabel(instant: string, zone: string, index: number): string {
  return `${index === 0 ? "First" : "Second"} occurrence · ${new Intl.DateTimeFormat("en-US", { timeZone: zone, hour: "numeric", minute: "2-digit", timeZoneName: "longOffset" }).format(new Date(instant))}`;
}
export function schedulingZone(data: Bootstrap, values: Record<string, unknown>, resource?: string): string {
  const episode = data.episodes.find((row: Row) => row.id === values.episode_id);
  const client = data.clients.find((row: Row) => row.id === values.client_id);
  const id = resource === "sessions" ? episode?.facility_id || client?.facility_id || values.facility_id
    : values.facility_id;
  const facility = data.facilities.find((row: Row) => row.id === id)
    || data.facilities.find((row: Row) => row.id === data.actor?.facility_id)
    || (data.facilities.length === 1 ? data.facilities[0] : undefined);
  return typeof facility?.timezone === "string" ? facility.timezone : "UTC";
}
