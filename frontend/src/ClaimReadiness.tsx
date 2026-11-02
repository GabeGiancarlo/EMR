import { useEffect, useState } from "react";
import { request, value } from "./api";
import type { Row } from "./api";
import { Button, Modal } from "./ui";

type ReadinessCheck = {
  code: string;
  title: string;
  passed: boolean;
  message: string;
  owner_role: string;
};

type ReadinessReport = {
  ready: boolean;
  checklist: ReadinessCheck[];
};

function ownerLabel(role: string) {
  if (role === "clinical") return "Clinical";
  if (role === "billing") return "Billing";
  return role;
}

function isReport(value: unknown): value is ReadinessReport {
  if (!value || typeof value !== "object") return false;
  const report = value as ReadinessReport;
  return typeof report.ready === "boolean" && Array.isArray(report.checklist);
}

export default function ClaimReadiness({
  claim,
  canWrite,
  onClose,
  onMarkedReady,
}: {
  claim: Row;
  canWrite: boolean;
  onClose: () => void;
  onMarkedReady: () => Promise<void>;
}) {
  const [report, setReport] = useState<ReadinessReport | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const status = value(claim, "status");

  useEffect(() => {
    let cancelled = false;
    setReport(null);
    setError("");
    request<ReadinessReport>(`/api/claims/${claim.id}/readiness`)
      .then((result) => {
        if (cancelled) return;
        if (!isReport(result)) {
          setError("Readiness did not include the checklist.");
          return;
        }
        setReport(result);
      })
      .catch((reason: unknown) => {
        if (cancelled) return;
        setError(
          reason instanceof Error
            ? reason.message
            : "Readiness could not be loaded.",
        );
      });
    return () => {
      cancelled = true;
    };
  }, [claim.id]);

  async function markReady() {
    if (!canWrite || status !== "draft" || !report?.ready) return;
    setBusy(true);
    setError("");
    try {
      await request(`/api/claims/${claim.id}/ready`, "POST", {});
      await onMarkedReady();
    } catch (reason) {
      setError(
        reason instanceof Error
          ? reason.message
          : "This claim could not be marked ready.",
      );
      try {
        const refreshed = await request<ReadinessReport>(
          `/api/claims/${claim.id}/readiness`,
        );
        if (isReport(refreshed)) setReport(refreshed);
      } catch {
        // Keep the transition error visible.
      }
    } finally {
      setBusy(false);
    }
  }

  const distinction =
    status === "ready"
      ? "This claim is marked internally ready. Review the current checks below."
      : status === "draft" && report?.ready
        ? "Every check passed. This draft can be marked internally ready."
        : status === "draft"
          ? "Resolve the items below before marking this draft ready."
          : "Review the current checks below.";

  return (
    <Modal
      title="Billing readiness"
      description="Internal validation only. A passing checklist does not transmit the claim."
      onClose={onClose}
      wide
    >
      <div className="readiness-content">
      {report && <div className="quiet-note">
        <span>{distinction}</span>
      </div>}
      {status === "draft" && report?.ready && !canWrite && (
        <p>Your role can review readiness but cannot mark claims ready.</p>
      )}
      {error && (
        <div className="error-banner" role="alert">
          {error}
        </div>
      )}
      {!report && !error && <p>Loading the readiness checklist…</p>}
      {report && (
        <>
          <p>
            <strong>
              {report.checklist.filter((item) => item.passed).length} of {report.checklist.length} checks passed
            </strong>
            {" — "}
            {report.ready
              ? "All listed checks are satisfied."
              : "Each remaining item names the responsible team."}
          </p>
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th scope="col">Check</th>
                  <th scope="col">Result</th>
                  <th scope="col">Owner</th>
                  <th scope="col">Detail</th>
                </tr>
              </thead>
              <tbody>
                {report.checklist.map((item) => (
                  <tr key={item.code}>
                    <td>
                      <strong>{item.title}</strong>
                    </td>
                    <td><span className={`readiness-result ${item.passed ? "passed" : "blocked"}`}>{item.passed ? "Passed" : "Needs attention"}</span></td>
                    <td>{ownerLabel(item.owner_role)}</td>
                    <td>{item.message}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
      </div>
      <div className="modal-foot">
        <Button variant="secondary" onClick={onClose}>
          Close
        </Button>
        {canWrite && status === "draft" && report?.ready && (
          <Button disabled={busy} onClick={() => void markReady()}>
            Mark internally ready
          </Button>
        )}
      </div>
    </Modal>
  );
}
