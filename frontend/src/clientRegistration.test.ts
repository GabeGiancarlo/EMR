import assert from "node:assert/strict";
import test from "node:test";
import { hasCompletedAdultYears } from "./adult.ts";
import {
  RESTRICTED_SAVE_MESSAGE,
  activeFacilityChoices,
  beginCheck,
  clinicianChoices,
  completeCheck,
  duplicateGate,
  emptyValues,
  invalidateCheck,
  isAbortError,
  isIdentityField,
  matchCheckBlock,
  matchRequestFromValues,
  matchSummary,
  parseMatchResponse,
  planSubmission,
  readApiError,
  registrationDirty,
  reviewRequired,
  snapshotFromResponse,
  valuesFromRow,
  withCurrentRow,
  type FacilityChoice,
  type MatchSnapshot,
  type RegistrationValues,
} from "./registrationRules.ts";

const facilities: FacilityChoice[] = [
  { id: "fac-1", active: true, timezone: "UTC", name: "North" },
];
const REASON = "Reviewed the match result for this chart.";

function baseValues(patch: Partial<RegistrationValues> = {}): RegistrationValues {
  return {
    ...emptyValues(),
    first_name: "Ada",
    last_name: "Lovelace",
    birth_date: "1990-05-02",
    email: "ada@example.com",
    phone: "(555) 010-1000",
    facility_id: "fac-1",
    status: "inquiry",
    ...patch,
  };
}

function checked(
  values: RegistrationValues,
  exclude?: string,
  response: {
    matches: MatchSnapshot["matches"];
    restricted_matches: boolean;
  } = { matches: [], restricted_matches: false },
): MatchSnapshot {
  return snapshotFromResponse(matchRequestFromValues(values, exclude), response);
}

test("adult anniversary policy used by registration", () => {
  assert.equal(hasCompletedAdultYears("2004-02-29", "2022-03-01"), true);
  assert.equal(hasCompletedAdultYears("2004-02-29", "2022-02-28"), false);
});

test("create uses facility-local adult date and omits review when no records match", () => {
  const values = baseValues({ birth_date: "2004-02-29", facility_id: "fac-utc" });
  const utc = [{ id: "fac-utc", active: true, timezone: "UTC" }];
  const ny = [{ id: "fac-ny", active: true, timezone: "America/New_York" }];
  const instant = new Date("2022-03-01T03:30:00Z");
  const pass = planSubmission({
    values,
    facilities: utc,
    snapshot: checked(values),
    reason: REASON,
    now: instant,
  });
  assert.equal(pass.ok, true);
  if (pass.ok) {
    assert.equal("_duplicate_review" in pass.body, false);
    assert.equal("_expected_updated_at" in pass.body, false);
    assert.equal("mrn" in pass.body, false);
    assert.equal(pass.body.status, "inquiry");
  }
  const blocked = planSubmission({
    values: { ...values, facility_id: "fac-ny" },
    facilities: ny,
    snapshot: checked({ ...values, facility_id: "fac-ny" }),
    reason: REASON,
    now: instant,
  });
  assert.equal(blocked.ok, false);
  if (!blocked.ok) assert.match(blocked.error, /adult program/);
});

test("unchanged edit skips duplicate metadata and preserves stage", () => {
  const values = baseValues({ city: "Boston", status: "inquiry" });
  const initial = {
    id: "client-1",
    updated_at: "2024-05-01T00:00:00Z",
    ...baseValues(),
    status: "active",
    birth_date: "2015-01-01",
    secret_note: "Do not submit",
  };
  const edited = valuesFromRow(initial);
  edited.city = "Boston";
  edited.status = "inquiry";
  assert.equal(reviewRequired(edited, initial), false);
  const plan = planSubmission({
    values: edited,
    initial,
    facilities,
    snapshot: null,
    reason: "should not be sent",
    now: new Date("2024-06-01T00:00:00Z"),
  });
  assert.equal(plan.ok, true);
  if (plan.ok) {
    assert.equal("_duplicate_review" in plan.body, false);
    assert.equal(plan.body._expected_updated_at, "2024-05-01T00:00:00Z");
    assert.equal(plan.body.status, "active");
    assert.equal(plan.body.city, "Boston");
    assert.equal("secret_note" in plan.body, false);
    assert.equal("mrn" in plan.body, false);
  }
});

test("identity change requires a current check and exact matched ids", () => {
  const initialValues = baseValues();
  const initial = {
    id: "client-1",
    updated_at: "stamp",
    ...initialValues,
    status: "intake",
  };
  const changed = { ...initialValues, phone: "555-010-2000" };
  const stale = planSubmission({
    values: changed,
    initial,
    facilities,
    snapshot: checked(initialValues, "client-1"),
    reason: REASON,
  });
  assert.equal(stale.ok, false);
  const visible = [
    {
      id: "other-2",
      mrn: "M2",
      first_name: "Ada",
      last_name: "Lovelace",
      preferred_name: "",
      birth_date: "1990-05-02",
      facility_id: "fac-1",
      status: "active",
      reasons: ["name", "birth_date"],
    },
    {
      id: "other-1",
      mrn: "M1",
      first_name: "A",
      last_name: "Lovelace",
      preferred_name: "",
      birth_date: "1990-05-02",
      facility_id: "fac-1",
      status: "inquiry",
      reasons: ["phone"],
    },
  ];
  const snapshot = checked(changed, "client-1", {
    matches: visible,
    restricted_matches: false,
  });
  const short = planSubmission({
    values: changed,
    initial,
    facilities,
    snapshot,
    reason: "too short",
  });
  assert.equal(short.ok, false);
  const plan = planSubmission({
    values: changed,
    initial,
    facilities,
    snapshot,
    reason: `  ${REASON}  `,
  });
  assert.equal(plan.ok, true);
  if (plan.ok) {
    assert.deepEqual(plan.body._duplicate_review, {
      matched_ids: ["other-2", "other-1"],
      reason: REASON,
    });
    assert.equal(plan.body._expected_updated_at, "stamp");
    assert.equal(plan.body.status, "intake");
  }
  snapshot.matched_ids.push("mutated");
  assert.deepEqual(
    (plan.ok ? plan.body._duplicate_review : null) as { matched_ids: string[] },
    { matched_ids: ["other-2", "other-1"], reason: REASON },
  );
});

test("restricted matches block save and errors do not echo hidden identity", () => {
  const hidden = "HiddenPerson";
  const values = baseValues();
  const snapshot = checked(values, undefined, {
    restricted_matches: true,
    matches: [
      {
        id: "visible-1",
        mrn: "V",
        first_name: "Visible",
        last_name: "Person",
        preferred_name: "",
        birth_date: "1990-05-02",
        facility_id: "fac-1",
        status: "active",
        reasons: ["name"],
      },
    ],
  });
  const plan = planSubmission({
    values,
    facilities,
    snapshot,
    reason: REASON,
  });
  assert.equal(plan.ok, false);
  if (!plan.ok) {
    assert.equal(plan.error, RESTRICTED_SAVE_MESSAGE);
    assert.equal(plan.error.includes("Visible"), false);
    assert.equal(plan.error.includes(hidden), false);
    assert.equal(/\d/.test(plan.error), false);
  }
  assert.equal(matchSummary(snapshot, false), RESTRICTED_SAVE_MESSAGE);
  const parsed = parseMatchResponse({
    restricted_matches: true,
    matches: [],
    hidden: [{ first_name: hidden, diagnosis: hidden }],
  });
  assert.equal(parsed.ok, true);
  if (parsed.ok) assert.equal(JSON.stringify(parsed.value).includes(hidden), false);
  const leaked = parseMatchResponse({
    restricted_matches: false,
    matches: [{ id: "ok", mrn:"NX0000000001",first_name: "Synthetic",last_name:"Match",preferred_name:"",birth_date:"1990-01-01",facility_id:"fac-1",status:"inquiry", secret_note: hidden, reasons: ["name", { leak: hidden }] }],
  });
  assert.equal(leaked.ok, true);
  if (leaked.ok) {
    assert.equal(JSON.stringify(leaked.value).includes(hidden), false);
    assert.deepEqual(leaked.value.matches[0].reasons, ["name"]);
  }
  const broken = parseMatchResponse({
    restricted_matches: true,
    matches: [{ id: "", first_name: hidden }],
  });
  assert.equal(broken.ok, false);
  if (!broken.ok) assert.equal(broken.error.includes(hidden), false);
  const fallback = readApiError({ error: { detail: hidden }, first_name: hidden }, 409);
  assert.equal(fallback.includes(hidden), false);
  assert.match(fallback, /409/);
});

test("stale checks are discarded by generation and identity edits", () => {
  const values = baseValues();
  const snapshot = checked(values);
  let state = { generation: 0, snapshot };
  state = invalidateCheck(state);
  assert.equal(state.snapshot, null);
  assert.equal(state.generation, 1);
  const begun = beginCheck(state);
  assert.equal(begun.ticket, 2);
  assert.equal(begun.state.snapshot, null);
  assert.equal(completeCheck(begun.state, 1, snapshot).snapshot, null);
  assert.equal(completeCheck(begun.state, 2, snapshot).snapshot?.matched_ids.length, 0);
  assert.equal(isAbortError({ name: "AbortError" }), true);
  assert.equal(isAbortError(new Error("nope")), false);
  assert.equal(isIdentityField("email"), true);
  assert.equal(isIdentityField("preferred_name"), false);
  assert.equal(isIdentityField("facility_id"), false);
});

test("validation guards facility, status, email, and match preconditions", () => {
  const values = baseValues({ status: "active", email: "not-an-email" });
  const plan = planSubmission({
    values,
    facilities,
    snapshot: checked(values),
    reason: REASON,
  });
  assert.equal(plan.ok, false);
  const inactive = planSubmission({
    values: baseValues(),
    facilities: [{ id: "fac-1", active: false, timezone: "UTC" }],
    snapshot: checked(baseValues()),
    reason: REASON,
  });
  assert.equal(inactive.ok, false);
  if (!inactive.ok) assert.match(inactive.error, /active facility/);
  assert.equal(matchCheckBlock(baseValues({ first_name: " " })), "Enter first and last name before checking matches.");
  assert.equal(reasonLength(9), false);
  assert.equal(reasonLength(2001), false);
  assert.equal(registrationDirty(baseValues(), baseValues(), ""), false);
  assert.equal(registrationDirty(baseValues(), baseValues(), "note"), true);
});

function reasonLength(length: number): boolean {
  const values = baseValues();
  const plan = planSubmission({
    values,
    facilities,
    snapshot: checked(values, undefined, {matches:[{id:"match",mrn:"NX0000000001",first_name:"Synthetic",last_name:"Match",preferred_name:"",birth_date:values.birth_date,facility_id:values.facility_id,status:"inquiry",reasons:["legal_name_and_birth_date"]}],restricted_matches:false}),
    reason: "x".repeat(length),
  });
  return plan.ok;
}

test("choice helpers keep the current assignment visible without widening inactive staff", () => {
  const staff = [
    { id: "s1", active: true, role: "clinician" },
    { id: "s2", active: false, role: "clinician" },
    { id: "s3", active: true, role: "support" },
    { id: "s4", active: true, role: "admin" },
  ];
  assert.deepEqual(
    clinicianChoices(staff, "s2").map((row) => row.id),
    ["s1", "s2", "s4"],
  );
  assert.deepEqual(
    activeFacilityChoices([
      { id: "a", active: true },
      { id: "b", active: false },
    ]).map((row) => row.id),
    ["a"],
  );
  const current = { id: "b", active: false };
  assert.deepEqual(
    withCurrentRow([{ id: "a", active: true }], current).map((row) => row.id),
    ["a", "b"],
  );
});

test("reverting identity does not keep a restricted review requirement", () => {
  const initial = { id: "client-9", updated_at: "v", ...baseValues(), status: "intake" };
  const loaded = valuesFromRow(initial);
  const gate = duplicateGate({
    values: loaded,
    initial,
    snapshot: checked(
      { ...loaded, last_name: "Other" },
      "client-9",
      { matches: [], restricted_matches: true },
    ),
    reason: "",
  });
  assert.deepEqual(gate, { ok: true, review: null });
});
