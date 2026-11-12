import { useEffect, useId, useRef, useState } from "react";
import type { FormEvent } from "react";
import { authorizedFetch, date, label, value } from "./api";
import type { Bootstrap, Row } from "./api";
import { facilityLocalToday } from "./adult.ts";
import { Badge, Button, Icon } from "./ui";
import "./clientRegistration.css";
import {
  RESTRICTED_SAVE_MESSAGE,
  activeFacilityChoices,
  beginCheck,
  clinicianChoices,
  completeCheck,
  eligibleBirthMax,
  facilityZone,
  invalidateCheck,
  isAbortError,
  isIdentityField,
  matchCheckBlock,
  matchRequestFromValues,
  matchSummary,
  needsAdultCheck,
  parseMatchResponse,
  planSubmission,
  readApiError,
  registrationDirty,
  reviewRequired,
  sameMatchRequest,
  snapshotFromResponse,
  valuesFromRow,
  withCurrentRow,
  type CheckState,
  type MatchRequest,
  type RegistrationValues,
} from "./registrationRules.ts";

export type ClientRegistrationProps = {
  data: Bootstrap;
  initial?: Row;
  onSubmit: (body: Record<string, unknown>) => Promise<void>;
  onOpenExisting: (clientId: string) => void;
  onDirtyChange: (dirty: boolean) => void;
};

async function postMatches(body: MatchRequest, signal: AbortSignal): Promise<unknown> {
  const response = await authorizedFetch("/api/intake/matches", {
    method: "POST",
    signal,
    cache: "no-store",
    headers: {
      Accept: "application/json",
      "Content-Type": "application/json",
    },
    body: JSON.stringify(body),
  });
  const payload = await response.json().catch(() => null);
  if (!response.ok) throw new Error(readApiError(payload, response.status));
  return payload;
}

function personLabel(match: {
  first_name: string;
  last_name: string;
  preferred_name: string;
  mrn: string;
  id: string;
}): string {
  const legal = `${match.first_name} ${match.last_name}`.trim();
  const preferred = match.preferred_name.trim();
  const named = preferred && preferred !== legal ? `${legal} (${preferred})` : legal;
  return named || (match.mrn ? `MRN ${match.mrn}` : match.id);
}

export function ClientRegistration({
  data,
  initial,
  onSubmit,
  onOpenExisting,
  onDirtyChange,
}: ClientRegistrationProps) {
  const baseId = useId();
  const fieldId = (key: string) => `${baseId}-${key}`;
  const loadedKey = `${initial?.id ?? ""}:${String(initial?.updated_at ?? "")}`;
  const initialRef = useRef(initial);
  initialRef.current = initial;
  const [values, setValues] = useState<RegistrationValues>(() => valuesFromRow(initial));
  const [saved, setSaved] = useState<RegistrationValues>(() => valuesFromRow(initial));
  const [reason, setReason] = useState("");
  const [savedReason, setSavedReason] = useState("");
  const [error, setError] = useState("");
  const [errorFocus, setErrorFocus] = useState(0);
  const [busy, setBusy] = useState(false);
  const [checking, setChecking] = useState(false);
  const [discardArmed, setDiscardArmed] = useState(false);
  const checkRef = useRef<CheckState>({ generation: 0, snapshot: null });
  const [checkState, setCheckState] = useState<CheckState>(checkRef.current);
  const abortRef = useRef<AbortController | null>(null);
  const valuesRef = useRef(values);
  valuesRef.current = values;
  const alertRef = useRef<HTMLDivElement>(null);
  const keepRef = useRef<HTMLButtonElement>(null);
  const onDirtyRef = useRef(onDirtyChange);
  onDirtyRef.current = onDirtyChange;

  const dirty = registrationDirty(values, saved, reason, savedReason);
  const excludeId = initial?.id;
  const mustReview = reviewRequired(values, initial);
  const currentSnapshot =
    mustReview && isCurrent(values, checkState, excludeId) ? checkState.snapshot : null;
  const canDiscard = dirty || reason !== "" || checkState.snapshot !== null;
  const zone = facilityZone(data.facilities, values.facility_id);
  const localToday = values.facility_id ? facilityLocalToday(zone) : "";
  const birthMax = needsAdultCheck(values, initial)
    ? eligibleBirthMax(data.facilities, values.facility_id)
    : undefined;
  const facilityOptions = withCurrentRow(
    activeFacilityChoices(data.facilities),
    data.facilities.find((row) => row.id === values.facility_id),
  );
  const clinicianOptions = withCurrentRow(
    clinicianChoices(data.staff, values.primary_clinician_id),
    data.staff.find((row) => row.id === values.primary_clinician_id),
  );
  const mrn = initial?.mrn == null || initial.mrn === "" ? "" : String(initial.mrn);

  useEffect(() => {
    onDirtyRef.current(dirty || busy || checking);
  }, [dirty, busy, checking]);

  useEffect(() => () => onDirtyRef.current(false), []);

  useEffect(() => {
    if (!dirty) return undefined;
    const onUnload = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", onUnload);
    return () => window.removeEventListener("beforeunload", onUnload);
  }, [dirty]);

  useEffect(() => () => abortRef.current?.abort(), []);

  useEffect(() => {
    if (errorFocus) alertRef.current?.focus();
  }, [errorFocus]);

  useEffect(() => {
    if (discardArmed) keepRef.current?.focus();
  }, [discardArmed]);

  useEffect(() => {
    const next = valuesFromRow(initialRef.current);
    abortRef.current?.abort();
    const cleared = invalidateCheck(checkRef.current);
    checkRef.current = cleared;
    setCheckState(cleared);
    setValues(next);
    setSaved(next);
    setReason("");
    setSavedReason("");
    setError("");
    setDiscardArmed(false);
    setChecking(false);
  }, [loadedKey]);

  function showError(message: string, focus = false) {
    setError(message);
    if (focus) setErrorFocus((tick) => tick + 1);
  }

  function syncCheck(next: CheckState) {
    checkRef.current = next;
    setCheckState(next);
  }

  function invalidate() {
    abortRef.current?.abort();
    syncCheck(invalidateCheck(checkRef.current));
  }

  function update(key: keyof RegistrationValues, next: string) {
    if (valuesRef.current[key] === next) return;
    const updated = { ...valuesRef.current, [key]: next };
    valuesRef.current = updated;
    setValues(updated);
    setDiscardArmed(false);
    setError("");
    if (isIdentityField(key)) invalidate();
  }

  function discard() {
    abortRef.current?.abort();
    const next = saved;
    valuesRef.current = next;
    setValues(next);
    setReason(savedReason);
    setError("");
    setDiscardArmed(false);
    syncCheck(invalidateCheck(checkRef.current));
    setChecking(false);
  }

  async function startCheck() {
    const current = valuesRef.current;
    const block = matchCheckBlock(current);
    if (block) {
      showError(block);
      return;
    }
    abortRef.current?.abort();
    const controller = new AbortController();
    abortRef.current = controller;
    const begun = beginCheck(checkRef.current);
    syncCheck(begun.state);
    const ticket = begun.ticket;
    setChecking(true);
    setError("");
    const requestBody = matchRequestFromValues(current, excludeId);
    try {
      const raw = await postMatches(requestBody, controller.signal);
      if (controller.signal.aborted || checkRef.current.generation !== ticket) return;
      if (
        !sameMatchRequest(
          matchRequestFromValues(valuesRef.current, initialRef.current?.id),
          requestBody,
        )
      ) {
        syncCheck(invalidateCheck(checkRef.current));
        return;
      }
      const parsed = parseMatchResponse(raw);
      if (!parsed.ok) {
        showError(parsed.error);
        return;
      }
      syncCheck(
        completeCheck(
          checkRef.current,
          ticket,
          snapshotFromResponse(requestBody, parsed.value),
        ),
      );
    } catch (err) {
      if (
        controller.signal.aborted ||
        isAbortError(err) ||
        checkRef.current.generation !== ticket
      ) {
        return;
      }
      showError(
        err instanceof Error
          ? err.message
          : "Possible matches could not be checked. Nothing was saved.",
      );
    } finally {
      if (abortRef.current === controller) {
        abortRef.current = null;
        setChecking(false);
      }
    }
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (busy) return;
    if (checking) {
      showError("Wait for the match check to finish before saving.", true);
      return;
    }
    const plan = planSubmission({
      values: valuesRef.current,
      initial,
      facilities: data.facilities,
      clinicians: clinicianOptions,
      snapshot: checkRef.current.snapshot,
      reason,
    });
    if (!plan.ok) {
      showError(plan.error, true);
      return;
    }
    setBusy(true);
    setError("");
    try {
      await onSubmit(plan.body);
      setSaved(valuesRef.current);
      setSavedReason(reviewRequired(valuesRef.current, initial) ? reason.trim() : "");
      if (!reviewRequired(valuesRef.current, initial)) setReason("");
    } catch (err) {
      showError(
        err instanceof Error ? err.message : "Unable to save. Please try again.",
        true,
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <form
      className="client-registration-form"
      aria-label="Client registration"
      aria-busy={busy || checking}
      autoComplete="off"
      noValidate
      onSubmit={submit}
    >
      <div className="form-body client-registration">
        <fieldset disabled={busy}>
          <legend>Identity</legend>
          <p className="registration-kicker">
            Confirm the legal name and date of birth before opening a patient record.
          </p>
          <div className="form-grid">
            {initial?.id && (
              <div className="form-field">
                <label htmlFor={fieldId("mrn")}>Medical record number</label>
                <input
                  id={fieldId("mrn")}
                  value={mrn || "Not assigned"}
                  readOnly
                  aria-readonly="true"
                />
              </div>
            )}
            <TextField
              id={fieldId("first_name")}
              label="First name"
              required
              value={values.first_name}
              autoComplete="off"
              onChange={(next) => update("first_name", next)}
            />
            <TextField
              id={fieldId("last_name")}
              label="Last name"
              required
              value={values.last_name}
              autoComplete="off"
              onChange={(next) => update("last_name", next)}
            />
            <TextField
              id={fieldId("preferred_name")}
              label="Preferred name"
              value={values.preferred_name}
              autoComplete="off"
              onChange={(next) => update("preferred_name", next)}
            />
            <TextField
              id={fieldId("pronouns")}
              label="Pronouns"
              value={values.pronouns}
              autoComplete="off"
              onChange={(next) => update("pronouns", next)}
            />
            <TextField
              id={fieldId("preferred_language")}
              label="Preferred language"
              value={values.preferred_language}
              hint="Language the person prefers for care conversations."
              onChange={(next) => update("preferred_language", next)}
            />
            <div className="form-field">
              <label htmlFor={fieldId("birth_date")}>
                Date of birth<span className="required"> *</span>
              </label>
              <input
                id={fieldId("birth_date")}
                type="date"
                required
                aria-required="true"
                aria-describedby={fieldId("birth-hint")}
                value={values.birth_date}
                max={birthMax}
                onChange={(event) => update("birth_date", event.target.value)}
                onInput={(event) => update("birth_date", event.currentTarget.value)}
              />
              <small id={fieldId("birth-hint")}>
                Adults 18+ only.
                {localToday
                  ? ` Age is checked as of ${localToday} at the selected facility.`
                  : " Choose a facility so age uses that location’s date."}
              </small>
            </div>
          </div>
        </fieldset>

        <fieldset disabled={busy}>
          <legend>Contact</legend>
          <div className="form-grid">
            <TextField
              id={fieldId("email")}
              label="Email"
              type="email"
              value={values.email}
              autoComplete="off"
              onChange={(next) => update("email", next)}
            />
            <TextField
              id={fieldId("phone")}
              label="Phone"
              type="tel"
              value={values.phone}
              autoComplete="off"
              onChange={(next) => update("phone", next)}
            />
            <TextField
              id={fieldId("emergency_contact")}
              label="Emergency contact"
              value={values.emergency_contact}
              hint="Name, relationship, and contact number."
              wide
              onChange={(next) => update("emergency_contact", next)}
            />
          </div>
        </fieldset>

        <fieldset disabled={busy}>
          <legend>Address</legend>
          <div className="form-grid">
            <TextField
              id={fieldId("address_line1")}
              label="Address line 1"
              value={values.address_line1}
              wide
              onChange={(next) => update("address_line1", next)}
            />
            <TextField
              id={fieldId("address_line2")}
              label="Address line 2"
              value={values.address_line2}
              wide
              onChange={(next) => update("address_line2", next)}
            />
            <TextField
              id={fieldId("city")}
              label="City"
              value={values.city}
              onChange={(next) => update("city", next)}
            />
            <TextField
              id={fieldId("region")}
              label="State or region"
              value={values.region}
              onChange={(next) => update("region", next)}
            />
            <TextField
              id={fieldId("postal_code")}
              label="Postal code"
              value={values.postal_code}
              onChange={(next) => update("postal_code", next)}
            />
            <TextField
              id={fieldId("country")}
              label="Country"
              value={values.country}
              onChange={(next) => update("country", next)}
            />
          </div>
        </fieldset>

        <fieldset disabled={busy}>
          <legend>Assignment</legend>
          <p className="registration-kicker">
            {initial?.id
              ? "Editing keeps the current intake stage. Saving sends the loaded version so a newer edit is not overwritten."
              : "New charts start as inquiry or intake."}
          </p>
          <div className="form-grid">
            <div className="form-field">
              <label htmlFor={fieldId("facility_id")}>
                Facility<span className="required"> *</span>
              </label>
              <select
                id={fieldId("facility_id")}
                value={values.facility_id}
                aria-required="true"
                onChange={(event) => update("facility_id", event.target.value)}
              >
                <option value="">Choose an active facility…</option>
                {facilityOptions.map((row) => (
                  <option value={row.id} key={row.id}>
                    {facilityOptionLabel(row)}
                  </option>
                ))}
              </select>
            </div>
            <div className="form-field">
              <label htmlFor={fieldId("primary_clinician_id")}>Primary clinician</label>
              <select
                id={fieldId("primary_clinician_id")}
                value={values.primary_clinician_id}
                onChange={(event) => update("primary_clinician_id", event.target.value)}
              >
                <option value="">Not assigned</option>
                {clinicianOptions.map((row) => (
                  <option value={row.id} key={row.id}>
                    {staffOptionLabel(row)}
                  </option>
                ))}
              </select>
            </div>
            {initial?.id ? (
              <div className="form-field">
                <span id={fieldId("status-label")}>Intake stage</span>
                <div aria-labelledby={fieldId("status-label")}>
                  <Badge status={initial.status || "not set"} />
                </div>
                <small>This form does not change the current stage.</small>
              </div>
            ) : (
              <div className="form-field">
                <label htmlFor={fieldId("status")}>
                  Intake stage<span className="required"> *</span>
                </label>
                <select
                  id={fieldId("status")}
                  value={values.status}
                  aria-required="true"
                  onChange={(event) => update("status", event.target.value)}
                >
                  <option value="inquiry">Inquiry</option>
                  <option value="intake">Intake</option>
                </select>
              </div>
            )}
          </div>
        </fieldset>

        {mustReview && (
          <fieldset disabled={busy}>
            <legend>Possible matches</legend>
            <p className="registration-kicker">
              Check for an existing chart using the name, date of birth, and contact details.
              Review any matches before creating a separate record.
            </p>
            <Button
              variant="secondary"
              onClick={() => void startCheck()}
              disabled={checking || busy}
            >
              {checking ? "Checking…" : "Check possible matches"}
            </Button>
            <p className="registration-status" aria-live="polite">
              {matchSummary(currentSnapshot, checking)}
            </p>
            {currentSnapshot?.restricted_matches && (
              <p className="registration-block" role="alert">
                {RESTRICTED_SAVE_MESSAGE}
              </p>
            )}
            {currentSnapshot && currentSnapshot.matches.length > 0 && (
              <ul className="match-list">
                {currentSnapshot.matches.map((match, index) => {
                  const facility = data.facilities.find((row) => row.id === match.facility_id);
                  const title = personLabel(match);
                  return (
                    <li className="match-card" key={`${match.id}-${index}`}>
                      <div className="match-identity">
                        <strong>{title}</strong>
                        <span>{match.mrn ? `MRN ${match.mrn}` : "MRN not returned"}</span>
                        <span>{date(match.birth_date)}</span>
                        <span>
                          {facility ? value(facility, "name") : "Facility unavailable"}
                        </span>
                        {match.status && <Badge status={match.status} />}
                        {match.reasons.length > 0 && (
                          <span>Match reasons: {match.reasons.map((item) => label(item)).join(", ")}</span>
                        )}
                      </div>
                      <Button
                        variant="secondary"
                        onClick={() => onOpenExisting(match.id)}
                      >
                        {`Discard this entry and open ${title}`}
                      </Button>
                    </li>
                  );
                })}
              </ul>
            )}
            {currentSnapshot && currentSnapshot.matches.length > 0 && !currentSnapshot.restricted_matches && (
              <div className="form-field full registration-reason">
                <label htmlFor={fieldId("review-reason")}>
                  Duplicate review note<span className="required"> *</span>
                </label>
                <textarea
                  id={fieldId("review-reason")}
                  rows={3}
                  maxLength={2000}
                  value={reason}
                  aria-required="true"
                  aria-describedby={fieldId("review-hint")}
                  onChange={(event) => {
                    setReason(event.target.value);
                    setDiscardArmed(false);
                    setError("");
                  }}
                />
                <small id={fieldId("review-hint")}>
                  {currentSnapshot.matches.length
                    ? "Required to create a separate chart. Explain why each visible match is not this person. 10 to 2000 characters."
                    : "Required to record that this match result was reviewed. 10 to 2000 characters."}
                </small>
              </div>
            )}
          </fieldset>
        )}

        {error && (
          <div
            className="error-banner"
            role="alert"
            tabIndex={-1}
            ref={alertRef}
            id={fieldId("error")}
          >
            {error}
          </div>
        )}
      </div>
      <div className="modal-foot">
        <span className="form-assurance">
          <Icon name="shield" size={15} /> Review possible matches before saving.
        </span>
        {discardArmed ? (
          <div className="registration-confirm" role="group" aria-label="Confirm discard">
            <p>Discard unsaved registration entries? This does not delete a saved chart.</p>
            <Button onClick={discard}>Discard now</Button>
            <button type="button" className="button secondary" ref={keepRef} onClick={() => setDiscardArmed(false)}>
              Keep editing
            </button>
          </div>
        ) : (
          <Button
            variant="secondary"
            onClick={() => setDiscardArmed(true)}
            disabled={!canDiscard || busy}
          >
            {initial?.id ? "Discard changes" : "Reset form"}
          </Button>
        )}
        <Button type="submit" disabled={busy || checking}>
          {busy ? "Saving…" : initial?.id ? "Save changes" : "Save client"}
          <Icon name="arrow" size={16} />
        </Button>
      </div>
    </form>
  );
}

function isCurrent(
  values: RegistrationValues,
  state: CheckState,
  excludeId?: string,
): state is CheckState & { snapshot: NonNullable<CheckState["snapshot"]> } {
  if (!state.snapshot) return false;
  return sameMatchRequest(
    state.snapshot.request,
    matchRequestFromValues(values, excludeId),
  );
}

function facilityOptionLabel(row: Row): string {
  const name = value(row, "name") || "Facility";
  const zone = value(row, "timezone");
  const base = zone ? `${name} · ${zone}` : name;
  return row.active === false ? `${base} (inactive)` : base;
}

function staffOptionLabel(row: Row): string {
  const name = value(row, "name") || "Team member";
  const role = value(row, "role");
  const base = role ? `${name} · ${label(role)}` : name;
  return row.active === false ? `${base} (inactive)` : base;
}

function TextField({
  id,
  label: title,
  value: fieldValue,
  onChange,
  required = false,
  type = "text",
  hint,
  wide = false,
  autoComplete,
}: {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  required?: boolean;
  type?: "text" | "email" | "tel";
  hint?: string;
  wide?: boolean;
  autoComplete?: string;
}) {
  return (
    <div className={`form-field${wide ? " full" : ""}`}>
      <label htmlFor={id}>
        {title}
        {required && <span className="required"> *</span>}
      </label>
      <input
        id={id}
        type={type}
        value={fieldValue}
        aria-required={required || undefined}
        aria-describedby={hint ? `${id}-hint` : undefined}
        autoComplete={autoComplete}
        spellCheck={type === "text" ? false : undefined}
        onChange={(event) => onChange(event.target.value)}
      />
      {hint && <small id={`${id}-hint`}>{hint}</small>}
    </div>
  );
}
