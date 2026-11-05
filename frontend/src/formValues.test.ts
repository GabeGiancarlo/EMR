import { test } from "node:test";
import assert from "node:assert/strict";
import type { Bootstrap, Row } from "./api";
import type { Field } from "./forms";
import { claimFormDefaults, initialFormValues } from "./formValues.ts";
import { normalizeFieldValue } from "./serialize.ts";

const amount: Field = { key: "amount_cents", label: "Charge", type: "money" };
const fields: Field[] = [{ key: "service_code", label: "Service" }, amount];
const session = (cents?: number): Row => ({ id: "session", program_service_id: "service",
  service_snapshot: { id: "service", service_code: "QA", default_charge_cents: cents } });
const data = (row: Row) => ({ sessions: [row], episodes: [], clients: [], facilities: [] }) as unknown as Bootstrap;

test("preselected and interactively selected claims preserve the same exact cents", () => {
  for (const cents of [12500, 12505, 0]) {
    const row = session(cents);
    const selected = claimFormDefaults(row);
    const initialized = initialFormValues(fields, data(row), { session_id: row.id }, "claims");
    assert.deepEqual(initialized, selected);
    assert.equal(initialized.amount_cents, cents / 100);
    assert.equal(normalizeFieldValue(amount, initialized.amount_cents), cents);
  }
});

test("explicit initial amounts take precedence and existing claims do not acquire catalog defaults", () => {
  const row = session(12500);
  for (const cents of [6505, 0]) {
    const initialized = initialFormValues(fields, data(row), { session_id: row.id, amount_cents: cents }, "claims");
    assert.equal(normalizeFieldValue(amount, initialized.amount_cents), cents);
  }
  assert.deepEqual(initialFormValues(fields, data(row), { id: "claim", session_id: row.id }, "claims"),
    { service_code: "", amount_cents: "" });
  assert.equal(initialFormValues(fields, data(row), { id: "claim", amount_cents: 3456 }, "claims").amount_cents, 34.56);
});

test("selecting a different session replaces defaults and missing prices remain blank", () => {
  const first = claimFormDefaults(session(12500));
  assert.equal(first.amount_cents, 125);
  assert.equal(({ ...first, ...claimFormDefaults(session(7505)) }).amount_cents, 75.05);
  assert.equal(({ ...first, ...claimFormDefaults(session()) }).amount_cents, "");
  assert.deepEqual(claimFormDefaults(undefined), { service_code: "", amount_cents: "" });
  assert.equal(normalizeFieldValue(amount, claimFormDefaults(session()).amount_cents), null);
});

test("shared initialization preserves custom fields and facility wall time through a DST fold", () => {
  const fixture = { ...data(session()), facilities: [{ id: "facility", timezone: "America/New_York" }],
    episodes: [{ id: "episode", facility_id: "facility" }] } as Bootstrap;
  const custom: Field[] = [{ key: "starts_at", label: "Start", type: "datetime-local" },
    { key: "checked", label: "Checked", type: "checkbox" },
    { key: "choice", label: "Choice", type: "select", required: true, options: ["first", "second"] },
    { key: "fields", label: "Fields", type: "fields" }, amount];
  for (const instant of ["2026-11-01T05:30:00Z", "2026-11-01T06:30:00Z"]) {
    assert.deepEqual(initialFormValues(custom, fixture, { episode_id: "episode", starts_at: instant, amount_cents: 12505 }, "sessions"),
      { starts_at: "2026-11-01T01:30", checked: false, choice: "first", fields: [], amount_cents: 125.05 });
  }
});
