import type { Row } from "./api";

export function mayEditResponse(response: Row, actor: Row | null) {
  return !!actor && (response.status === "draft" || response.status === "reviewed")
    && (response.author_id === actor.id || actor.role === "admin" || actor.role === "clinical_lead");
}

/** Find the latest unfinished response the operator can continue in this exact care context. */
export function savedFormResponse(
  responses: Row[], clientId: string, episodeId: string | null, templateId: string, actor: Row | null,
) {
  const matching = responses.filter(row =>
    row.client_id === clientId && (row.episode_id ?? null) === episodeId
    && row.template_id === templateId && (row.status === "draft" || row.status === "reviewed"),
  );
  const canContinue = (row: Row) => row.status === "draft" && mayEditResponse(row, actor);
  return matching.sort((a, b) =>
    Number(canContinue(b)) - Number(canContinue(a))
    || String(b.updated_at).localeCompare(String(a.updated_at))
    || b.id.localeCompare(a.id),
  )[0];
}
