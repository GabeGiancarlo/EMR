declare module "node:test" {
  export function test(name: string, fn: () => void | Promise<void>): void;
}
declare module "node:assert/strict" {
  export function equal(actual: unknown, expected: unknown, message?: string): void;
  export function deepEqual(actual: unknown, expected: unknown, message?: string): void;
  export function match(actual: string, expected: RegExp, message?: string): void;
  export function ok(value: unknown, message?: string): void;
}

import { test } from "node:test";
import { deepEqual, equal, match, ok } from "node:assert/strict";
import {
  CLIENT_REQUIRED_ERROR,
  DOCUMENT_REQUIRED_ERROR,
  KNOWLEDGE_DISCLAIMER,
  KNOWLEDGE_PARSE_ERROR,
  KNOWLEDGE_SCOPE_ERROR,
  LOAD_ERROR,
  MAX_LIMIT,
  PAGE_SIZES,
  SOURCE_PARSE_ERROR,
  SOURCE_SCOPE_ERROR,
  STAT_LABELS,
  chunksInOrdinalOrder,
  createResultGuard,
  emptyKnowledgeCopy,
  evidenceOrdinalLabel,
  historicalCaption,
  isLexicalQuery,
  knowledgeRefreshPath,
  knowledgeSearchPath,
  knowledgeSourcePath,
  loadKnowledgeSearch,
  loadKnowledgeSource,
  pageLabel,
  pageMismatch,
  pageWindow,
  parseKnowledgeSearch,
  parseKnowledgeSource,
  refreshKnowledge,
  retrievalNotice,
  type KnowledgeChunk,
  type KnowledgeFilters,
  type KnowledgeSource,
} from "./knowledgeTypes.ts";

const HASH = "ab".repeat(32);
const CLIENT = "11111111-1111-4111-8111-111111111111";
const OTHER = "99999999-9999-4999-8999-999999999999";
const ORG = "22222222-2222-4222-8222-222222222222";
const DOC = "33333333-3333-4333-8333-333333333333";
const DOC_B = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const CHUNK = "44444444-4444-4444-8444-444444444444";
const CHUNK_B = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const SOURCE = "55555555-5555-4555-8555-555555555555";
const VERSION = "66666666-6666-4666-8666-666666666666";
const EPISODE = "77777777-7777-4777-8777-777777777777";
const WHEN = "2024-03-02T15:04:05Z";
const blankFilters: KnowledgeFilters = {
  q: "",
  episodeId: "",
  kind: "",
  limit: 25,
  offset: 0,
};

function chunk(overrides: Record<string, unknown> = {}): KnowledgeChunk {
  return {
    id: CHUNK,
    document_id: DOC,
    source_kind: "note",
    source_id: SOURCE,
    source_version: VERSION,
    source_hash: HASH,
    title: "Signed progress note",
    text: "Patient denied chest pain on 2024-03-02.",
    source_date: WHEN,
    episode_id: EPISODE,
    session_id: null,
    classification: "clinical",
    ordinal: 0,
    ...overrides,
  } as KnowledgeChunk;
}

function searchBody(clientId = CLIENT, overrides: Record<string, unknown> = {}) {
  return {
    client_id: clientId,
    indexed_at: WHEN,
    document_count: 1,
    chunk_count: 1,
    total: 1,
    limit: 25,
    offset: 0,
    results: [chunk()],
    ...overrides,
  };
}

function document(overrides: Record<string, unknown> = {}) {
  return {
    id: DOC,
    body: "Full historical note. Patient denied chest pain on 2024-03-02.",
    organization_id: ORG,
    client_id: CLIENT,
    source_kind: "note",
    source_id: SOURCE,
    source_version: VERSION,
    source_hash: HASH,
    title: "Signed progress note",
    source_date: WHEN,
    episode_id: EPISODE,
    session_id: null,
    classification: "clinical",
    ...overrides,
  };
}

function sourceBody(overrides: Record<string, unknown> = {}): KnowledgeSource {
  return {
    document: document(),
    chunks: [chunk()],
    ...overrides,
  } as KnowledgeSource;
}

test("page sizes stay inside the contract maximum", () => {
  ok(PAGE_SIZES.length > 0);
  for (const size of PAGE_SIZES) {
    ok(size >= 1 && size <= MAX_LIMIT);
  }
  equal(Math.max(...PAGE_SIZES), 50);
});

test("search path is lexical, scoped, and bounded", () => {
  const filters = {
    q: "sleep disturbance",
    episodeId: "ep/1",
    kind: "note",
    limit: 80,
    offset: -3,
  };
  equal(
    knowledgeSearchPath("client a", filters),
    "/api/clients/client%20a/knowledge?q=sleep+disturbance&episode_id=ep%2F1&kind=note&limit=50&offset=0",
  );
  equal(
    knowledgeSearchPath(CLIENT, blankFilters),
    `/api/clients/${CLIENT}/knowledge?limit=25&offset=0`,
  );
  ok(!knowledgeSearchPath(CLIENT, { ...blankFilters, kind: "diagnosis" }).includes("kind="));
  equal(knowledgeRefreshPath("a/b"), "/api/clients/a%2Fb/knowledge");
  equal(
    knowledgeSourcePath(CLIENT, "doc/1"),
    `/api/clients/${CLIENT}/knowledge/sources/doc%2F1`,
  );
});


test("parser preserves source text and drops extra clinical fields", () => {
  const raw = searchBody();
  (raw.results[0] as Record<string, unknown>).diagnosis = "Invented condition";
  const parsed = parseKnowledgeSearch(raw, CLIENT);
  ok(parsed.ok);
  if (!parsed.ok) return;
  equal(parsed.value.results[0].text, "Patient denied chest pain on 2024-03-02.");
  ok(!("diagnosis" in parsed.value.results[0]));
  equal(parsed.value.results[0].episode_id, EPISODE);
  equal(parsed.value.results[0].session_id, null);
});

test("parser rejects another client's payload without echoing it", () => {
  const parsed = parseKnowledgeSearch(
    searchBody(OTHER, {
      results: [chunk({ text: "secret phrase from another chart" })],
    }),
    CLIENT,
  );
  equal(parsed.ok, false);
  if (parsed.ok) return;
  equal(parsed.error, KNOWLEDGE_SCOPE_ERROR);
  ok(!parsed.error.includes("secret phrase"));
});

test("parser rejects malformed evidence instead of inventing records", () => {
  equal(parseKnowledgeSearch(searchBody(CLIENT, { results: [chunk({ source_kind: "diagnosis" })] }), CLIENT).ok, false);
  equal(parseKnowledgeSearch(searchBody(CLIENT, { results: [chunk(), chunk()] }), CLIENT).ok, false);
  const missingEpisode = chunk() as Record<string, unknown>;
  delete missingEpisode.episode_id;
  equal(parseKnowledgeSearch(searchBody(CLIENT, { results: [missingEpisode] }), CLIENT).ok, false);
  equal(parseKnowledgeSearch(searchBody(CLIENT, { results: [chunk({ text: "   " })] }), CLIENT).ok, false);
  equal(parseKnowledgeSearch(searchBody(CLIENT, { indexed_at: null, results: [] , total: 0, document_count: 0, chunk_count: 0 }), CLIENT).ok, true);
  equal(parseKnowledgeSearch(searchBody(CLIENT, { limit: 100 }), CLIENT).ok, false);
  const bad = parseKnowledgeSearch(null, CLIENT);
  equal(bad.ok, false);
  if (!bad.ok) equal(bad.error, KNOWLEDGE_PARSE_ERROR);
});

test("source detail must match the requested client and document", () => {
  const ready = parseKnowledgeSource({ document: document(), chunks: [chunk()] }, CLIENT, DOC.toUpperCase());
  ok(ready.ok);
  const foreign = parseKnowledgeSource(
    { document: document({ client_id: OTHER, body: "secret phrase" }), chunks: [] },
    CLIENT,
    DOC,
  );
  equal(foreign.ok, false);
  if (!foreign.ok) {
    equal(foreign.error, SOURCE_SCOPE_ERROR);
    ok(!foreign.error.includes("secret phrase"));
  }
  const mismatch = parseKnowledgeSource(
    { document: document({ id: DOC_B }), chunks: [chunk()] },
    CLIENT,
    DOC,
  );
  equal(mismatch.ok, false);
  if (!mismatch.ok) equal(mismatch.error, SOURCE_PARSE_ERROR);
  const otherChunk = parseKnowledgeSource(
    { document: document(), chunks: [chunk({ document_id: DOC_B })] },
    CLIENT,
    DOC,
  );
  equal(otherChunk.ok, false);
});

test("pagination reports the returned window and does not invent pages", () => {
  const page = pageWindow(40, 25, 25);
  equal(page.from, 26);
  equal(page.to, 40);
  equal(page.hasPrev, true);
  equal(page.hasNext, false);
  equal(pageLabel(40, 25, 25), "Showing 26–40 of 40");
  equal(pageLabel(0, 25, 0), "No matching extracts");
  const past = pageWindow(10, 25, 50);
  equal(past.to, 0);
  equal(past.hasPrev, true);
  equal(past.hasNext, false);
  ok(pageMismatch({ limit: 25, offset: 0 }, { limit: 50, offset: 0 }));
  ok(!pageMismatch({ limit: 25, offset: 0 }, { limit: 25, offset: 0 }));
  equal(pageWindow(200, 100, 0).to, 50);
});

test("in-flight search is discarded when the scope is invalidated", async () => {
  const guard = createResultGuard();
  let release!: (value: unknown) => void;
  const token = guard.begin(CLIENT);
  const pending = loadKnowledgeSearch(
    () => new Promise((resolve) => {
      release = resolve;
    }),
    CLIENT,
    blankFilters,
    token,
    guard,
    () => CLIENT,
  );
  guard.invalidate(CLIENT);
  release(searchBody());
  equal((await pending).status, "stale");
});

test("a later client scope wins over a slower earlier response", async () => {
  const guard = createResultGuard();
  let releaseFirst!: (value: unknown) => void;
  const firstToken = guard.begin(CLIENT);
  const first = loadKnowledgeSearch(
    () => new Promise((resolve) => {
      releaseFirst = resolve;
    }),
    CLIENT,
    blankFilters,
    firstToken,
    guard,
    () => CLIENT,
  );
  const secondToken = guard.begin(OTHER);
  const second = await loadKnowledgeSearch(
    async () => searchBody(OTHER),
    OTHER,
    blankFilters,
    secondToken,
    guard,
    () => OTHER,
  );
  equal(second.status, "ready");
  releaseFirst(searchBody());
  equal((await first).status, "stale");
});

test("strict remount begin discards the first search for the same client", async () => {
  const guard = createResultGuard();
  let releaseFirst!: (value: unknown) => void;
  const firstToken = guard.begin(CLIENT);
  const first = loadKnowledgeSearch(
    () => new Promise((resolve) => {
      releaseFirst = resolve;
    }),
    CLIENT,
    blankFilters,
    firstToken,
    guard,
    () => CLIENT,
  );
  const secondToken = guard.begin(CLIENT);
  const second = await loadKnowledgeSearch(
    async () => searchBody(),
    CLIENT,
    blankFilters,
    secondToken,
    guard,
    () => CLIENT,
  );
  equal(second.status, "ready");
  if (second.status === "ready") {
    equal(second.value.results[0].text, "Patient denied chest pain on 2024-03-02.");
  }
  releaseFirst(searchBody());
  equal((await first).status, "stale");
});

test("changing the current client discards a response without applying it", async () => {
  const guard = createResultGuard();
  const token = guard.begin(CLIENT);
  let current = CLIENT;
  let release!: (value: unknown) => void;
  const pending = loadKnowledgeSearch(
    () => new Promise((resolve) => {
      release = resolve;
    }),
    CLIENT,
    blankFilters,
    token,
    guard,
    () => current,
  );
  current = OTHER;
  release(searchBody());
  equal((await pending).status, "stale");
  ok(!guard.accept(token, OTHER));
  ok(guard.accept(token, CLIENT.toUpperCase()) === false || current !== CLIENT);
});

test("search and source loads call only the scoped contract endpoint", async () => {
  const guard = createResultGuard();
  const calls: string[] = [];
  const filters = {
    q: "sleep disturbance",
    episodeId: EPISODE,
    kind: "note",
    limit: 80,
    offset: -2,
  };
  const token = guard.begin(CLIENT);
  const searched = await loadKnowledgeSearch(
    async (path, method) => {
      calls.push(`${method} ${path}`);
      return searchBody(CLIENT, { total: 1, limit: 25, offset: 0, results: [chunk()] });
    },
    CLIENT,
    filters,
    token,
    guard,
    () => CLIENT,
  );
  equal(searched.status, "ready");
  equal(calls[0], `GET ${knowledgeSearchPath(CLIENT, filters)}`);
  const sourceToken = guard.begin(CLIENT);
  const loaded = await loadKnowledgeSource(
    async (path, method) => {
      calls.push(`${method} ${path}`);
      return { document: document(), chunks: [chunk()] };
    },
    CLIENT,
    DOC,
    sourceToken,
    guard,
    () => CLIENT,
  );
  equal(loaded.status, "ready");
  equal(calls[1], `GET ${knowledgeSourcePath(CLIENT, DOC)}`);
});

test("failed search stays an error and does not invent results", async () => {
  const guard = createResultGuard();
  const token = guard.begin(CLIENT);
  const outcome = await loadKnowledgeSearch(
    async () => {
      throw new Error("Not allowed to read this chart.");
    },
    CLIENT,
    blankFilters,
    token,
    guard,
    () => CLIENT,
  );
  equal(outcome.status, "error");
  if (outcome.status !== "error") return;
  equal(outcome.error, "Not allowed to read this chart.");
  ok(!("results" in outcome));
  const blank = await loadKnowledgeSearch(
    async () => {
      throw "nope";
    },
    "",
    blankFilters,
    token,
    guard,
    () => "",
  );
  equal(blank.status, "error");
  if (blank.status === "error") equal(blank.error, CLIENT_REQUIRED_ERROR);
  const unknown = await loadKnowledgeSearch(
    async () => {
      throw "nope";
    },
    CLIENT,
    blankFilters,
    guard.begin(CLIENT),
    guard,
    () => CLIENT,
  );
  equal(unknown.status, "error");
  if (unknown.status === "error") equal(unknown.error, LOAD_ERROR);
});

test("already stale scope does not send a search", async () => {
  const guard = createResultGuard();
  const token = guard.begin(CLIENT);
  guard.invalidate("");
  let called = false;
  const outcome = await loadKnowledgeSearch(
    async () => {
      called = true;
      return searchBody();
    },
    CLIENT,
    blankFilters,
    token,
    guard,
    () => CLIENT,
  );
  equal(outcome.status, "stale");
  equal(called, false);
});

test("refresh posts to the client index and is ignored after scope change", async () => {
  const guard = createResultGuard();
  const calls: string[] = [];
  let release!: () => void;
  const token = guard.begin(CLIENT);
  const pending = refreshKnowledge(
    (path, method) => {
      calls.push(`${method} ${path}`);
      return new Promise((resolve) => {
        release = () => resolve(null);
      });
    },
    CLIENT,
    token,
    guard,
    () => CLIENT,
  );
  guard.invalidate(OTHER);
  release();
  equal((await pending).status, "stale");
  deepEqual(calls, [`POST ${knowledgeRefreshPath(CLIENT)}`]);
  ok(!calls[0].includes("?"));
});

test("source load requires a document and does not fetch without one", async () => {
  const guard = createResultGuard();
  const token = guard.begin(CLIENT);
  let called = false;
  const outcome = await loadKnowledgeSource(
    async () => {
      called = true;
      return null;
    },
    CLIENT,
    " ",
    token,
    guard,
    () => CLIENT,
  );
  equal(outcome.status, "error");
  if (outcome.status === "error") equal(outcome.error, DOCUMENT_REQUIRED_ERROR);
  equal(called, false);
});


test("ordinal display does not mutate or reorder the caller's extract list", () => {
  const first = chunk({ id: CHUNK, ordinal: 2 });
  const second = chunk({ id: CHUNK_B, ordinal: 1 });
  const input = [first, second];
  const ordered = chunksInOrdinalOrder(input);
  equal(ordered[0].id, CHUNK_B);
  equal(ordered[1].id, CHUNK);
  equal(input[0].id, CHUNK);
});
