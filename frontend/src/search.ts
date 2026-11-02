import type { Bootstrap, Resource, Row } from "./api.ts";

export type WorkspaceView = "overview" | "clients" | "schedule" | "notes" | "billing" | "tasks" | "team" | "settings";
export type SearchResult = { key: string; resource: Resource; row: Row; title: string; detail: string; view: WorkspaceView };
const sources: { resource: Resource; view: WorkspaceView; label: string; fields: string[] }[] = [
  { resource: "clients", view: "clients", label: "Client", fields: ["first_name", "last_name", "preferred_name"] },
  { resource: "sessions", view: "schedule", label: "Session", fields: ["kind", "location", "starts_at"] },
  { resource: "groups", view: "schedule", label: "Group", fields: ["title", "location", "starts_at"] },
  { resource: "notes", view: "notes", label: "Note", fields: ["title"] },
  { resource: "claims", view: "billing", label: "Claim", fields: ["service_code", "external_reference"] },
  { resource: "authorizations", view: "billing", label: "Authorization", fields: ["authorization_number", "service_code"] },
  { resource: "policies", view: "billing", label: "Coverage", fields: ["payer_name", "member_id"] },
  { resource: "tasks", view: "tasks", label: "Task", fields: ["title"] },
  { resource: "staff", view: "team", label: "Team member", fields: ["name", "role"] },
  { resource: "templates", view: "settings", label: "Form template", fields: ["name", "kind"] },
];
const text = (row: Row | undefined, key: string) => typeof row?.[key] === "string" ? row[key] as string : "";
const normalize = (s: string) => s.normalize("NFKD").replace(/\p{M}/gu, "").toLocaleLowerCase("en-US");
const name = (row?: Row) => row ? `${text(row, "first_name")} ${text(row, "last_name")}`.trim() : "";
const readable = (data: Bootstrap, resource: Resource) => data.permissions?.[resource]?.read === true;

/** Search only the currently authorized bootstrap, with an explicit field allowlist.
 * No narrative, transcript, cached record or search term is persisted or indexed. */
export function searchWorkspace(data: Bootstrap, query: string, limit = 30): SearchResult[] {
  const terms = normalize(query.trim()).split(/\s+/).filter(Boolean);
  if (!terms.length || !data.actor) return [];
  const clients = new Map((readable(data, "clients") ? data.clients : []).map(row => [row.id, row]));
  const results: (SearchResult & { rank: number })[] = [];
  for (const source of sources) {
    if (!readable(data, source.resource)) continue;
    for (const row of data[source.resource] ?? []) {
      const client = clients.get(text(row, "client_id"));
      const clientTitle = name(client);
      const fields = source.fields.map(field => text(row, field)).filter(Boolean);
      const title = source.resource === "clients" ? name(row)
        : ["sessions", "claims", "authorizations", "policies"].includes(source.resource)
          ? `${clientTitle || source.label} · ${fields[0] || source.label}`
          : fields[0] || source.label;
      const detail = [source.label, text(row, "status") || text(row, "role"), source.resource === "clients" ? text(row, "preferred_name") : clientTitle].filter(Boolean).join(" · ");
      const searchable = normalize([row.id, source.label, ...fields, clientTitle, text(row, "status")].join(" "));
      if (!terms.every(term => searchable.includes(term))) continue;
      results.push({ key: `${source.resource}:${row.id}`, resource: source.resource, row, title, detail, view: source.view,
        rank: terms.every(term => normalize(title).includes(term)) ? 0 : 1 });
    }
  }
  return results.sort((a, b) => a.rank - b.rank || a.title.localeCompare(b.title) || a.key.localeCompare(b.key)).slice(0, Math.max(0, Math.min(100, limit)));
}

/** Navigation is an affordance; the API still enforces access on every action. */
export function visibleViews(data: Bootstrap): WorkspaceView[] {
  const views: WorkspaceView[] = ["overview"];
  for (const view of ["clients", "schedule", "notes", "billing", "tasks", "team", "settings"] as WorkspaceView[]) {
    if (sources.some(source => source.view === view && readable(data, source.resource))) views.push(view);
    else if (view === "settings" && (readable(data, "facilities") || data.permissions?.exceptions?.read)) views.push(view);
  }
  return views;
}
