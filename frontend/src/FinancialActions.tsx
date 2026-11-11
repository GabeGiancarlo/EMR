import { clientName, date, money, value } from "./api";
import type { Bootstrap, Row } from "./api";
import type { Field } from "./forms";
import { Badge, Button, Panel } from "./ui";
type OpenAction = (
  path: string,
  title: string,
  description: string,
  fields?: Field[],
  initial?: Record<string, unknown>,
  submitLabel?: string,
) => void;

export function ClaimCorrections({
  claim,
  action,
  edit,
}: {
  claim: Row;
  action: OpenAction;
  edit: () => void;
}) {
  if (!["draft", "ready"].includes(value(claim, "status"))) return null;
  return (
    <>
      {claim.status === "draft" && (
        <Button variant="text small" onClick={edit}>
          Edit draft
        </Button>
      )}
      {claim.status === "ready" && (
        <Button
          variant="text small"
          onClick={() =>
            action(
              `/api/claims/${claim.id}/return-to-draft`,
              "Return claim to draft",
              "Remove internal readiness so you can correct the claim. The authorization reservation is released. This does not withdraw any externally submitted claim.",
              [],
              {},
              "Return to draft",
            )
          }
        >
          Return to draft
        </Button>
      )}
      <Button
        variant="text small danger"
        onClick={() =>
          action(
            `/api/claims/${claim.id}/void`,
            "Void unsubmitted claim",
            "Preserve this claim in history, mark it void, and release its authorization reservation. Only an unsubmitted claim can be voided. You may create a replacement claim for the same service.",
            [
              {
                key: "reason",
                label: "Reason for voiding",
                type: "textarea",
                required: true,
                hint: "Give a specific reason for the correction.",
              },
            ],
            {},
            "Void claim",
          )
        }
      >
        Void
      </Button>
    </>
  );
}

export function PaymentCorrection({
  payment,
  reversal,
  action,
  canWrite,
}: {
  payment: Row;
  reversal?: Row;
  action: OpenAction;
  canWrite: boolean;
}) {
  if (reversal) return <Badge status="reversed" />;
  if (!canWrite) return <Badge status="posted" />;
  return (
    <Button
      variant="text small danger"
      onClick={() =>
        action(
          `/api/payments/${payment.id}/reverse`,
          "Reverse recorded payment",
          `Record a full reversal of ${money(Number(payment.amount_cents))}. The original entry stays in the ledger, the correction is appended, and the claim balance is recalculated. This records a correction; it does not move money.`,
          [
            {
              key: "reversed_on",
              label: "Reversal date",
              type: "date",
              required: true,
            },
            {
              key: "reason",
              label: "Reason for reversal",
              type: "textarea",
              required: true,
              hint: "Explain the correction in at least 10 characters.",
            },
          ],
          {},
          "Record reversal",
        )
      }
    >
      Reverse entry
    </Button>
  );
}

export function ReversalHistory({
  data,
  reversals,
}: {
  data: Bootstrap;
  reversals: Row[];
}) {
  if (!reversals.length) return null;
  return (
    <Panel
      title="Payment reversal history"
      subtitle="Corrections are appended; original payment entries are retained."
      className="export-panel"
    >
      <div className="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Client / payment</th>
              <th>Reversed</th>
              <th>Amount</th>
              <th>Reason</th>
              <th>Recorded by</th>
            </tr>
          </thead>
          <tbody>
            {reversals.map((r) => {
              const payment = data.payments.find((p) => p.id === r.payment_id);
              const claim = data.claims.find((c) => c.id === r.claim_id);
              const client = data.clients.find(
                (c) => c.id === claim?.client_id,
              );
              return (
                <tr key={r.id}>
                  <td>
                    <strong>{clientName(client)}</strong>
                    <small>{value(payment, "reference")}</small>
                  </td>
                  <td>{date(r.reversed_on)}</td>
                  <td>
                    <strong>−{money(Number(r.amount_cents))}</strong>
                  </td>
                  <td>{value(r, "reason")}</td>
                  <td>
                    {value(
                      data.staff.find((s) => s.id === r.actor_id),
                      "name",
                    ) || "Team member"}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </Panel>
  );
}
