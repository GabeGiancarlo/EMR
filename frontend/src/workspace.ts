import type { Bootstrap, Row } from "./api.ts";

/**
 * Drill-down filters for onNavigate(view, filter). Existing screens consume them:
 * - clients: "active" | "inquiry" | "intake" | "discharged" | "all"
 * - schedule: "today" resets the agenda to the current week (not a fake historical census)
 * - notes: "draft" | "signed" | "all"
 * - tasks: "open" | "in_progress" | "done" | "all"
 * - billing tab ids: "claims" | "policies" | "authorizations" | "payments"
 * - billing claim filters: "draft" | "ready" | "submitted" | "denied" | "paid" |
 *   "voided" | "partially_paid" | "balance" | "attention" | "all"
 * Date inputs apply only to flow metrics (charges, cash, adjustments).
 * Outstanding, census, expiring authorizations, and staffing are current snapshots.
 */
export type WorkspaceMode = "clinical" | "billing" | "executive";

export type MoneyState =
  | { state: "value"; cents: number }
  | { state: "unavailable"; reason: string }
  | { state: "unknown"; reason: string };

export const LEDGER_DEFINITIONS = {
  charges:
    "Charges are the sum of non-voided claim amounts whose linked session starts on a facility-local calendar date inside the inclusive range. This is not cash, not revenue, and not profit.",
  cash:
    "Payer and client receipts dated in the selected range, less cash reversals dated in that range, including reversals of earlier receipts. Excludes adjustments.",
  adjustments:
    "Adjustments are payments sourced as adjustment with received_on inside the inclusive range, minus reversals of those adjustment payments whose reversed_on is inside the range. They are not cash.",
  outstanding:
    "Current outstanding is submitted, denied, partially paid, and paid claim amounts, minus every loaded payment, plus every loaded reversal. It ignores the date filter. It is not revenue and not profit.",
} as const;

export const UNAVAILABLE_EXECUTIVE_METRICS =
  "Outcomes, compliance scores, and profit are not available. This workspace has no outcome measures, compliance rules, or expenses, so those figures are not estimated.";

export const SERVICE_COLORS: Record<string, string> = {
  individual: "#2f6f62",
  group: "#3c7ea6",
  assessment: "#c4843c",
  family: "#8a6794",
  case_management: "#b45c4e",
};

export const LEVEL_COLORS: Record<string, string> = {
  outpatient: "#2f6f62",
  iop: "#3c7ea6",
  php: "#c4843c",
  residential: "#8a6794",
  detox: "#b45c4e",
};

export const AUTH_WINDOW_DAYS = 30;

const MODE_ORDER: WorkspaceMode[] = ["clinical", "billing", "executive"];
const CLINICAL_RESOURCES = [
  "notes",
  "treatment_plans",
  "goals",
  "plan_reviews",
  "form_responses",
];
const FINANCIAL_RESOURCES = [
  "policies",
  "authorizations",
  "claims",
  "payments",
  "payment_reversals",
];
const OUTSTANDING_STATUSES = ["submitted", "denied", "partially_paid", "paid"];

export function actorRole(data: Bootstrap): string {
  const role = data.actor?.role;
  return typeof role === "string" ? role : "";
}

export function roleGrantsRead(role: string, resource: string): boolean {
  if (role === "admin") return true;
  switch (resource) {
    case "organizations":
    case "facilities":
    case "staff":
    case "groups":
    case "group_attendance":
    case "clients":
    case "episodes":
    case "sessions":
    case "consents":
    case "tasks":
      return true;
    case "audit":
    case "exceptions":
      return role === "clinical_lead" || role === "executive";
    case "notes":
    case "treatment_plans":
    case "goals":
    case "plan_reviews":
    case "form_responses":
      return role === "clinical_lead" || role === "clinician";
    case "templates":
      return role === "clinical_lead" || role === "clinician" || role === "support";
    case "policies":
    case "authorizations":
    case "claims":
    case "payments":
    case "payment_reversals":
      return role === "billing" || role === "executive" || role === "clinical_lead";
    default:
      return false;
  }
}

function permissionEntry(
  data: Bootstrap,
  resource: string,
): { read?: boolean; write?: boolean } | undefined {
  const permissions = data.permissions as
    | Record<string, { read?: boolean; write?: boolean } | undefined>
    | undefined;
  return permissions?.[resource];
}

export function readAllowed(data: Bootstrap, resource: string): boolean {
  return permissionEntry(data, resource)?.read === true;
}

export function writeAllowed(data: Bootstrap, resource: string): boolean {
  return permissionEntry(data, resource)?.write === true;
}

export function noteActionsAllowed(data: Bootstrap): boolean {
  return writeAllowed(data, "notes");
}

export function visibleRows(data: Bootstrap, resource: string): Row[] | null {
  if (!readAllowed(data, resource)) return null;
  const value = (data as unknown as Record<string, unknown>)[resource];
  return Array.isArray(value) ? (value as Row[]) : null;
}

function roleDefaultModes(role: string): WorkspaceMode[] {
  if (role === "admin") return ["clinical", "billing", "executive"];
  if (role === "clinician" || role === "clinical_lead" || role === "support")
    return ["clinical"];
  if (role === "billing") return ["billing"];
  if (role === "executive") return ["executive"];
  return [];
}

export function allowedWorkspaceModes(data: Bootstrap): WorkspaceMode[] {
  const role = actorRole(data);
  const modes = new Set<WorkspaceMode>(roleDefaultModes(role));
  if (role !== "admin") {
    if (
      !modes.has("clinical") &&
      CLINICAL_RESOURCES.some((resource) => readAllowed(data, resource))
    ) {
      modes.add("clinical");
    }
    if (
      !modes.has("billing") &&
      role !== "executive" &&
      FINANCIAL_RESOURCES.some((resource) => readAllowed(data, resource))
    ) {
      modes.add("billing");
    }
    if (
      !modes.has("executive") &&
      readAllowed(data, "audit") &&
      !roleGrantsRead(role, "audit")
    ) {
      modes.add("executive");
    }
  }
  return MODE_ORDER.filter((mode) => modes.has(mode));
}

export function defaultWorkspaceMode(data: Bootstrap): WorkspaceMode {
  const role = actorRole(data);
  const allowed = allowedWorkspaceModes(data);
  const preferred: WorkspaceMode | null =
    role === "billing"
      ? "billing"
      : role === "executive"
        ? "executive"
        : role === "admin" ||
            role === "clinician" ||
            role === "clinical_lead" ||
            role === "support"
          ? "clinical"
          : null;
  if (preferred && allowed.includes(preferred)) return preferred;
  return allowed[0] ?? "clinical";
}

export function claimMatchesFilter(status: string, filter: string): boolean {
  if (!filter || filter === "all" || filter === "claims") return true;
  if (filter === "balance") return OUTSTANDING_STATUSES.filter((s) => s !== "paid").includes(status);
  if (filter === "attention")
    return ["draft", "ready", "submitted", "denied"].includes(status);
  return status === filter;
}

export function isYmd(value: string): boolean {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const [year, month, day] = value.split("-").map(Number);
  const utc = new Date(Date.UTC(year, month - 1, day));
  return (
    utc.getUTCFullYear() === year &&
    utc.getUTCMonth() === month - 1 &&
    utc.getUTCDate() === day
  );
}

export function addDays(ymd: string, days: number): string | null {
  if (!isYmd(ymd) || !Number.isSafeInteger(days)) return null;
  const [year, month, day] = ymd.split("-").map(Number);
  const utc = new Date(Date.UTC(year, month - 1, day));
  utc.setUTCDate(utc.getUTCDate() + days);
  return utc.toISOString().slice(0, 10);
}

export function monthStart(ymd: string): string | null {
  return isYmd(ymd) ? `${ymd.slice(0, 8)}01` : null;
}

export function monthEnd(ymd: string): string | null {
  const start = monthStart(ymd);
  if (!start) return null;
  const [year, month] = start.split("-").map(Number);
  const next = new Date(Date.UTC(year, month, 1));
  next.setUTCDate(next.getUTCDate() - 1);
  return next.toISOString().slice(0, 10);
}

export function defaultFlowRange(today: string): { from: string; to: string } | null {
  const from = monthStart(today);
  if (!from || !isYmd(today)) return null;
  return { from, to: today };
}

export function inInclusiveRange(date: string, from: string, to: string): boolean {
  return isYmd(date) && isYmd(from) && isYmd(to) && from <= to && date >= from && date <= to;
}

export function calendarDateInZone(instant: Date, timeZone: string): string | null {
  if (!(instant instanceof Date) || Number.isNaN(instant.getTime())) return null;
  if (!timeZone) return null;
  try {
    const parts = new Intl.DateTimeFormat("en-US", {
      timeZone,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
    }).formatToParts(instant);
    const year = parts.find((part) => part.type === "year")?.value;
    const month = parts.find((part) => part.type === "month")?.value;
    const day = parts.find((part) => part.type === "day")?.value;
    if (!year || !month || !day) return null;
    const ymd = `${year}-${month}-${day}`;
    return isYmd(ymd) ? ymd : null;
  } catch {
    return null;
  }
}

export function formatZoneTime(instant: Date, timeZone: string): string | null {
  if (Number.isNaN(instant.getTime())) return null;
  try {
    return new Intl.DateTimeFormat("en-US", {
      timeZone,
      hour: "numeric",
      minute: "2-digit",
    }).format(instant);
  } catch {
    return null;
  }
}

export function facilityZone(data: Bootstrap, facilityId: string | undefined): string | null {
  if (!facilityId) return null;
  const facilities = visibleRows(data, "facilities") ?? [];
  const facility = facilities.find((row) => row.id === facilityId);
  const zone = facility?.timezone;
  if (typeof zone !== "string" || !zone.trim()) return null;
  return calendarDateInZone(new Date("2026-01-15T12:00:00Z"), zone) ? zone : null;
}

export function actorFacilityZone(data: Bootstrap): string {
  const actorFacility =
    typeof data.actor?.facility_id === "string" ? data.actor.facility_id : undefined;
  return (
    facilityZone(data, actorFacility) ??
    facilityZone(
      data,
      data.facilities.find((row) => row.active !== false)?.id,
    ) ??
    "UTC"
  );
}

export function asCents(value: unknown): number | null {
  if (typeof value === "number" && Number.isSafeInteger(value)) return value;
  if (typeof value === "string" && /^-?\d+$/.test(value)) {
    const parsed = Number(value);
    return Number.isSafeInteger(parsed) ? parsed : null;
  }
  return null;
}

export function formatCents(cents: number): string {
  const sign = cents < 0 ? "-" : "";
  const abs = Math.abs(cents);
  const dollars = Math.trunc(abs / 100);
  const remainder = abs % 100;
  return `${sign}$${dollars.toLocaleString("en-US")}.${String(remainder).padStart(2, "0")}`;
}

function text(row: Row | undefined, key: string): string {
  const value = row?.[key];
  return value == null ? "" : String(value);
}

function addCents(total: number, delta: number): number | null {
  if (!Number.isSafeInteger(total) || !Number.isSafeInteger(delta)) return null;
  const next = total + delta;
  return Number.isSafeInteger(next) ? next : null;
}

function unavailable(reason: string): MoneyState {
  return { state: "unavailable", reason };
}

function unknown(reason: string): MoneyState {
  return { state: "unknown", reason };
}

function sessionFacilityId(data: Bootstrap, session: Row): string {
  const episodes = visibleRows(data, "episodes") ?? [];
  const clients = visibleRows(data, "clients") ?? [];
  const episode = episodes.find((row) => row.id === session.episode_id);
  if (episode && typeof episode.facility_id === "string") return episode.facility_id;
  const client = clients.find((row) => row.id === session.client_id);
  return typeof client?.facility_id === "string" ? client.facility_id : "";
}

export function claimServiceDate(data: Bootstrap, claim: Row): string | null {
  const sessions = visibleRows(data, "sessions");
  if (!sessions) return null;
  const session = sessions.find((row) => row.id === claim.session_id);
  if (!session || typeof session.starts_at !== "string") return null;
  const zone = facilityZone(data, sessionFacilityId(data, session));
  if (!zone) return null;
  return calendarDateInZone(new Date(session.starts_at), zone);
}

function sourceBucket(source: unknown): "cash" | "adjustment" | "unknown" {
  if (source === "payer" || source === "client") return "cash";
  if (source === "adjustment") return "adjustment";
  return "unknown";
}

export function currentOutstanding(data: Bootstrap): MoneyState {
  const claims = visibleRows(data, "claims");
  const payments = visibleRows(data, "payments");
  const reversals = visibleRows(data, "payment_reversals");
  if (!claims) return unavailable("Claims are not included for this access, so outstanding is not shown as zero.");
  if (!payments)
    return unavailable("Payments are not included for this access, so outstanding is not shown as zero.");
  if (!reversals)
    return unavailable(
      "Payment reversals are not included for this access, so outstanding is not shown as zero.",
    );
  let total = 0;
  for (const claim of claims) {
    if (!OUTSTANDING_STATUSES.includes(text(claim, "status"))) continue;
    const amount = asCents(claim.amount_cents);
    if (amount === null) {
      return unknown("A receivable amount is missing or not a safe integer, so outstanding is not shown as zero.");
    }
    const next = addCents(total, amount);
    if (next === null) return unknown("Outstanding exceeds safe integer cents.");
    total = next;
  }
  for (const payment of payments) {
    const amount = asCents(payment.amount_cents);
    if (amount === null) {
      return unknown("A payment amount is missing or not a safe integer, so outstanding is not shown as zero.");
    }
    const next = addCents(total, -amount);
    if (next === null) return unknown("Outstanding exceeds safe integer cents.");
    total = next;
  }
  for (const reversal of reversals) {
    const amount = asCents(reversal.amount_cents);
    if (amount === null) {
      return unknown("A reversal amount is missing or not a safe integer, so outstanding is not shown as zero.");
    }
    const next = addCents(total, amount);
    if (next === null) return unknown("Outstanding exceeds safe integer cents.");
    total = next;
  }
  return { state: "value", cents: total };
}

function flowBucket(
  data: Bootstrap,
  from: string,
  to: string,
  bucket: "cash" | "adjustment",
): MoneyState {
  const payments = visibleRows(data, "payments");
  const reversals = visibleRows(data, "payment_reversals");
  if (!payments) return unavailable("Payments are not included for this access, so this flow is not shown as zero.");
  if (!reversals) {
    return unknown("Payment reversals are not included, so cash and adjustments are not guessed.");
  }
  let total = 0;
  let saw = false;
  for (const payment of payments) {
    const kind = sourceBucket(payment.source);
    const received = text(payment, "received_on");
    if (!isYmd(received)) return unknown("A payment is missing a valid receipt date. Review the payment ledger.");
    const inRange = isYmd(received) && inInclusiveRange(received, from, to);
    if (!inRange && kind !== "unknown") continue;
    if (kind === "unknown" && inRange) {
      return unknown("A payment source is missing, so cash and adjustments are not guessed.");
    }
    if (!inRange || kind !== bucket) continue;
    const amount = asCents(payment.amount_cents);
    if (amount === null) {
      return unknown("A payment amount is missing or not a safe integer, so the flow is not shown as zero.");
    }
    const next = addCents(total, amount);
    if (next === null) return unknown("The flow total exceeds safe integer cents.");
    total = next;
    saw = true;
  }
  for (const reversal of reversals) {
    const reversedOn = text(reversal, "reversed_on");
    if (!isYmd(reversedOn)) return unknown("A reversal is missing a valid date. Review the payment ledger.");
    if (!isYmd(reversedOn) || !inInclusiveRange(reversedOn, from, to)) continue;
    const payment = payments.find((row) => row.id === reversal.payment_id);
    if (!payment) {
      return unknown(
        "A reversal in this range has no loaded payment, so cash and adjustments are not guessed or zeroed.",
      );
    }
    const kind = sourceBucket(payment.source);
    if (kind === "unknown") {
      return unknown("A reversed payment has no cash or adjustment source, so neither flow is guessed.");
    }
    if (kind !== bucket) continue;
    const amount = asCents(reversal.amount_cents);
    if (amount === null) {
      return unknown("A reversal amount is missing or not a safe integer, so the flow is not shown as zero.");
    }
    const next = addCents(total, -amount);
    if (next === null) return unknown("The flow total exceeds safe integer cents.");
    total = next;
    saw = true;
  }
  return { state: "value", cents: saw ? total : 0 };
}

export function flowCharges(data: Bootstrap, from: string, to: string): MoneyState {
  const claims = visibleRows(data, "claims");
  if (!claims) return unavailable("Claims are not included for this access, so charges are not shown as zero.");
  if (!visibleRows(data, "sessions")) {
    return unknown("Sessions are not included, so charge dates cannot be placed in the range.");
  }
  let total = 0;
  let saw = false;
  for (const claim of claims) {
    if (text(claim, "status") === "voided") continue;
    const serviceDate = claimServiceDate(data, claim);
    if (!serviceDate) {
      return unknown(
        "A non-voided claim has no facility-local service date, so month charges are not shown as zero or partial.",
      );
    }
    if (!inInclusiveRange(serviceDate, from, to)) continue;
    const amount = asCents(claim.amount_cents);
    if (amount === null) {
      return unknown("A charge amount is missing or not a safe integer, so charges are not shown as zero.");
    }
    const next = addCents(total, amount);
    if (next === null) return unknown("Charges exceed safe integer cents.");
    total = next;
    saw = true;
  }
  return { state: "value", cents: saw ? total : 0 };
}

export type FlowLedger = {
  charges: MoneyState;
  cash: MoneyState;
  adjustments: MoneyState;
  outstanding: MoneyState;
  definitions: typeof LEDGER_DEFINITIONS;
};

export function flowLedger(data: Bootstrap, from: string, to: string): FlowLedger {
  const outstanding = currentOutstanding(data);
  if (!isYmd(from) || !isYmd(to) || from > to) {
    const reason = "The date range is missing or reversed, so flow totals are not shown as zero.";
    return {
      charges: unknown(reason),
      cash: unknown(reason),
      adjustments: unknown(reason),
      outstanding,
      definitions: LEDGER_DEFINITIONS,
    };
  }
  return {
    charges: flowCharges(data, from, to),
    cash: flowBucket(data, from, to, "cash"),
    adjustments: flowBucket(data, from, to, "adjustment"),
    outstanding,
    definitions: LEDGER_DEFINITIONS,
  };
}

export function claimBalance(data: Bootstrap, claim: Row): MoneyState {
  const payments = visibleRows(data, "payments");
  const reversals = visibleRows(data, "payment_reversals");
  if (!payments || !reversals) {
    return unavailable("Balance needs loaded payments and reversals, so it is not shown as zero.");
  }
  const amount = asCents(claim.amount_cents);
  if (amount === null || text(claim, "status") === "voided") {
    return unknown("This claim amount cannot be turned into a current balance.");
  }
  let total = amount;
  for (const payment of payments) {
    if (payment.claim_id !== claim.id) continue;
    const paid = asCents(payment.amount_cents);
    if (paid === null) return unknown("A payment on this claim is not a safe integer.");
    const next = addCents(total, -paid);
    if (next === null) return unknown("This claim balance exceeds safe integer cents.");
    total = next;
  }
  for (const reversal of reversals) {
    if (reversal.claim_id !== claim.id) continue;
    const reversed = asCents(reversal.amount_cents);
    if (reversed === null) return unknown("A reversal on this claim is not a safe integer.");
    const next = addCents(total, reversed);
    if (next === null) return unknown("This claim balance exceeds safe integer cents.");
    total = next;
  }
  return { state: "value", cents: total };
}

export type DatedSession = {
  row: Row;
  facilityId: string;
  timeZone: string;
  localDate: string;
  localTime: string | null;
};

export function todaysSessions(
  data: Bootstrap,
  now: Date,
): { available: boolean; rows: DatedSession[]; undated: Row[] } {
  const sessions = visibleRows(data, "sessions");
  if (!sessions) return { available: false, rows: [], undated: [] };
  const rows: DatedSession[] = [];
  const undated: Row[] = [];
  for (const row of sessions) {
    if (typeof row.starts_at !== "string") {
      undated.push(row);
      continue;
    }
    const facilityId = sessionFacilityId(data, row);
    const zone = facilityZone(data, facilityId);
    if (!zone) {
      undated.push(row);
      continue;
    }
    const instant = new Date(row.starts_at);
    const localDate = calendarDateInZone(instant, zone);
    const today = calendarDateInZone(now, zone);
    if (!localDate || !today) {
      undated.push(row);
      continue;
    }
    if (localDate !== today) continue;
    rows.push({
      row,
      facilityId,
      timeZone: zone,
      localDate,
      localTime: formatZoneTime(instant, zone),
    });
  }
  rows.sort((a, b) => text(a.row, "starts_at").localeCompare(text(b.row, "starts_at")));
  return { available: true, rows, undated };
}

export function todaysGroupCount(data: Bootstrap, now: Date): number | null {
  const groups = visibleRows(data, "groups");
  if (!groups) return null;
  let count = 0;
  for (const group of groups) {
    if (typeof group.starts_at !== "string") continue;
    const zone = facilityZone(data, typeof group.facility_id === "string" ? group.facility_id : undefined);
    if (!zone) continue;
    const local = calendarDateInZone(new Date(group.starts_at), zone);
    const today = calendarDateInZone(now, zone);
    if (local && today && local === today) count += 1;
  }
  return count;
}

export function upcomingPlanReviews(
  data: Bootstrap,
  today: string | null,
): { available: boolean; dated: Row[]; undated: number } {
  const plans = visibleRows(data, "treatment_plans");
  if (!plans) return { available: false, dated: [], undated: 0 };
  const active = plans.filter(plan => plan.status === "active");
  const dated = active
    .filter((plan) => isYmd(text(plan, "next_review_date")))
    .sort((a, b) => text(a, "next_review_date").localeCompare(text(b, "next_review_date")));
  const undated = active.length - dated.length;
  if (!today) return { available: true, dated, undated };
  return { available: true, dated, undated };
}

export function expiringAuthorizations(
  data: Bootstrap,
  today: string | null,
): { available: boolean; rows: Row[]; undated: number } {
  const authorizations = visibleRows(data, "authorizations");
  if (!authorizations || !today || !isYmd(today)) {
    return { available: authorizations !== null && today !== null, rows: [], undated: 0 };
  }
  const limit = addDays(today, AUTH_WINDOW_DAYS);
  if (!limit) return { available: true, rows: [], undated: 0 };
  const undated = authorizations.filter((row) => !isYmd(text(row, "ends_on"))).length;
  const rows = authorizations
    .filter((row) => {
      const ends = text(row, "ends_on");
      return (
        text(row, "status") === "active" &&
        isYmd(ends) &&
        ends <= limit &&
        (text(row, "status") === "active" || ends >= today)
      );
    })
    .sort((a, b) => text(a, "ends_on").localeCompare(text(b, "ends_on")));
  return { available: true, rows, undated };
}

export type Census = {
  available: boolean;
  totalActive: number | null;
  unclassified: number;
  facilities: {
    id: string;
    name: string;
    total: number;
    levels: { level: string; count: number }[];
  }[];
};

export function currentCensus(data: Bootstrap): Census {
  const episodes = visibleRows(data, "episodes");
  if (!episodes) {
    return { available: false, totalActive: null, unclassified: 0, facilities: [] };
  }
  const facilities = visibleRows(data, "facilities");
  const grouped = new Map<string, Map<string, number>>();
  let totalActive = 0;
  let unclassified = 0;
  for (const episode of episodes) {
    const status = text(episode, "status");
    if (!status) {
      unclassified += 1;
      continue;
    }
    if (status !== "active") continue;
    const facilityId =
      typeof episode.facility_id === "string" && episode.facility_id
        ? episode.facility_id
        : "unassigned";
    const level = text(episode, "level_of_care") || "unspecified";
    const levels = grouped.get(facilityId) ?? new Map<string, number>();
    levels.set(level, (levels.get(level) ?? 0) + 1);
    grouped.set(facilityId, levels);
    totalActive += 1;
  }
  return {
    available: true,
    totalActive,
    unclassified,
    facilities: [...grouped.entries()].map(([id, levels]) => {
      const named = facilities?.find((row) => row.id === id);
      return {
        id,
        name: named ? text(named, "name") || "Unnamed facility" : facilities ? "Facility unavailable" : "Facility name not included",
        total: [...levels.values()].reduce((sum, count) => sum + count, 0),
        levels: [...levels.entries()].map(([level, count]) => ({ level, count })),
      };
    }),
  };
}

export type Staffing = {
  available: boolean;
  activeTotal: number | null;
  unclassified: number;
  activeByRole: { role: string; count: number }[];
};

export function currentStaffing(data: Bootstrap): Staffing {
  const staff = visibleRows(data, "staff");
  if (!staff) return { available: false, activeTotal: null, unclassified: 0, activeByRole: [] };
  const counts = new Map<string, number>();
  let unclassified = 0;
  let activeTotal = 0;
  for (const person of staff) {
    if (person.active === false) continue;
    if (person.active !== true) {
      unclassified += 1;
      continue;
    }
    const role = text(person, "role") || "unspecified";
    counts.set(role, (counts.get(role) ?? 0) + 1);
    activeTotal += 1;
  }
  return {
    available: true,
    activeTotal,
    unclassified,
    activeByRole: [...counts.entries()].map(([role, count]) => ({ role, count })),
  };
}

export function operationalWorkload(data: Bootstrap, today: string | null) {
  const tasks = visibleRows(data, "tasks");
  const sessions = visibleRows(data, "sessions");
  const notes = visibleRows(data, "notes");
  const open = tasks ? tasks.filter((task) => task.status === "open" || task.status === "in_progress").length : null;
  let overdue: number | null = null;
  if (tasks && today && isYmd(today)) {
    overdue = tasks.filter((task) => {
      const due = text(task, "due_date");
      return (
        task.status !== "done" &&
        isYmd(due) &&
        due < today
      );
    }).length;
  }
  return {
    tasks: {
      available: tasks !== null,
      open,
      overdue,
    },
    sessions: {
      available: sessions !== null,
      scheduled: sessions ? sessions.filter((row) => row.status === "scheduled").length : null,
    },
    drafts: notes
      ? { available: true, count: notes.filter((row) => row.status === "draft").length, source: "rows" as const }
      : {
          available: false,
          count: null,
          source: "unavailable" as const,
        },
  };
}
