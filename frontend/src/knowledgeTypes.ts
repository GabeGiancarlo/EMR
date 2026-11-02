export const SOURCE_KINDS = [
  "note",
  "form_response",
  "treatment_plan",
  "plan_review",
  "transcript_excerpt",
] as const;

export type SourceKind = (typeof SOURCE_KINDS)[number];

export const DEFAULT_LIMIT = 25;
export const MAX_LIMIT = 50;
export const PAGE_SIZES = [25, 50] as const;

export const KNOWLEDGE_DISCLAIMER =
  "Approved notes, forms, plans and reviewed patient excerpts. Check each source date and encounter before using historical information in new documentation.";

export const STAT_LABELS = {
  documents: "Indexed documents",
  extracts: "Indexed extracts",
  matches: "Matches this view",
} as const;

export const KNOWLEDGE_PARSE_ERROR =
  "The knowledge response was incomplete or unexpected. Nothing was shown.";
export const KNOWLEDGE_SCOPE_ERROR =
  "The knowledge response did not match this client. Nothing was shown.";
export const SOURCE_PARSE_ERROR =
  "The source response was incomplete or did not match the requested document. Nothing was shown.";
export const SOURCE_SCOPE_ERROR =
  "The source response did not match this client. Nothing was shown.";
export const CLIENT_REQUIRED_ERROR =
  "A client is required before sources can be loaded.";
export const DOCUMENT_REQUIRED_ERROR = "A source document is required.";
export const LOAD_ERROR = "Knowledge could not be loaded.";
export const SOURCE_LOAD_ERROR = "The source could not be loaded.";
export const REFRESH_ERROR = "The knowledge index could not be refreshed.";

const UUID =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const RFC3339 =
  /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/;
const SHA256 = /^[0-9a-f]{64}$/i;

export type KnowledgeChunk = {
  id: string;
  document_id: string;
  source_kind: SourceKind;
  source_id: string;
  source_version: string;
  source_hash: string;
  title: string;
  text: string;
  source_date: string;
  episode_id: string | null;
  session_id: string | null;
  classification: string;
  ordinal: number;
};

export type KnowledgeDocument = {
  id: string;
  body: string;
  organization_id: string;
  client_id: string;
  source_kind: SourceKind;
  source_id: string;
  source_version: string;
  source_hash: string;
  title: string;
  source_date: string;
  episode_id: string | null;
  session_id: string | null;
  classification: string;
};

export type KnowledgeSearch = {
  client_id: string;
  indexed_at: string | null;
  document_count: number;
  chunk_count: number;
  total: number;
  limit: number;
  offset: number;
  results: KnowledgeChunk[];
};

export type KnowledgeSource = {
  document: KnowledgeDocument;
  chunks: KnowledgeChunk[];
};

export type KnowledgeFilters = {
  q: string;
  episodeId: string;
  kind: string;
  limit: number;
  offset: number;
};

export type NormalizedFilters = {
  q: string;
  episodeId: string;
  kind: SourceKind | "";
  limit: number;
  offset: number;
};

export type ParseResult<T> =
  | { ok: true; value: T }
  | { ok: false; error: string };

export type LoadResult<T> =
  | { status: "ready"; value: T }
  | { status: "stale" }
  | { status: "error"; error: string };

export type RefreshResult =
  | { status: "refreshed" }
  | { status: "stale" }
  | { status: "error"; error: string };

export type KnowledgeFetcher = (
  path: string,
  method?: string,
) => Promise<unknown>;

export type ResultToken = { generation: number; clientId: string };

export type ResultGuard = {
  begin(clientId: string): ResultToken;
  invalidate(nextClientId?: string): void;
  accept(token: ResultToken, currentClientId: string): boolean;
};

export type KnowledgeSelection = {
  clientId: string;
  documentId: string | null;
  chunkIds: string[];
};

export type PageWindow = {
  total: number;
  from: number;
  to: number;
  hasPrev: boolean;
  hasNext: boolean;
  prevOffset: number;
  nextOffset: number;
};


export function isSourceKind(value: string): value is SourceKind {
  return (SOURCE_KINDS as readonly string[]).includes(value);
}

export function sameId(a: string, b: string): boolean {
  return a.trim().toLowerCase() === b.trim().toLowerCase();
}

export function clampLimit(limit: number): number {
  if (!Number.isFinite(limit)) return DEFAULT_LIMIT;
  const n = Math.trunc(limit);
  if (n < 1) return DEFAULT_LIMIT;
  if (n > MAX_LIMIT) return MAX_LIMIT;
  return n;
}

export function clampOffset(offset: number): number {
  if (!Number.isFinite(offset)) return 0;
  return Math.max(0, Math.trunc(offset));
}

export function normalizeFilters(filters: KnowledgeFilters): NormalizedFilters {
  return {
    q: filters.q.trim(),
    episodeId: filters.episodeId.trim(),
    kind: isSourceKind(filters.kind) ? filters.kind : "",
    limit: clampLimit(filters.limit),
    offset: clampOffset(filters.offset),
  };
}

export function knowledgeSearchPath(
  clientId: string,
  filters: KnowledgeFilters,
): string {
  const normalized = normalizeFilters(filters);
  const params = new URLSearchParams();
  if (normalized.q) params.set("q", normalized.q);
  if (normalized.episodeId) params.set("episode_id", normalized.episodeId);
  if (normalized.kind) params.set("kind", normalized.kind);
  params.set("limit", String(normalized.limit));
  params.set("offset", String(normalized.offset));
  return `/api/clients/${encodeURIComponent(clientId)}/knowledge?${params.toString()}`;
}

export function knowledgeRefreshPath(clientId: string): string {
  return `/api/clients/${encodeURIComponent(clientId)}/knowledge`;
}

export function knowledgeSourcePath(clientId: string, documentId: string): string {
  return `/api/clients/${encodeURIComponent(clientId)}/knowledge/sources/${encodeURIComponent(documentId)}`;
}

export function isLexicalQuery(query: string): boolean {
  return query.trim().length > 0;
}

export function retrievalNotice(query: string): string {
  const q = query.trim();
  if (!q) {
    return "Browsing recent approved records.";
  }
  return `Word search for "${q}".`;
}

export function historicalCaption(formattedDate: string): string {
  return `Source date: ${formattedDate}`;
}

export function sourceKindLabel(kind: string): string {
  switch (kind) {
    case "note":
      return "Signed note";
    case "form_response":
      return "Signed form";
    case "treatment_plan":
      return "Treatment plan";
    case "plan_review":
      return "Plan review";
    case "transcript_excerpt":
      return "Transcript excerpt";
    default:
      return "Source";
  }
}

export function sourceTitle(title: string): string {
  const trimmed = title.trim();
  return trimmed || "Untitled source";
}

export function evidenceOrdinalLabel(ordinal: number): string {
  return `Excerpt ${ordinal + 1}`;
}

export function emptyKnowledgeCopy(input: {
  query: string;
  kind: string;
  episodeId: string;
}): { title: string; description: string } {
  const lexical = isLexicalQuery(input.query);
  const filtered = Boolean(input.kind || input.episodeId);
  if (lexical) {
    return {
      title: "No lexical matches",
      description: filtered
        ? "No approved extract matched those words with the current type and episode filters. Lexical search does not understand meaning. Clearing a filter may show other historical sources. Nothing was invented."
        : "No approved extract matched those words. Lexical search does not understand meaning, negation, or current status. Nothing was invented.",
    };
  }
  return {
    title: filtered
      ? "No approved sources in this filter"
      : "No approved sources indexed",
    description: filtered
      ? "The index has no eligible approved sources for this type or episode. This is not a statement that other chart history is absent."
      : "No eligible approved sources are indexed for this client. Refresh rebuilds from current eligible records and does not create clinical findings.",
  };
}

export function pageWindow(
  total: number,
  limit: number,
  offset: number,
): PageWindow {
  const safeLimit = clampLimit(limit);
  const safeOffset = clampOffset(offset);
  const safeTotal = Number.isInteger(total) && total > 0 ? total : 0;
  const prevOffset = Math.max(0, safeOffset - safeLimit);
  const nextOffset = safeOffset + safeLimit;
  if (safeTotal === 0 || safeOffset >= safeTotal) {
    return {
      total: safeTotal,
      from: 0,
      to: 0,
      hasPrev: safeOffset > 0,
      hasNext: false,
      prevOffset,
      nextOffset,
    };
  }
  return {
    total: safeTotal,
    from: safeOffset + 1,
    to: Math.min(safeOffset + safeLimit, safeTotal),
    hasPrev: safeOffset > 0,
    hasNext: nextOffset < safeTotal,
    prevOffset,
    nextOffset,
  };
}

export function pageLabel(total: number, limit: number, offset: number): string {
  const page = pageWindow(total, limit, offset);
  if (page.total === 0) return "No matching extracts";
  if (page.to === 0) {
    return `No extracts on this page. ${page.total} match the current view.`;
  }
  return `Showing ${page.from}–${page.to} of ${page.total}`;
}

export function pageMismatch(
  requested: { limit: number; offset: number },
  reported: { limit: number; offset: number },
): boolean {
  return (
    clampLimit(requested.limit) !== reported.limit ||
    clampOffset(requested.offset) !== reported.offset
  );
}

export function chunksInOrdinalOrder(chunks: KnowledgeChunk[]): KnowledgeChunk[] {
  return chunks
    .slice()
    .sort((a, b) => a.ordinal - b.ordinal || a.id.localeCompare(b.id));
}

export function createResultGuard(): ResultGuard {
  let generation = 0;
  let clientId = "";
  return {
    begin(nextClientId: string): ResultToken {
      generation += 1;
      clientId = nextClientId;
      return { generation, clientId: nextClientId };
    },
    invalidate(nextClientId = "") {
      generation += 1;
      clientId = nextClientId;
    },
    accept(token: ResultToken, currentClientId: string): boolean {
      return (
        token.generation === generation &&
        sameId(token.clientId, clientId) &&
        sameId(token.clientId, currentClientId)
      );
    },
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function fail<T>(error: string): ParseResult<T> {
  return { ok: false, error };
}

function readCount(row: Record<string, unknown>, key: string): number | null {
  const value = row[key];
  if (typeof value !== "number" || !Number.isInteger(value) || value < 0) {
    return null;
  }
  return value;
}

function readDate(value: unknown): string | null {
  if (typeof value !== "string" || !RFC3339.test(value)) return null;
  if (Number.isNaN(Date.parse(value))) return null;
  return value;
}

function readUuid(value: unknown): string | null {
  return typeof value === "string" && UUID.test(value) ? value : null;
}

function readNullableUuid(
  row: Record<string, unknown>,
  key: string,
): string | null | undefined {
  if (!Object.prototype.hasOwnProperty.call(row, key)) return undefined;
  const value = row[key];
  if (value === null) return null;
  return readUuid(value) ?? undefined;
}

function parseChunk(raw: unknown): ParseResult<KnowledgeChunk> {
  if (!isRecord(raw)) return fail(KNOWLEDGE_PARSE_ERROR);
  const id = readUuid(raw.id);
  const documentId = readUuid(raw.document_id);
  const sourceId = readUuid(raw.source_id);
  const sourceVersion = readUuid(raw.source_version);
  const sourceDate = readDate(raw.source_date);
  const episodeId = readNullableUuid(raw, "episode_id");
  const sessionId = readNullableUuid(raw, "session_id");
  const kind = raw.source_kind;
  const title = raw.title;
  const text = raw.text;
  const hash = raw.source_hash;
  const classification = raw.classification;
  const ordinal = raw.ordinal;
  if (
    !id ||
    !documentId ||
    !sourceId ||
    !sourceVersion ||
    !sourceDate ||
    episodeId === undefined ||
    sessionId === undefined ||
    typeof kind !== "string" ||
    !isSourceKind(kind) ||
    typeof title !== "string" ||
    typeof text !== "string" ||
    text.trim().length === 0 ||
    typeof hash !== "string" ||
    !SHA256.test(hash) ||
    typeof classification !== "string" ||
    classification.trim().length === 0 ||
    typeof ordinal !== "number" ||
    !Number.isInteger(ordinal) ||
    ordinal < 0
  ) {
    return fail(KNOWLEDGE_PARSE_ERROR);
  }
  return {
    ok: true,
    value: {
      id,
      document_id: documentId,
      source_kind: kind,
      source_id: sourceId,
      source_version: sourceVersion,
      source_hash: hash,
      title,
      text,
      source_date: sourceDate,
      episode_id: episodeId,
      session_id: sessionId,
      classification,
      ordinal,
    },
  };
}

function parseDocument(
  raw: Record<string, unknown>,
  clientId: string,
): ParseResult<KnowledgeDocument> {
  if (typeof raw.client_id === "string" && !sameId(raw.client_id, clientId)) {
    return fail(SOURCE_SCOPE_ERROR);
  }
  const id = readUuid(raw.id);
  const client = readUuid(raw.client_id);
  const organizationId = readUuid(raw.organization_id);
  const sourceId = readUuid(raw.source_id);
  const sourceVersion = readUuid(raw.source_version);
  const sourceDate = readDate(raw.source_date);
  const episodeId = readNullableUuid(raw, "episode_id");
  const sessionId = readNullableUuid(raw, "session_id");
  const kind = raw.source_kind;
  if (
    !id ||
    !client ||
    !organizationId ||
    !sourceId ||
    !sourceVersion ||
    !sourceDate ||
    episodeId === undefined ||
    sessionId === undefined ||
    typeof kind !== "string" ||
    !isSourceKind(kind) ||
    typeof raw.title !== "string" ||
    typeof raw.body !== "string" ||
    typeof raw.source_hash !== "string" ||
    !SHA256.test(raw.source_hash) ||
    typeof raw.classification !== "string" ||
    raw.classification.trim().length === 0 ||
    !sameId(client, clientId)
  ) {
    return fail(SOURCE_PARSE_ERROR);
  }
  return {
    ok: true,
    value: {
      id,
      body: raw.body,
      organization_id: organizationId,
      client_id: client,
      source_kind: kind,
      source_id: sourceId,
      source_version: sourceVersion,
      source_hash: raw.source_hash,
      title: raw.title,
      source_date: sourceDate,
      episode_id: episodeId,
      session_id: sessionId,
      classification: raw.classification,
    },
  };
}

export function parseKnowledgeSearch(
  raw: unknown,
  clientId: string,
): ParseResult<KnowledgeSearch> {
  if (!isRecord(raw)) return fail(KNOWLEDGE_PARSE_ERROR);
  if (typeof raw.client_id === "string" && !sameId(raw.client_id, clientId)) {
    return fail(KNOWLEDGE_SCOPE_ERROR);
  }
  const client = readUuid(raw.client_id);
  const documentCount = readCount(raw, "document_count");
  const chunkCount = readCount(raw, "chunk_count");
  const total = readCount(raw, "total");
  const limit = readCount(raw, "limit");
  const offset = readCount(raw, "offset");
  const indexedAt =
    raw.indexed_at === null ? null : readDate(raw.indexed_at);
  if (
    !client ||
    !sameId(client, clientId) ||
    documentCount === null ||
    chunkCount === null ||
    total === null ||
    limit === null ||
    limit > MAX_LIMIT ||
    offset === null ||
    indexedAt === null && raw.indexed_at !== null ||
    !Array.isArray(raw.results)
  ) {
    return fail(KNOWLEDGE_PARSE_ERROR);
  }
  if (raw.results.length > total || raw.results.length > limit || raw.results.length > MAX_LIMIT) {
    return fail(KNOWLEDGE_PARSE_ERROR);
  }
  const results: KnowledgeChunk[] = [];
  const seen = new Set<string>();
  for (const item of raw.results) {
    const parsed = parseChunk(item);
    if (!parsed.ok) return fail(KNOWLEDGE_PARSE_ERROR);
    const key = parsed.value.id.toLowerCase();
    if (seen.has(key)) return fail(KNOWLEDGE_PARSE_ERROR);
    seen.add(key);
    results.push(parsed.value);
  }
  return {
    ok: true,
    value: {
      client_id: client,
      indexed_at: indexedAt,
      document_count: documentCount,
      chunk_count: chunkCount,
      total,
      limit,
      offset,
      results,
    },
  };
}

export function parseKnowledgeSource(
  raw: unknown,
  clientId: string,
  documentId: string,
): ParseResult<KnowledgeSource> {
  if (!isRecord(raw) || !isRecord(raw.document) || !Array.isArray(raw.chunks)) {
    return fail(SOURCE_PARSE_ERROR);
  }
  if (
    typeof raw.document.client_id === "string" &&
    !sameId(raw.document.client_id, clientId)
  ) {
    return fail(SOURCE_SCOPE_ERROR);
  }
  const document = parseDocument(raw.document, clientId);
  if (!document.ok) return document;
  if (!sameId(document.value.id, documentId)) return fail(SOURCE_PARSE_ERROR);
  const chunks: KnowledgeChunk[] = [];
  const seen = new Set<string>();
  for (const item of raw.chunks) {
    const parsed = parseChunk(item);
    if (!parsed.ok) return fail(SOURCE_PARSE_ERROR);
    if (!sameId(parsed.value.document_id, document.value.id)) {
      return fail(SOURCE_PARSE_ERROR);
    }
    const key = parsed.value.id.toLowerCase();
    if (seen.has(key)) return fail(SOURCE_PARSE_ERROR);
    seen.add(key);
    chunks.push(parsed.value);
  }
  return { ok: true, value: { document: document.value, chunks } };
}

function errorMessage(err: unknown, fallback: string): string {
  return err instanceof Error && err.message ? err.message : fallback;
}

function scopeClosed(
  token: ResultToken,
  guard: ResultGuard,
  currentClient: () => string,
  clientId: string,
): boolean {
  return !guard.accept(token, currentClient()) || !sameId(currentClient(), clientId);
}

export async function loadKnowledgeSearch(
  fetcher: KnowledgeFetcher,
  clientId: string,
  filters: KnowledgeFilters,
  token: ResultToken,
  guard: ResultGuard,
  currentClient: () => string,
): Promise<LoadResult<KnowledgeSearch>> {
  if (!clientId.trim()) return { status: "error", error: CLIENT_REQUIRED_ERROR };
  if (scopeClosed(token, guard, currentClient, clientId)) return { status: "stale" };
  try {
    const raw = await fetcher(knowledgeSearchPath(clientId, filters), "GET");
    if (scopeClosed(token, guard, currentClient, clientId)) return { status: "stale" };
    const parsed = parseKnowledgeSearch(raw, clientId);
    if (!parsed.ok) return { status: "error", error: parsed.error };
    return { status: "ready", value: parsed.value };
  } catch (err) {
    if (scopeClosed(token, guard, currentClient, clientId)) return { status: "stale" };
    return { status: "error", error: errorMessage(err, LOAD_ERROR) };
  }
}

export async function loadKnowledgeSource(
  fetcher: KnowledgeFetcher,
  clientId: string,
  documentId: string,
  token: ResultToken,
  guard: ResultGuard,
  currentClient: () => string,
): Promise<LoadResult<KnowledgeSource>> {
  if (!clientId.trim()) return { status: "error", error: CLIENT_REQUIRED_ERROR };
  if (!documentId.trim()) return { status: "error", error: DOCUMENT_REQUIRED_ERROR };
  if (scopeClosed(token, guard, currentClient, clientId)) return { status: "stale" };
  try {
    const raw = await fetcher(knowledgeSourcePath(clientId, documentId), "GET");
    if (scopeClosed(token, guard, currentClient, clientId)) return { status: "stale" };
    const parsed = parseKnowledgeSource(raw, clientId, documentId);
    if (!parsed.ok) return { status: "error", error: parsed.error };
    return { status: "ready", value: parsed.value };
  } catch (err) {
    if (scopeClosed(token, guard, currentClient, clientId)) return { status: "stale" };
    return { status: "error", error: errorMessage(err, SOURCE_LOAD_ERROR) };
  }
}

export async function refreshKnowledge(
  fetcher: KnowledgeFetcher,
  clientId: string,
  token: ResultToken,
  guard: ResultGuard,
  currentClient: () => string,
): Promise<RefreshResult> {
  if (!clientId.trim()) return { status: "error", error: CLIENT_REQUIRED_ERROR };
  if (scopeClosed(token, guard, currentClient, clientId)) return { status: "stale" };
  try {
    await fetcher(knowledgeRefreshPath(clientId), "POST");
    if (scopeClosed(token, guard, currentClient, clientId)) return { status: "stale" };
    return { status: "refreshed" };
  } catch (err) {
    if (scopeClosed(token, guard, currentClient, clientId)) return { status: "stale" };
    return { status: "error", error: errorMessage(err, REFRESH_ERROR) };
  }
}
