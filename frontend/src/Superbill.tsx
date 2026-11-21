import { useEffect, useState } from "react";
import { label, money, request } from "./api";
import { formatDateText, formatTimeText, zoneLabel } from "./dateText";
import { NxIcon } from "./nexus-ui/Icon";
import { Button, Modal } from "./ui";
import "./superbill.css";

type Statement = {
  number: string;
  generated_by: string;
  organization: string;
  facility: string;
  time_zone: string;
  client: { name: string; birth_date: string; mrn: string; address: string[] };
  provider: { name: string; role: string };
  session: { kind: string; location: string; service_date: string; start_time: string; end_time: string; minutes: number };
  service: { code: string; name: string; units: number; charge_cents: number };
  diagnosis_code: string;
  coverage: { payer_name: string; member_id: string; group_number: string; authorization_number: string | null };
  claim: { id: string; status: string; external_reference: string | null };
  payments: { received_on: string; source: string; reference: string; amount_cents: number; reversed: boolean }[];
  totals: { charge_cents: number; received_cents: number; adjustment_cents: number; balance_cents: number };
  not_on_file: string[];
};

/** While a superbill prints, the page shows nothing else. The class lets the layout be checked on screen too. */
function printing(on: boolean) {
  document.documentElement.classList.toggle("nx-printing", on);
}

/**
 * An itemized statement of one billed session, built by the API from the claim and its
 * ledger. Opening it is audited. It is shown here as it prints.
 */
export default function Superbill({ claimId, onClose }: { claimId: string; onClose: () => void }) {
  const [bill, setBill] = useState<Statement | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    let live = true;
    request<Statement>(`/api/claims/${claimId}/superbill`).then(
      (statement) => live && setBill(statement),
      (problem) => live && setError(problem instanceof Error ? problem.message : "The superbill could not be prepared."),
    );
    return () => {
      live = false;
    };
  }, [claimId]);
  useEffect(() => {
    // Covers the browser's own print command as well as the button.
    const before = () => printing(true);
    const after = () => printing(false);
    window.addEventListener("beforeprint", before);
    window.addEventListener("afterprint", after);
    return () => {
      window.removeEventListener("beforeprint", before);
      window.removeEventListener("afterprint", after);
      printing(false);
    };
  }, []);
  const today = formatDateText(new Date().toLocaleDateString("en-CA"));
  return (
    <Modal
      wide
      icon="print"
      title="Superbill"
      description="An itemized statement of this session, for the client to keep or send to their own payer."
      onClose={onClose}
    >
      <div className="form-body">
        {error && (
          <div className="error-banner" role="alert">
            <NxIcon name="error" size={17} />
            <span>{error}</span>
          </div>
        )}
        {!bill && !error && <p role="status" className="superbill-wait">Preparing the statement.</p>}
        {bill && (
          <article className="superbill" aria-label={`Superbill ${bill.number}`}>
            <header className="superbill-head">
              <div>
                <strong className="superbill-practice">{bill.organization}</strong>
                <span>{bill.facility}</span>
              </div>
              <div className="superbill-title">
                <h3>Superbill</h3>
                <span>{bill.number}</span>
                <span>Prepared {today}</span>
              </div>
            </header>

            <div className="superbill-parties">
              <section>
                <h4>Client</h4>
                <strong>{bill.client.name}</strong>
                <span>Born {formatDateText(bill.client.birth_date) || bill.client.birth_date}</span>
                <span>Record number {bill.client.mrn}</span>
                {bill.client.address.length ? bill.client.address.map((line) => <span key={line}>{line}</span>) : <span className="superbill-missing">No address on file</span>}
              </section>
              <section>
                <h4>Provider</h4>
                <strong>{bill.provider.name}</strong>
                <span>{label(bill.provider.role)}</span>
                <span>{bill.facility}</span>
              </section>
              <section>
                <h4>Coverage on file</h4>
                <strong>{bill.coverage.payer_name}</strong>
                <span>Member ID {bill.coverage.member_id}</span>
                {bill.coverage.group_number && <span>Group {bill.coverage.group_number}</span>}
                {bill.coverage.authorization_number && <span>Authorization {bill.coverage.authorization_number}</span>}
              </section>
            </div>

            <table className="superbill-lines">
              <caption>Service delivered</caption>
              <thead>
                <tr>
                  <th scope="col">Date of service</th>
                  <th scope="col">Service</th>
                  <th scope="col">Diagnosis</th>
                  <th scope="col" className="num">Units</th>
                  <th scope="col" className="num">Charge</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td>
                    <strong>{formatDateText(bill.session.service_date) || bill.session.service_date}</strong>
                    <small>{formatTimeText(bill.session.start_time)} to {formatTimeText(bill.session.end_time)} · {bill.session.minutes} min</small>
                  </td>
                  <td>
                    <strong>{bill.service.code}</strong>
                    <small>{[bill.service.name, label(bill.session.kind), bill.session.location].filter(Boolean).join(" · ")}</small>
                  </td>
                  <td><strong>{bill.diagnosis_code}</strong></td>
                  <td className="num">{bill.service.units}</td>
                  <td className="num"><strong>{money(bill.service.charge_cents)}</strong></td>
                </tr>
              </tbody>
            </table>

            <div className="superbill-money">
              <section>
                <h4>Payments recorded</h4>
                {bill.payments.length ? (
                  <ul>
                    {bill.payments.map((payment) => (
                      <li key={payment.reference} data-reversed={payment.reversed ? "" : undefined}>
                        <span>{formatDateText(payment.received_on) || payment.received_on} · {label(payment.source)} · {payment.reference}{payment.reversed ? " · Reversed" : ""}</span>
                        <span className="num">{money(payment.amount_cents)}</span>
                      </li>
                    ))}
                  </ul>
                ) : <span className="superbill-missing">None recorded.</span>}
              </section>
              <dl className="superbill-totals">
                <dt>Charge</dt><dd>{money(bill.totals.charge_cents)}</dd>
                <dt>Received</dt><dd>{money(bill.totals.received_cents)}</dd>
                <dt>Adjustments</dt><dd>{money(bill.totals.adjustment_cents)}</dd>
                <dt className="due">Balance</dt><dd className="due">{money(bill.totals.balance_cents)}</dd>
              </dl>
            </div>

            <footer className="superbill-foot">
              <p>This statement lists care that was delivered and what has been recorded as paid. It is not a claim submission and does not promise reimbursement. Claim status at Nexus: {label(bill.claim.status).toLowerCase()}.</p>
              <p><strong>Not on this statement.</strong> A payer may ask for these, and Nexus does not hold them yet: {bill.not_on_file.map((item, index) => (index ? item.charAt(0).toLowerCase() + item.slice(1) : item)).join("; ")}.</p>
              <p className="superbill-prepared">Prepared by {bill.generated_by} with Nexus. Times are {zoneLabel(bill.time_zone)}.</p>
            </footer>
          </article>
        )}
      </div>
      <div className="modal-foot">
        <span className="form-assurance">
          <NxIcon name="shield" size={15} /> Opening a superbill is recorded in the audit trail.
        </span>
        <Button variant="secondary" onClick={onClose}>Close</Button>
        <button type="button" className="button" disabled={!bill} onClick={() => { printing(true); window.print(); }}>
          <NxIcon name="print" size={16} />Print or save as PDF
        </button>
      </div>
    </Modal>
  );
}
