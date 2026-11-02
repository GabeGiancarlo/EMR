import { useEffect, useState } from "react";
import { label, request } from "./api";
import type { Bootstrap } from "./api";
import { Badge, Button, Empty, Panel } from "./ui";

export type FormRequirement = {
  template_id: string;
  template_name: string;
  disposition: string;
  completion: string;
  response_status?: string | null;
  reason: string;
  episode_id: string | null;
  applies_to_stage: string;
  applies_to_care_level: string;
  required_for_stage: boolean;
};

export type FormRequirementsPayload = {
  client_id: string;
  client_status: string;
  relevant_episode_id: string | null;
  relevant_stage: string;
  relevant_care_level: string | null;
  disclaimer: string;
  requirements: FormRequirement[];
};

export default function FormRequirements({
  clientId,
  data,
  canStart,
  onStart,
  onAuthorRules,
}: {
  clientId: string;
  data: Bootstrap;
  canStart: boolean;
  onStart: (
    templateId: string,
    episodeId: string | null,
    templateName?: string,
  ) => void;
  onAuthorRules?: () => void;
}) {
  const [payload, setPayload] = useState<FormRequirementsPayload | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setPayload(null);
    setError("");
  }, [clientId]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    request<FormRequirementsPayload>(
      `/api/clients/${clientId}/form-requirements`,
    )
      .then((result) => {
        if (cancelled) return;
        setPayload(result);
        setError("");
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setPayload(null);
        setError(
          err instanceof Error
            ? err.message
            : "Form rules could not be loaded.",
        );
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [clientId, data]);

  const rules = Array.isArray(payload?.requirements) ? payload.requirements : [];
  const unsignedRequired = rules.filter(
    (item) => item.disposition === "required" && item.completion !== "signed",
  ).length;
  const context = payload
    ? `${label(payload.relevant_stage || payload.client_status)} stage${
        payload.relevant_care_level
          ? ` · ${label(payload.relevant_care_level)}`
          : " · no relevant care episode"
      }`
    : "";

  return (
    <Panel
      title="Organization form rules"
      subtitle={
        payload?.disclaimer ||
        "Active organization-authored rules only. Nexus does not infer legal or clinical requirements."
      }
    >
      {loading && !payload ? (
        <p>Checking organization form rules…</p>
      ) : error ? (
        <div className="error-banner" role="alert">
          {error}
        </div>
      ) : rules.length === 0 ? (
        <Empty
          icon="notes"
          title="No form rules apply"
          description="No active form rule matches this client's stage and care level."
          action={
            onAuthorRules || canStart ? (
              <div className="row-actions">
                {onAuthorRules && (
                  <Button variant="secondary" onClick={onAuthorRules}>
                    Author a form rule
                  </Button>
                )}
                {canStart && (
                  <Button variant="secondary" onClick={() => onStart("", null)}>
                    Start a form
                  </Button>
                )}
              </div>
            ) : undefined
          }
        />
      ) : (
        <>
          <p className="form-rules-context">
            <small>{context}</small>
          </p>
          {unsignedRequired > 0 && (
            <p className="form-rules-context">
              <small>
                {unsignedRequired} form{unsignedRequired === 1 ? "" : "s"} your
                organization marked required {unsignedRequired === 1 ? "is" : "are"} not
                signed.
              </small>
            </p>
          )}
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Form</th>
                  <th>Rule</th>
                  <th>Status</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {rules.map((item) => (
                  <tr key={item.template_id}>
                    <td>
                      <strong>{item.template_name}</strong>
                    </td>
                    <td>
                      <Badge
                        status={
                          item.disposition === "required" ? "required" : "suggested"
                        }
                      />
                      <small>{item.reason}</small>
                    </td>
                    <td>
                      <Badge status={item.completion} />
                      {item.response_status === "reviewed" && (
                        <small>Reviewed, not signed</small>
                      )}
                    </td>
                    <td>
                      {canStart && (
                        <Button
                          variant="text small"
                          onClick={() =>
                            onStart(
                              item.template_id,
                              item.episode_id,
                              item.template_name,
                            )
                          }
                        >
                          {item.completion === "signed" ? "Start another" : "Start form"}
                        </Button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </Panel>
  );
}
