import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import { date, label, request, value } from "./api";
import type { Bootstrap, Row } from "./api";
import ClinicalDrafting from "./ClinicalDrafting";
import { Button, Empty, Icon, Panel } from "./ui";
import {
  CLIENT_REQUIRED_ERROR,
  DEFAULT_LIMIT,
  KNOWLEDGE_DISCLAIMER,
  PAGE_SIZES,
  SOURCE_KINDS,
  chunksInOrdinalOrder,
  createResultGuard,
  emptyKnowledgeCopy,
  evidenceOrdinalLabel,
  historicalCaption,
  isLexicalQuery,
  loadKnowledgeSearch,
  loadKnowledgeSource,
  pageLabel,
  pageMismatch,
  pageWindow,
  refreshKnowledge,
  retrievalNotice,
  sourceKindLabel,
  sourceTitle,
  type KnowledgeSearch,
  type KnowledgeSource,
  type KnowledgeChunk,
} from "./knowledgeTypes";
import "./knowledge.css";

export type PatientKnowledgeProps = {
  clientId: string;
  data: Bootstrap;
  onOpenSource?: (kind: string, id: string) => void;
  onNoteCreated: (note: Row) => void | Promise<void>;
  onReviewDirty: (dirty: boolean) => void;
};

function workspaceEpisode(
  data: Bootstrap,
  clientId: string,
  episodeId: string | null,
): string {
  if (!episodeId) return "No episode linked on the source";
  const episode = data.episodes.find(
    (row) => row.id === episodeId && row.client_id === clientId,
  );
  if (!episode) return "Episode link is not in the current workspace snapshot";
  const name = label(value(episode, "level_of_care")) || "Episode";
  return `Workspace episode: ${name} · started ${date(episode.start_date)}`;
}

function workspaceSession(
  data: Bootstrap,
  clientId: string,
  sessionId: string | null,
): string {
  if (!sessionId) return "No session linked on the source";
  const session = data.sessions.find(
    (row) => row.id === sessionId && row.client_id === clientId,
  );
  if (!session) return "Session link is not in the current workspace snapshot";
  return `Workspace session: ${label(value(session, "kind")) || "Session"} · ${date(session.starts_at, true)}`;
}

export default function PatientKnowledge({
  clientId,
  data,
  onOpenSource,
  onNoteCreated,
  onReviewDirty,
}: PatientKnowledgeProps) {
  const [selectedEvidence, setSelectedEvidence] = useState<KnowledgeChunk[]>(
    [],
  );
  const [scopedClient, setScopedClient] = useState(clientId);
  const [draftQuery, setDraftQuery] = useState("");
  const [appliedQuery, setAppliedQuery] = useState("");
  const [kind, setKind] = useState("");
  const [episodeId, setEpisodeId] = useState("");
  const [limit, setLimit] = useState<number>(DEFAULT_LIMIT);
  const [offset, setOffset] = useState(0);
  const [reloadNonce, setReloadNonce] = useState(0);
  const [detailNonce, setDetailNonce] = useState(0);
  const [search, setSearch] = useState<KnowledgeSearch | null>(null);
  const [detail, setDetail] = useState<KnowledgeSource | null>(null);
  const [activeDocumentId, setActiveDocumentId] = useState<string | null>(null);
  const [listError, setListError] = useState("");
  const [detailError, setDetailError] = useState("");
  const [listLoading, setListLoading] = useState(Boolean(clientId));
  const [detailLoading, setDetailLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const listGuard = useRef(createResultGuard());
  const detailGuard = useRef(createResultGuard());
  const refreshGuard = useRef(createResultGuard());
  const clientRef = useRef(clientId);
  const detailRef = useRef<HTMLElement | null>(null);
  clientRef.current = clientId;

  if (scopedClient !== clientId) {
    setSelectedEvidence([]);
    listGuard.current.invalidate(clientId);
    detailGuard.current.invalidate(clientId);
    refreshGuard.current.invalidate(clientId);
    setScopedClient(clientId);
    setDraftQuery("");
    setAppliedQuery("");
    setKind("");
    setEpisodeId("");
    setLimit(DEFAULT_LIMIT);
    setOffset(0);
    setSearch(null);
    setDetail(null);
    setActiveDocumentId(null);
    setListError("");
    setDetailError("");
    setListLoading(Boolean(clientId));
    setDetailLoading(false);
    setRefreshing(false);
  }

  useEffect(() => {
    return () => {
      refreshGuard.current.invalidate("");
    };
  }, [clientId]);

  useEffect(() => {
    if (!clientId) return;
    const token = listGuard.current.begin(clientId);
    const filters = { q: appliedQuery, episodeId, kind, limit, offset };
    setListLoading(true);
    setListError("");
    let active = true;
    void loadKnowledgeSearch(
      (path, method) => request(path, method),
      clientId,
      filters,
      token,
      listGuard.current,
      () => clientRef.current,
    ).then((outcome) => {
      if (!active || outcome.status === "stale") return;
      if (outcome.status === "error") {
        setSearch(null);
        setListError(outcome.error);
        setListLoading(false);
        return;
      }
      setSearch(outcome.value);
      setListLoading(false);
    });
    return () => {
      active = false;
      listGuard.current.invalidate(clientId);
    };
  }, [clientId, appliedQuery, episodeId, kind, limit, offset, reloadNonce]);

  useEffect(() => {
    if (!clientId || !activeDocumentId) return;
    const token = detailGuard.current.begin(clientId);
    const documentId = activeDocumentId;
    setDetail(null);
    setDetailLoading(true);
    setDetailError("");
    let active = true;
    void loadKnowledgeSource(
      (path, method) => request(path, method),
      clientId,
      documentId,
      token,
      detailGuard.current,
      () => clientRef.current,
    ).then((outcome) => {
      if (!active || outcome.status === "stale") return;
      if (outcome.status === "error") {
        setDetail(null);
        setDetailError(outcome.error);
        setDetailLoading(false);
        return;
      }
      setDetail(outcome.value);
      setDetailLoading(false);
    });
    return () => {
      active = false;
      detailGuard.current.invalidate(clientId);
    };
  }, [clientId, activeDocumentId, reloadNonce, detailNonce]);

  function applySearch(event: FormEvent) {
    event.preventDefault();
    setAppliedQuery(draftQuery.trim());
    setOffset(0);
    setSearch(null);
    setReloadNonce((n) => n + 1);
  }

  function clearFilters() {
    setDraftQuery("");
    setAppliedQuery("");
    setKind("");
    setEpisodeId("");
    setOffset(0);
    setSearch(null);
    setReloadNonce((n) => n + 1);
  }

  function goTo(nextOffset: number) {
    setOffset(nextOffset);
    setSearch(null);
  }

  async function onRefresh() {
    if (!clientId) return;
    const requestedClient = clientId;
    const token = refreshGuard.current.begin(requestedClient);
    setRefreshing(true);
    setListError("");
    const outcome = await refreshKnowledge(
      (path, method) => request(path, method),
      requestedClient,
      token,
      refreshGuard.current,
      () => clientRef.current,
    );
    if (outcome.status === "stale" || clientRef.current !== requestedClient)
      return;
    setRefreshing(false);
    if (outcome.status === "error") {
      setListError(outcome.error);
      return;
    }
    setSearch(null);
    setDetail(null);
    setReloadNonce((n) => n + 1);
  }

  function openSource(documentId: string) {
    setDetail(null);
    setDetailError("");
    setActiveDocumentId(documentId);
    setDetailNonce((n) => n + 1);
    queueMicrotask(() => {
      detailRef.current?.scrollIntoView({ block: "nearest" });
    });
  }

  const fieldPrefix = `knowledge-${clientId.replace(/[^A-Za-z0-9_-]/g, "") || "client"}`;
  const episodes = data.episodes
    .filter((row) => row.client_id === clientId)
    .slice()
    .sort((a, b) =>
      value(b, "start_date").localeCompare(value(a, "start_date")),
    );
  const visibleSearch =
    search && search.client_id.toLowerCase() === clientId.toLowerCase()
      ? search
      : null;
  const visibleDetail =
    detail && detail.document.client_id.toLowerCase() === clientId.toLowerCase()
      ? detail
      : null;
  const emptyCopy = emptyKnowledgeCopy({
    query: appliedQuery,
    kind,
    episodeId,
  });
  const filtersActive = Boolean(
    appliedQuery || draftQuery || kind || episodeId,
  );

  if (!clientId) {
    return (
      <Panel
        className="knowledge-panel"
        title="Source-linked knowledge"
        subtitle="A client chart is required before approved sources can be loaded."
      >
        <div className="knowledge-content">
          <p className="knowledge-disclaimer">{KNOWLEDGE_DISCLAIMER}</p>
          <div className="error-banner" role="alert">
            {CLIENT_REQUIRED_ERROR}
          </div>
        </div>
      </Panel>
    );
  }

  return (
    <Panel
      className="knowledge-panel"
      title="Patient knowledge"
      subtitle="Find approved records, review their context, and select evidence for documentation."
      action={
        <Button
          variant="secondary small"
          className="knowledge-refresh"
          disabled={refreshing}
          onClick={() => void onRefresh()}
        >
          <span className={refreshing ? "knowledge-spin" : undefined}>
            <Icon name="refresh" size={16} />
          </span>
          {refreshing ? "Refreshing…" : "Refresh sources"}
        </Button>
      }
    >
      <div className="knowledge-content" data-knowledge-client={clientId}>
        <p className="knowledge-disclaimer">{KNOWLEDGE_DISCLAIMER}</p>
        <p className="knowledge-mode" role="status">
          {retrievalNotice(appliedQuery)}
        </p>
        <form
          className="knowledge-filters"
          role="search"
          onSubmit={applySearch}
        >
          <div className="form-field">
            <label htmlFor={`${fieldPrefix}-q`}>Search approved records</label>
            <input
              id={`${fieldPrefix}-q`}
              value={draftQuery}
              autoComplete="off"
              placeholder="Words in approved sources"
              onChange={(event) => setDraftQuery(event.target.value)}
            />
            <small>Search by words or a quoted phrase.</small>
          </div>
          <div className="form-field">
            <label htmlFor={`${fieldPrefix}-kind`}>Source kind</label>
            <select
              id={`${fieldPrefix}-kind`}
              value={kind}
              onChange={(event) => {
                setKind(event.target.value);
                setOffset(0);
                setSearch(null);
              }}
            >
              <option value="">All source kinds</option>
              {SOURCE_KINDS.map((sourceKind) => (
                <option key={sourceKind} value={sourceKind}>
                  {sourceKindLabel(sourceKind)}
                </option>
              ))}
            </select>
          </div>
          <div className="form-field">
            <label htmlFor={`${fieldPrefix}-episode`}>Episode</label>
            <select
              id={`${fieldPrefix}-episode`}
              value={episodeId}
              onChange={(event) => {
                setEpisodeId(event.target.value);
                setOffset(0);
                setSearch(null);
              }}
            >
              <option value="">All episodes</option>
              {episodes.map((episode) => (
                <option key={episode.id} value={episode.id}>
                  {label(value(episode, "level_of_care")) || "Episode"} ·{" "}
                  {date(episode.start_date)}
                </option>
              ))}
            </select>
          </div>
          <div className="form-field">
            <label htmlFor={`${fieldPrefix}-limit`}>Page size</label>
            <select
              id={`${fieldPrefix}-limit`}
              value={String(limit)}
              onChange={(event) => {
                setLimit(Number(event.target.value));
                setOffset(0);
                setSearch(null);
              }}
            >
              {PAGE_SIZES.map((size) => (
                <option key={size} value={size}>
                  {size}
                </option>
              ))}
            </select>
            <small>At most 50 extracts per page.</small>
          </div>
          <div className="knowledge-filter-actions">
            <Button type="submit">
              <Icon name="search" size={16} /> Search
            </Button>
            {filtersActive && (
              <Button variant="text small" onClick={clearFilters}>
                Clear filters
              </Button>
            )}
          </div>
        </form>

        {listError && (
          <div className="error-banner" role="alert">
            {listError}
          </div>
        )}

        {listLoading && !visibleSearch && (
          <div className="knowledge-loading" role="status">
            <div className="spinner" />
            <p>Loading approved sources for this client…</p>
          </div>
        )}

        {visibleSearch && (
          <>
            <p className="knowledge-index-note">
              {visibleSearch.indexed_at
                ? `Sources checked ${date(visibleSearch.indexed_at, true)}.`
                : "Refresh to check approved sources."}
            </p>
            <p className="knowledge-summary">
              {visibleSearch.document_count} approved sources ·{" "}
              {visibleSearch.total} matching excerpts
            </p>
          </>
        )}

        <div className="knowledge-layout">
          <div className="knowledge-results" aria-busy={listLoading}>
            {visibleSearch &&
              visibleSearch.results.length > 0 &&
              visibleSearch.results.map((chunk) => {
                const active = activeDocumentId === chunk.document_id;
                return (
                  <article
                    key={chunk.id}
                    className={
                      active ? "knowledge-card active" : "knowledge-card"
                    }
                    aria-current={active ? "true" : undefined}
                  >
                    <div className="knowledge-card-top">
                      <span
                        className={`knowledge-kind knowledge-kind-${chunk.source_kind}`}
                      >
                        {sourceKindLabel(chunk.source_kind)}
                      </span>
                      <span>{evidenceOrdinalLabel(chunk.ordinal)}</span>
                    </div>
                    <h3>{sourceTitle(chunk.title)}</h3>
                    <div className="knowledge-date">
                      <Icon name="clock" size={16} />
                      <span>
                        {historicalCaption(date(chunk.source_date, true))}
                      </span>
                    </div>
                    <p className="knowledge-context">
                      {workspaceEpisode(data, clientId, chunk.episode_id)}
                      {" · "}
                      {workspaceSession(data, clientId, chunk.session_id)}
                    </p>
                    <p className="knowledge-snippet">{chunk.text}</p>
                    <div className="knowledge-card-actions">
                      <label className="knowledge-pick">
                        <input
                          type="checkbox"
                          checked={selectedEvidence.some(
                            (s) => s.id === chunk.id,
                          )}
                          disabled={
                            !selectedEvidence.some((s) => s.id === chunk.id) &&
                            selectedEvidence.length >= 12
                          }
                          onChange={(e) =>
                            setSelectedEvidence((old) =>
                              e.target.checked
                                ? [...old, chunk]
                                : old.filter((s) => s.id !== chunk.id),
                            )
                          }
                        />
                        Use this excerpt in a draft
                      </label>
                      <Button
                        variant="secondary small"
                        onClick={() => openSource(chunk.document_id)}
                      >
                        Read full source <Icon name="arrow" size={15} />
                      </Button>
                      {onOpenSource && (
                        <Button
                          variant="text small"
                          onClick={() =>
                            onOpenSource(chunk.source_kind, chunk.source_id)
                          }
                        >
                          Open original record
                        </Button>
                      )}
                    </div>
                  </article>
                );
              })}
            {!listLoading &&
              !listError &&
              visibleSearch &&
              visibleSearch.results.length === 0 && (
                <Empty
                  icon="search"
                  title={emptyCopy.title}
                  description={emptyCopy.description}
                />
              )}
            {visibleSearch && (
              <div className="knowledge-pager">
                <p>
                  {pageLabel(
                    visibleSearch.total,
                    visibleSearch.limit,
                    visibleSearch.offset,
                  )}
                </p>
                {pageMismatch(
                  { limit, offset },
                  { limit: visibleSearch.limit, offset: visibleSearch.offset },
                ) && (
                  <p className="knowledge-page-note">
                    The index reported a different page window than requested.
                    The range above follows the index response.
                  </p>
                )}
                <Button
                  variant="secondary small"
                  disabled={
                    !pageWindow(
                      visibleSearch.total,
                      visibleSearch.limit,
                      visibleSearch.offset,
                    ).hasPrev || listLoading
                  }
                  onClick={() =>
                    goTo(
                      pageWindow(
                        visibleSearch.total,
                        visibleSearch.limit,
                        visibleSearch.offset,
                      ).prevOffset,
                    )
                  }
                >
                  Previous
                </Button>
                <Button
                  variant="secondary small"
                  disabled={
                    !pageWindow(
                      visibleSearch.total,
                      visibleSearch.limit,
                      visibleSearch.offset,
                    ).hasNext || listLoading
                  }
                  onClick={() =>
                    goTo(
                      pageWindow(
                        visibleSearch.total,
                        visibleSearch.limit,
                        visibleSearch.offset,
                      ).nextOffset,
                    )
                  }
                >
                  Next
                </Button>
              </div>
            )}
          </div>

          <aside
            className="knowledge-detail"
            ref={detailRef}
            aria-live="polite"
          >
            <span className="eyebrow">SOURCE DETAIL</span>
            {!activeDocumentId && (
              <>
                <h3>Review an approved source</h3>
                <p>
                  Choose “Read full source” to review the original wording, date
                  and encounter context.
                </p>
              </>
            )}
            {detailLoading && (
              <div className="knowledge-loading" role="status">
                <div className="spinner" />
                <p>Loading the exact indexed source…</p>
              </div>
            )}
            {detailError && (
              <div className="error-banner" role="alert">
                {detailError}
              </div>
            )}
            {visibleDetail && (
              <>
                <span
                  className={`knowledge-kind knowledge-kind-${visibleDetail.document.source_kind}`}
                >
                  {sourceKindLabel(visibleDetail.document.source_kind)}
                </span>
                <h3>{sourceTitle(visibleDetail.document.title)}</h3>
                <div className="knowledge-date">
                  <Icon name="clock" size={16} />
                  <span>
                    {historicalCaption(
                      date(visibleDetail.document.source_date, true),
                    )}
                  </span>
                </div>
                <p className="knowledge-context">
                  {workspaceEpisode(
                    data,
                    clientId,
                    visibleDetail.document.episode_id,
                  )}
                  {" · "}
                  {workspaceSession(
                    data,
                    clientId,
                    visibleDetail.document.session_id,
                  )}
                </p>
                {onOpenSource && (
                  <div className="knowledge-card-actions">
                    <Button
                      variant="secondary small"
                      onClick={() =>
                        onOpenSource(
                          visibleDetail.document.source_kind,
                          visibleDetail.document.source_id,
                        )
                      }
                    >
                      Open original record <Icon name="link" size={15} />
                    </Button>
                  </div>
                )}
                <h3 className="knowledge-section-title">Source text</h3>
                <div className="knowledge-body">
                  {visibleDetail.document.body ||
                    "The indexed source body was empty."}
                </div>
                <h3 className="knowledge-section-title">Evidence extracts</h3>
                {visibleDetail.chunks.length === 0 ? (
                  <p>The index returned this source without extracts.</p>
                ) : (
                  chunksInOrdinalOrder(visibleDetail.chunks).map((chunk) => (
                    <article key={chunk.id} className="knowledge-chunk">
                      <span className="eyebrow">
                        {evidenceOrdinalLabel(chunk.ordinal)}
                      </span>
                      <p>{chunk.text}</p>
                    </article>
                  ))
                )}
                <details className="knowledge-provenance">
                  <summary>Source provenance</summary>
                  <dl>
                    <dt>Source date</dt>
                    <dd>{visibleDetail.document.source_date}</dd>
                    <dt>Canonical record</dt>
                    <dd>{visibleDetail.document.source_id}</dd>
                    <dt>Source version</dt>
                    <dd>{visibleDetail.document.source_version}</dd>
                    <dt>Source hash</dt>
                    <dd className="knowledge-hash">
                      {visibleDetail.document.source_hash}
                    </dd>
                    <dt>Index document</dt>
                    <dd>{visibleDetail.document.id}</dd>
                    <dt>Organization</dt>
                    <dd>{visibleDetail.document.organization_id}</dd>
                    <dt>Episode ID</dt>
                    <dd>{visibleDetail.document.episode_id ?? "None"}</dd>
                    <dt>Session ID</dt>
                    <dd>{visibleDetail.document.session_id ?? "None"}</dd>
                  </dl>
                </details>
              </>
            )}
          </aside>
        </div>
        {!!selectedEvidence.length && (
          <div className="knowledge-selection">
            <strong>{selectedEvidence.length} excerpts selected</strong>
            <Button
              variant="secondary small"
              onClick={() => setSelectedEvidence([])}
            >
              Clear selected evidence
            </Button>
          </div>
        )}
        <ClinicalDrafting
          key={clientId}
          clientId={clientId}
          data={data}
          sources={selectedEvidence}
          onNoteCreated={onNoteCreated}
          onReviewDirty={onReviewDirty}
        />
      </div>
    </Panel>
  );
}
