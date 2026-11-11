import { useState } from "react";
import { actorStorageKey } from "./api";
import { Button, Icon, Panel } from "./ui";

function isoDay(date: Date) {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}
export default function ExportReports({ role }: { role: string }) {
  const [from, setFrom] = useState(() => {
    const date = new Date();
    date.setDate(date.getDate() - 29);
    return isoDay(date);
  });
  const [to, setTo] = useState(() => isoDay(new Date()));
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  async function download(kind: "accounting" | "claims") {
    setError("");
    setBusy(kind);
    try {
      if (!from || !to || from > to)
        throw new Error("Choose a valid start and end date.");
      const base =
        (import.meta.env.VITE_API_BASE_URL as string | undefined)?.replace(
          /\/$/,
          "",
        ) || "";
      const response = await fetch(
        `${base}/api/exports/${kind}.csv?${new URLSearchParams({ from, to })}`,
        {
          headers: {
            "x-nexus-actor": localStorage.getItem(actorStorageKey) || "",
            Accept: "text/csv",
          },
        },
      );
      if (!response.ok) {
        const body = await response.json().catch(() => null);
        throw new Error(
          body?.error?.message || `Export failed (${response.status}).`,
        );
      }
      const blob = await response.blob();
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = `nexus-${kind}-${from}-to-${to}.csv`;
      document.body.append(anchor);
      anchor.click();
      anchor.remove();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
    } catch (e) {
      setError(
        e instanceof Error ? e.message : "This report could not be downloaded.",
      );
    } finally {
      setBusy("");
    }
  }
  if (!["admin", "billing", "executive"].includes(role)) return null;
  return (
    <Panel
      title="Reports you can work with"
      subtitle="Download actual recorded activity for internal review and reconciliation."
      className="export-panel"
    >
      <div className="export-controls">
        <div className="form-field">
          <label htmlFor="export-from">From</label>
          <input
            id="export-from"
            type="date"
            value={from}
            onInput={(e) => setFrom(e.currentTarget.value)}
            onChange={(e) => setFrom(e.target.value)}
          />
        </div>
        <div className="form-field">
          <label htmlFor="export-to">Through</label>
          <input
            id="export-to"
            type="date"
            value={to}
            onInput={(e) => setTo(e.currentTarget.value)}
            onChange={(e) => setTo(e.target.value)}
          />
        </div>
        <Button
          variant="secondary"
          disabled={!!busy}
          onClick={() => void download("accounting")}
        >
          <Icon name="download" size={17} />
          {busy === "accounting" ? "Preparing…" : "Accounting summary"}
        </Button>
        {["admin", "billing"].includes(role) && (
          <Button
            variant="secondary"
            disabled={!!busy}
            onClick={() => void download("claims")}
          >
            <Icon name="download" size={17} />
            {busy === "claims" ? "Preparing…" : "Claims review"}
          </Button>
        )}
      </div>
      <div className="export-description">
        <p>
          <strong>Accounting summary:</strong> aggregated dates, payment
          sources, cash receipts, and adjustments. No client identifiers. This
          is not a configured accounting-system import.
        </p>
        <p>
          <strong>Claims review:</strong> an internal billing worksheet. It is
          not an X12 claim file and does not submit anything to a payer.
        </p>
      </div>
      {error && (
        <div className="error-banner export-error" role="alert">
          {error}
        </div>
      )}
    </Panel>
  );
}
