export type Row = { id: string; [key: string]: unknown };
export type FormField = {
  key: string;
  label: string;
  type: string;
  required: boolean;
  options?: string[];
};
export type Resource =
  | "facilities"
  | "staff"
  | "clients"
  | "episodes"
  | "sessions"
  | "notes"
  | "tasks"
  | "policies"
  | "authorizations"
  | "claims"
  | "payments"
  | "templates"
  | "consents"
  | "exceptions"
  | "treatment_plans"
  | "goals"
  | "plan_reviews"
  | "groups"
  | "group_attendance"
  | "form_responses";
export type Bootstrap = Record<Resource, Row[]> & {
  setup_required: boolean;
  authentication: string;
  actor: Row | null;
  organizations: Row[];
  audit: Row[];
  payment_reversals: Row[];
  permissions?: Partial<Record<Resource | "audit" | "payment_reversals", { read: boolean; write: boolean }>>;
  metrics: {
    active_clients: number;
    open_tasks: number;
    scheduled_sessions: number;
    draft_notes: number;
    outstanding_cents: number;
    unbilled_cents: number;
    cash_received_cents: number;
    adjustments_cents: number;
  };
};
const base =
  (import.meta.env.VITE_API_BASE_URL as string | undefined)?.replace(
    /\/$/,
    "",
  ) ?? "";
export const actorStorageKey = "nexus.local-operator";
export const apiUrl = (path: string): string => `${base}${path}`;
export async function request<T>(
  path: string,
  method = "GET",
  body?: unknown,
): Promise<T> {
  const actor = localStorage.getItem(actorStorageKey);
  const response = await fetch(`${base}${path}`, {
    method,
    headers: {
      Accept: "application/json",
      ...(body !== undefined ? { "Content-Type": "application/json" } : {}),
      ...(actor ? { "x-nexus-actor": actor } : {}),
    },
    ...(body !== undefined ? { body: JSON.stringify(body) } : {}),
  });
  const data = await response.json().catch(() => null);
  if (!response.ok)
    throw new Error(
      data?.error?.message ||
        `Request could not be completed (${response.status}).`,
    );
  return data as T;
}
export function withOriginalUpdatedAt(
  record: Row | null | undefined,
  body: unknown,
): Record<string, unknown> {
  const fields =
    typeof body === "object" && body !== null && !Array.isArray(body)
      ? { ...(body as Record<string, unknown>) }
      : {};
  // Token is the row already loaded for this edit. Do not refetch updated_at.
  return { ...fields, _expected_updated_at: record?.updated_at };
}
export const value = (row: Row | undefined | null, key: string): string =>
  row?.[key] == null ? "" : String(row[key]);
export const numeric = (row: Row | undefined | null, key: string): number =>
  Number(row?.[key] ?? 0);
export const clientName = (row: Row | undefined | null): string =>
  row
    ? `${value(row, "first_name")} ${value(row, "last_name")}`.trim()
    : "Client unavailable";
export const initials = (name: string): string =>
  name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((s) => s[0])
    .join("")
    .toUpperCase();
export const money = (cents: number): string =>
  new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" }).format(
    cents / 100,
  );
const acronyms: Record<string, string> = {
  iop: "IOP",
  php: "PHP",
  id: "ID",
  api: "API",
  fhir: "FHIR",
  oauth: "OAuth",
  cpt: "CPT",
};
export const label = (text: string): string =>
  text
    .replace(/_/g, " ")
    .replace(
      /\b\w+/g,
      (word) =>
        acronyms[word.toLowerCase()] ||
        word.charAt(0).toUpperCase() + word.slice(1),
    );
export const date = (input: unknown, time = false): string => {
  if (!input) return "Not set";
  const text = String(input);
  const parsed = new Date(text.length === 10 ? `${text}T12:00:00` : text);
  return Number.isNaN(parsed.getTime())
    ? text
    : new Intl.DateTimeFormat("en-US", {
        month: "short",
        day: "numeric",
        year: "numeric",
        ...(time ? { hour: "numeric", minute: "2-digit" } : {}),
      }).format(parsed);
};
