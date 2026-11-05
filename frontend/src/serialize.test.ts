import { strict as assert } from "node:assert";
import { test } from "node:test";
import { normalizeFieldValue } from "./serialize.ts";

test("minimal client payload preserves NOT NULL optional text while omitting nullable relationships", () => {
  const fields = [
    { key: "first_name", label: "First name", required: true },
    { key: "preferred_name", label: "Preferred name" },
    { key: "email", label: "Email", type: "email" },
    { key: "phone", label: "Phone", type: "tel" },
    { key: "emergency_contact", label: "Emergency contact" },
    {
      key: "primary_clinician_id",
      label: "Primary clinician",
      type: "select",
      ref: "staff",
    },
    { key: "birth_date", label: "Date of birth", type: "date", required: true },
  ];
  const entered: Record<string, unknown> = {
    first_name: "  Intake  ",
    preferred_name: "",
    email: "",
    phone: "",
    emergency_contact: "",
    primary_clinician_id: "",
    birth_date: "1990-01-01",
  };
  const body = Object.fromEntries(
    fields.map((field) => [
      field.key,
      normalizeFieldValue(field, entered[field.key]),
    ]),
  );
  assert.deepEqual(body, {
    first_name: "Intake",
    preferred_name: "",
    email: "",
    phone: "",
    emergency_contact: "",
    primary_clinician_id: null,
    birth_date: "1990-01-01",
  });
});
test("optional end dates stay null and required missing dates are rejected", () => {
  assert.equal(
    normalizeFieldValue(
      { key: "termination_date", label: "Termination date", type: "date" },
      "",
    ),
    null,
  );
  assert.throws(
    () =>
      normalizeFieldValue(
        {
          key: "birth_date",
          label: "Date of birth",
          type: "date",
          required: true,
        },
        "",
      ),
    /Date of birth is required/,
  );
});
test("financial entry converts decimal dollars into exact integer cents and rejects nonnumbers", () => {
  const field = {
    key: "amount_cents",
    label: "Amount",
    type: "money",
    required: true,
  };
  assert.equal(normalizeFieldValue(field, "19.95"), 1995);
  assert.throws(() => normalizeFieldValue(field, "not money"), /valid number/);
  assert.throws(() => normalizeFieldValue(field, ""), /Amount is required/);
});
test("legacy booking service selections stay nullable while new bookings require a service", () => {
  const field = {
    key: "program_service_id",
    label: "Program service",
    type: "select",
    catalog: "services" as const,
    required: false,
  };
  for (const empty of ["", "   ", null, undefined]) {
    assert.equal(normalizeFieldValue(field, empty), null);
    assert.throws(
      () => normalizeFieldValue({ ...field, required: true }, empty),
      /Program service is required/,
    );
  }
  const service = "11111111-1111-1111-1111-111111111111";
  assert.equal(normalizeFieldValue(field, ` ${service} `), service);
});
