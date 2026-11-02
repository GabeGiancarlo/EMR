/** Pure client registration rules. No I/O and no chart merging. */
import {
  facilityLocalToday,
  hasCompletedAdultYears,
  latestEligibleBirthDate,
  parseIsoDate,
} from "./adult.ts";

export const IDENTITY_FIELDS = [
  "first_name",
  "last_name",
  "birth_date",
  "email",
  "phone",
] as const;

export const CREATE_STATUSES = ["inquiry", "intake"] as const;
export const DUPLICATE_REASON_MIN = 10;
export const DUPLICATE_REASON_MAX = 2000;
export const CLINICAL_ROLES = ["clinician", "clinical_lead", "admin"] as const;

export const RESTRICTED_SAVE_MESSAGE =
  "Saving is blocked because a possible match is outside your access. Hidden identities are not shown or counted.";

export const MATCH_CHECK_STALE_MESSAGE =
  "Check possible matches for the current name, date of birth, email, and phone before saving.";

export type IdentityField = (typeof IDENTITY_FIELDS)[number];

export type RegistrationValues = {
  first_name: string;
  last_name: string;
  preferred_name: string;
  pronouns: string;
  preferred_language: string;
  birth_date: string;
  email: string;
  phone: string;
  address_line1: string;
  address_line2: string;
  city: string;
  region: string;
  postal_code: string;
  country: string;
  emergency_contact: string;
  facility_id: string;
  primary_clinician_id: string;
  status: string;
};

export type RegistrationRow = {
  id?: string;
  updated_at?: unknown;
  [key: string]: unknown;
};

export type FacilityChoice = {
  id: string;
  active?: unknown;
  timezone?: unknown;
  name?: unknown;
};

export type StaffChoice = {
  id: string;
  active?: unknown;
  role?: unknown;
  name?: unknown;
};

export type MatchCandidate = {
  id: string;
  mrn: string;
  first_name: string;
  last_name: string;
  preferred_name: string;
  birth_date: string;
  facility_id: string;
  status: string;
  reasons: string[];
};

export type MatchResponse = {
  matches: MatchCandidate[];
  restricted_matches: boolean;
};

export type MatchRequest = {
  first_name: string;
  last_name: string;
  birth_date: string;
  email: string;
  phone: string;
  exclude_client_id?: string;
};

export type MatchSnapshot = {
  request: MatchRequest;
  matched_ids: string[];
  restricted_matches: boolean;
  matches: MatchCandidate[];
};

export type CheckState = {
  generation: number;
  snapshot: MatchSnapshot | null;
};

export type DuplicateReview = {
  matched_ids: string[];
  reason: string;
};

const TEXT_KEYS = [
  "first_name",
  "last_name",
  "preferred_name",
  "pronouns",
  "preferred_language",
  "birth_date",
  "email",
  "phone",
  "address_line1",
  "address_line2",
  "city",
  "region",
  "postal_code",
  "country",
  "emergency_contact",
  "facility_id",
  "primary_clinician_id",
  "status",
] as const;

export function emptyValues(): RegistrationValues {
  return {
    first_name: "",
    last_name: "",
    preferred_name: "",
    pronouns: "",
    preferred_language: "",
    birth_date: "",
    email: "",
    phone: "",
    address_line1: "",
    address_line2: "",
    city: "",
    region: "",
    postal_code: "",
    country: "",
    emergency_contact: "",
    facility_id: "",
    primary_clinician_id: "",
    status: "inquiry",
  };
}

function cell(row: RegistrationRow | null | undefined, key: string): string {
  const value = row?.[key];
  return value == null ? "" : String(value);
}

export function valuesFromRow(row?: RegistrationRow | null): RegistrationValues {
  const values = emptyValues();
  if (!row) return values;
  for (const key of TEXT_KEYS) values[key] = cell(row, key);
  if (!values.status) values.status = "inquiry";
  return values;
}

export function isIdentityField(key: string): key is IdentityField {
  return (IDENTITY_FIELDS as readonly string[]).includes(key);
}

export function reviewRequired(
  values: RegistrationValues,
  initial?: RegistrationRow | null,
): boolean {
  if (!initial?.id) return true;
  const loaded = valuesFromRow(initial);
  return IDENTITY_FIELDS.some((key) => values[key].trim() !== loaded[key].trim());
}

export function needsAdultCheck(
  values: RegistrationValues,
  initial?: RegistrationRow | null,
): boolean {
  if (!initial?.id) return true;
  const loadedBirth = cell(initial, "birth_date").trim();
  const loadedFacility = cell(initial, "facility_id");
  return (
    values.birth_date.trim() !== loadedBirth || values.facility_id !== loadedFacility
  );
}

export function matchRequestFromValues(
  values: RegistrationValues,
  excludeClientId?: string,
): MatchRequest {
  const request: MatchRequest = {
    first_name: values.first_name.trim(),
    last_name: values.last_name.trim(),
    birth_date: values.birth_date.trim(),
    email: values.email.trim(),
    phone: values.phone.trim(),
  };
  if (excludeClientId) request.exclude_client_id = excludeClientId;
  return request;
}

export function sameMatchRequest(a: MatchRequest, b: MatchRequest): boolean {
  return (
    a.first_name === b.first_name &&
    a.last_name === b.last_name &&
    a.birth_date === b.birth_date &&
    a.email === b.email &&
    a.phone === b.phone &&
    (a.exclude_client_id ?? "") === (b.exclude_client_id ?? "")
  );
}

export function isSnapshotCurrent(
  values: RegistrationValues,
  snapshot: MatchSnapshot | null,
  excludeClientId?: string,
): boolean {
  if (!snapshot) return false;
  return sameMatchRequest(
    snapshot.request,
    matchRequestFromValues(values, excludeClientId),
  );
}

function textField(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function parseCandidate(item: unknown): MatchCandidate | null {
  if (!item || typeof item !== "object" || Array.isArray(item)) return null;
  const row = item as Record<string, unknown>;
  if (typeof row.id !== "string" || row.id.trim() === "") return null;
  if (["mrn", "first_name", "last_name", "preferred_name", "birth_date", "facility_id", "status"].some(key=>typeof row[key]!=="string") || !parseIsoDate(row.birth_date as string)) return null;
  const reasons = Array.isArray(row.reasons)
    ? row.reasons.filter((reason): reason is string => typeof reason === "string")
    : [];
  return {
    id: row.id,
    mrn: textField(row.mrn),
    first_name: textField(row.first_name),
    last_name: textField(row.last_name),
    preferred_name: textField(row.preferred_name),
    birth_date: textField(row.birth_date),
    facility_id: textField(row.facility_id),
    status: textField(row.status),
    reasons,
  };
}

export function parseMatchResponse(
  data: unknown,
): { ok: true; value: MatchResponse } | { ok: false; error: string } {
  const error = "Match results could not be read. Nothing was saved.";
  if (!data || typeof data !== "object" || Array.isArray(data)) return { ok: false, error };
  const record = data as Record<string, unknown>;
  if (typeof record.restricted_matches !== "boolean" || !Array.isArray(record.matches)) {
    return { ok: false, error };
  }
  if (record.matches.length > 1000) return { ok: false, error };
  const matches: MatchCandidate[] = [];
  for (const item of record.matches) {
    const parsed = parseCandidate(item);
    if (!parsed || matches.some(match=>match.id===parsed.id)) return { ok: false, error };
    matches.push(parsed);
  }
  return { ok: true, value: { matches, restricted_matches: record.restricted_matches } };
}

export function snapshotFromResponse(
  request: MatchRequest,
  response: MatchResponse,
): MatchSnapshot {
  return {
    request: { ...request },
    matched_ids: response.matches.map((match) => match.id),
    restricted_matches: response.restricted_matches,
    matches: response.matches.map((match) => ({
      ...match,
      reasons: [...match.reasons],
    })),
  };
}

export function invalidateCheck(state: CheckState): CheckState {
  return { generation: state.generation + 1, snapshot: null };
}

export function beginCheck(state: CheckState): { state: CheckState; ticket: number } {
  const ticket = state.generation + 1;
  return { ticket, state: { generation: ticket, snapshot: null } };
}

export function completeCheck(
  state: CheckState,
  ticket: number,
  snapshot: MatchSnapshot,
): CheckState {
  if (state.generation !== ticket) return state;
  return { generation: state.generation, snapshot };
}

export function isAbortError(err: unknown): boolean {
  return (
    typeof err === "object" &&
    err !== null &&
    "name" in err &&
    (err as { name?: unknown }).name === "AbortError"
  );
}

export function readApiError(data: unknown, status: number): string {
  if (data && typeof data === "object" && !Array.isArray(data)) {
    const error = (data as { error?: unknown }).error;
    if (error && typeof error === "object" && !Array.isArray(error)) {
      const message = (error as { message?: unknown }).message;
      if (typeof message === "string" && message.trim()) return message.trim();
    }
  }
  return `Request could not be completed (${status}).`;
}

export function activeFacilityChoices<T extends { id: string; active?: unknown }>(rows: T[]): T[] {
  return rows.filter((row) => row.active !== false);
}

export function clinicianChoices<T extends StaffChoice>(rows: T[], currentId: string): T[] {
  return rows.filter((row) => {
    if (currentId && row.id === currentId) return true;
    return (
      row.active !== false &&
      (CLINICAL_ROLES as readonly string[]).includes(String(row.role ?? ""))
    );
  });
}

export function withCurrentRow<T extends { id: string }>(rows: T[], current: T | undefined): T[] {
  if (!current || rows.some((row) => row.id === current.id)) return rows;
  return [...rows, current];
}

export function facilityZone(
  facilities: FacilityChoice[],
  facilityId: string,
): string | undefined {
  const facility = facilities.find((row) => row.id === facilityId);
  return facility && typeof facility.timezone === "string" ? facility.timezone : undefined;
}

export function eligibleBirthMax(
  facilities: FacilityChoice[],
  facilityId: string,
  now = new Date(),
): string | undefined {
  return (
    latestEligibleBirthDate(facilityLocalToday(facilityZone(facilities, facilityId), now)) ??
    undefined
  );
}

export function isPlausibleEmail(value: string): boolean {
  const text = value.trim();
  if (!text) return true;
  const at = text.indexOf("@");
  if (at <= 0 || at !== text.lastIndexOf("@")) return false;
  const domain = text.slice(at + 1);
  if (!domain.includes(".") || domain.startsWith(".") || domain.endsWith(".")) return false;
  return !text.includes(" ");
}

export function matchCheckBlock(values: RegistrationValues): string | null {
  if (!values.first_name.trim() || !values.last_name.trim()) {
    return "Enter first and last name before checking matches.";
  }
  if (!parseIsoDate(values.birth_date.trim())) {
    return "Enter a valid date of birth before checking matches.";
  }
  return null;
}

export function validateRegistration(input: {
  values: RegistrationValues;
  initial?: RegistrationRow | null;
  facilities: FacilityChoice[];
  clinicians?: StaffChoice[];
  now?: Date;
}): string | null {
  const { values, initial, facilities, clinicians, now } = input;
  if (!values.first_name.trim()) return "First name is required.";
  if (!values.last_name.trim()) return "Last name is required.";
  if (!parseIsoDate(values.birth_date.trim())) return "Enter a valid date of birth.";
  const facility = facilities.find((row) => row.id === values.facility_id);
  if (!values.facility_id || !facility || facility.active === false) {
    return "Choose an active facility.";
  }
  if (needsAdultCheck(values, initial)) {
    const today = facilityLocalToday(
      typeof facility.timezone === "string" ? facility.timezone : undefined,
      now,
    );
    if (!hasCompletedAdultYears(values.birth_date.trim(), today)) {
      return "Date of birth is not eligible under the adult program rule for the selected facility’s local date.";
    }
  }
  if (!isPlausibleEmail(values.email)) return "Enter a valid email address.";
  if (!initial?.id && !(CREATE_STATUSES as readonly string[]).includes(values.status)) {
    return "New charts can start as inquiry or intake only.";
  }
  if (
    values.primary_clinician_id &&
    clinicians &&
    !clinicians.some((row) => row.id === values.primary_clinician_id)
  ) {
    return "Choose an active primary clinician or leave the field unassigned.";
  }
  return null;
}

export function reasonError(reason: string, matchCount: number): string | null {
  const text = reason.trim();
  if (text.length >= DUPLICATE_REASON_MIN && text.length <= DUPLICATE_REASON_MAX) return null;
  if (matchCount > 0) {
    return "To create a separate chart, explain why the visible matches are not this person (10 to 2000 characters).";
  }
  return "Record a match-review note of 10 to 2000 characters before saving.";
}

export function duplicateGate(input: {
  values: RegistrationValues;
  initial?: RegistrationRow | null;
  snapshot: MatchSnapshot | null;
  reason: string;
}): { ok: true; review: DuplicateReview | null } | { ok: false; error: string } {
  if (!reviewRequired(input.values, input.initial)) return { ok: true, review: null };
  const exclude = input.initial?.id || undefined;
  if (!input.snapshot || !isSnapshotCurrent(input.values, input.snapshot, exclude)) {
    return { ok: false, error: MATCH_CHECK_STALE_MESSAGE };
  }
  if (input.snapshot.restricted_matches) {
    return { ok: false, error: RESTRICTED_SAVE_MESSAGE };
  }
  if (input.snapshot.matches.length === 0) return { ok: true, review: null };
  const reasonProblem = reasonError(input.reason, input.snapshot.matches.length);
  if (reasonProblem) return { ok: false, error: reasonProblem };
  return {
    ok: true,
    review: {
      matched_ids: input.snapshot.matched_ids.slice(),
      reason: input.reason.trim(),
    },
  };
}

export function buildSubmitBody(input: {
  values: RegistrationValues;
  initial?: RegistrationRow | null;
  review: DuplicateReview | null;
}): Record<string, unknown> {
  const values = input.values;
  const initial = input.initial;
  const body: Record<string, unknown> = {
    first_name: values.first_name.trim(),
    last_name: values.last_name.trim(),
    preferred_name: values.preferred_name.trim(),
    pronouns: values.pronouns.trim(),
    preferred_language: values.preferred_language.trim(),
    birth_date: values.birth_date.trim() || null,
    email: values.email.trim(),
    phone: values.phone.trim(),
    address_line1: values.address_line1.trim(),
    address_line2: values.address_line2.trim(),
    city: values.city.trim(),
    region: values.region.trim(),
    postal_code: values.postal_code.trim(),
    country: values.country.trim(),
    emergency_contact: values.emergency_contact.trim(),
    facility_id: values.facility_id.trim() || null,
    primary_clinician_id: values.primary_clinician_id.trim() || null,
  };
  if (!initial?.id) body.status = values.status;
  else if (initial.status != null && String(initial.status) !== "") {
    body.status = String(initial.status);
  }
  if (initial?.id) body._expected_updated_at = initial.updated_at;
  if (input.review) {
    body._duplicate_review = {
      matched_ids: input.review.matched_ids.slice(),
      reason: input.review.reason,
    };
  }
  return body;
}

export function planSubmission(input: {
  values: RegistrationValues;
  initial?: RegistrationRow | null;
  facilities: FacilityChoice[];
  clinicians?: StaffChoice[];
  snapshot: MatchSnapshot | null;
  reason: string;
  now?: Date;
}): { ok: true; body: Record<string, unknown> } | { ok: false; error: string } {
  const fieldError = validateRegistration(input);
  if (fieldError) return { ok: false, error: fieldError };
  const gate = duplicateGate(input);
  if (!gate.ok) return gate;
  return {
    ok: true,
    body: buildSubmitBody({
      values: input.values,
      initial: input.initial,
      review: gate.review,
    }),
  };
}

export function registrationDirty(
  values: RegistrationValues,
  baseline: RegistrationValues,
  reason: string,
  baselineReason = "",
): boolean {
  if (reason !== baselineReason) return true;
  return (Object.keys(values) as (keyof RegistrationValues)[]).some(
    (key) => values[key] !== baseline[key],
  );
}

export function matchSummary(
  snapshot: MatchSnapshot | null,
  checking: boolean,
): string {
  if (checking) return "Checking possible matches.";
  if (!snapshot) return "No match check is current for these identity details.";
  if (snapshot.restricted_matches) return RESTRICTED_SAVE_MESSAGE;
  if (snapshot.matches.length === 0) return "No visible matches for these details.";
  const count = snapshot.matches.length;
  return `${count} visible possible match${count === 1 ? "" : "es"}. Review them before creating a separate chart.`;
}
