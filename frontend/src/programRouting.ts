import type { Bootstrap, Row } from "./api";

export function serviceSnapshot(session: Row | undefined): Row | undefined {
  const raw = session?.service_snapshot;
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return undefined;
  const row = raw as Row;
  return typeof row.id === "string" && row.id === session?.program_service_id ? row : undefined;
}

export function availableServices(data: Bootstrap, kind: "sessions" | "groups", context: Record<string, unknown>): Row[] {
  const episode = data.episodes.find(e => e.id === context.episode_id);
  return (data.program_services || []).filter(service => {
    const program = data.care_programs?.find(p => p.id === service.program_id);
    const facility = data.facilities.find(f => f.id === program?.facility_id);
    if (service.active !== true || program?.active !== true || facility?.active !== true) return false;
    return kind === "sessions" ? service.session_kind !== "group" && episode?.program_id === program.id
      : service.session_kind === "group" && context.facility_id === program.facility_id;
  });
}

export function serviceDurationError(snapshot: Row | undefined, start: string, end: string): string | undefined {
  if (!snapshot) return undefined;
  const minutes = (Date.parse(end) - Date.parse(start)) / 60000;
  if (!Number.isFinite(minutes)) return "Enter a valid start and end time.";
  const min = Number(snapshot.min_minutes), max = Number(snapshot.max_minutes);
  if (!Number.isInteger(min) || !Number.isInteger(max) || min < 1 || max < min) return "The saved service duration needs review.";
  if (minutes < min || minutes > max) return `This service allows ${min}–${max} minutes. Review the appointment times.`;
}

/** Values use the persisted record units, including integer cents. */
export function claimServiceDefaults(session: Row | undefined): Record<string, unknown> {
  const snapshot = serviceSnapshot(session);
  if (!snapshot || typeof snapshot.service_code !== "string") return { service_code: "", amount_cents: "" };
  return { service_code: snapshot.service_code, amount_cents: typeof snapshot.default_charge_cents === "number" ? snapshot.default_charge_cents : "" };
}
