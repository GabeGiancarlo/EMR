import { Choice, FieldInput } from "./nexus-ui/FieldControls";
import { NxIcon } from "./nexus-ui/Icon";
import { formatDateText } from "./dateText";
import { useEffect, useId, useRef, useState } from "react";
import type { FormEvent } from "react";
import { authorizedFetch, date, label, value } from "./api";
import type { Bootstrap, Row } from "./api";
import { facilityLocalToday, parseIsoDate } from "./adult.ts";
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
  isPlausibleEmail,
  isIdentityField,
  matchCheckBlock,
  matchRequestFromValues,
  matchSummary,
  needsAdultCheck,
  parseMatchResponse,
  planSubmission,
  validateRegistration,
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
  // A new chart starts at the registrar's own facility, or the only one there is.
  const starting = (row: Row | undefined): RegistrationValues => {
    const start = valuesFromRow(row);
    if (row?.id || start.facility_id) return start;
    const open = activeFacilityChoices(data.facilities);
    const own = open.find((facility) => facility.id === data.actor?.facility_id);
    return { ...start, facility_id: own?.id ?? (open.length === 1 ? open[0].id : "") };
  };
  const [values, setValues] = useState<RegistrationValues>(() => starting(initial));
  const [saved, setSaved] = useState<RegistrationValues>(() => starting(initial));
  const [reason, setReason] = useState("");
  const [savedReason, setSavedReason] = useState("");
  const [error, setError] = useState("");
  const [errorFocus, setErrorFocus] = useState(0);
  const [busy, setBusy] = useState(false);
  const [checking, setChecking] = useState(false);
  const [step, setStep] = useState<"identity" | "assignment" | "review">("identity");
  const [fieldErrors, setFieldErrors] = useState<Partial<Record<keyof RegistrationValues, string>>>({});
  const contactKeys = ["email", "phone", "emergency_contact", "address_line1", "address_line2", "city", "region", "postal_code", "country"] as const;
  // Contact details open by themselves when there is something in them to see or fix.
  const [contactOpen, setContactOpen] = useState(() => contactKeys.some((key) => valuesFromRow(initial)[key].trim() !== ""));
  const assignmentHeading = useRef<HTMLLegendElement>(null);
  const reviewHeading = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    if (step === "assignment") assignmentHeading.current?.focus();
    if (step === "review") reviewHeading.current?.focus();
  }, [step]);
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
    const next = starting(initialRef.current);
    abortRef.current?.abort();
    const cleared = invalidateCheck(checkRef.current);
    checkRef.current = cleared;
    setCheckState(cleared);
    setValues(next);
    setSaved(next);
    setStep("identity");
    setFieldErrors({});
    setContactOpen(contactKeys.some((key) => next[key].trim() !== ""));
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
    if (fieldErrors[key]) setFieldErrors(({ [key]: _fixed, ...rest }) => rest);
    if (isIdentityField(key)) invalidate();
  }
  /** Mark one field, say what to enter, and put the cursor there. */
  function fieldProblem(key: keyof RegistrationValues, message: string) {
    setFieldErrors({ [key]: message });
    setError("");
    if ((contactKeys as readonly string[]).includes(key)) setContactOpen(true);
    requestAnimationFrame(() => document.getElementById(fieldId(key))?.focus());
  }

  function discard() {
    abortRef.current?.abort();
    setStep("identity");
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
    if (step === "identity") {
      // Each problem is shown on its own field, on the step where that field is.
      if (!values.first_name.trim()) return fieldProblem("first_name", "Enter the legal first name.");
      if (!values.last_name.trim()) return fieldProblem("last_name", "Enter the legal last name.");
      if (!parseIsoDate(values.birth_date.trim())) return fieldProblem("birth_date", "Enter the date of birth, like 6/15/1990.");
      if (!isPlausibleEmail(values.email)) return fieldProblem("email", "Enter an email address, like name@example.org, or leave it blank.");
      const invalid = matchCheckBlock(values);
      if (invalid) { showError(invalid, true); return; }
      setError("");
      setFieldErrors({});
      setStep("assignment");
      return;
    }
    if (step === "assignment") {
      if (!values.facility_id) return fieldProblem("facility_id", "Choose a facility.");
      const invalid = validateRegistration({ values, initial, facilities: data.facilities, clinicians: clinicianOptions });
      if (invalid) { showError(invalid, true); return; }
      setError("");
      setFieldErrors({});
      setStep("review");
      if (mustReview && !currentSnapshot) void startCheck();
      return;
    }
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
        <ol className="registration-steps" aria-label="Registration steps">
          {([["identity", "Person"], ["assignment", "Care team"], ["review", "Review"]] as const).map(([id, title], index) => {
            const at = ["identity", "assignment", "review"].indexOf(step);
            return (
              <li key={id} aria-current={step === id ? "step" : undefined} data-done={index < at ? "" : undefined}>
                <span className="nx-step" aria-hidden="true">{index < at ? <NxIcon name="check" size={12} /> : index + 1}</span>
                {title}
              </li>
            );
          })}
        </ol>
        {step === "identity" ? <>
        <fieldset disabled={busy}>
          <legend>Legal name and date of birth</legend>
          <div className="form-grid">
            {initial?.id && (
              <div className="form-field full">
                <label htmlFor={fieldId("mrn")}>Medical record number</label>
                <input
                  id={fieldId("mrn")}
                  value={mrn || "Not assigned"}
                  readOnly
                  aria-readonly="true"
                />
                <small>Assigned by Nexus. It never changes.</small>
              </div>
            )}
            <TextField
              id={fieldId("first_name")}
              label="First name"
              required
              value={values.first_name}
              error={fieldErrors.first_name}
              autoComplete="off"
              onChange={(next) => update("first_name", next)}
            />
            <TextField
              id={fieldId("last_name")}
              label="Last name"
              required
              value={values.last_name}
              error={fieldErrors.last_name}
              autoComplete="off"
              onChange={(next) => update("last_name", next)}
            />
            <div className="form-field" data-invalid={fieldErrors.birth_date ? "" : undefined}>
              <label htmlFor={fieldId("birth_date")}>
                Date of birth<span className="required" aria-hidden="true"> *</span>
              </label>
              <FieldInput
                id={fieldId("birth_date")}
                type="date"
                required
                aria-required="true"
                aria-invalid={fieldErrors.birth_date ? true : undefined}
                aria-describedby={fieldId("birth-hint")}
                value={values.birth_date}
                max={birthMax}
                onChange={(event) => update("birth_date", event.target.value)}
                onInput={(event) => update("birth_date", event.currentTarget.value)}
              />
              {fieldErrors.birth_date && <span className="nx-field-error" role="alert"><NxIcon name="error" size={14} />{fieldErrors.birth_date}</span>}
              <small id={fieldId("birth-hint")}>
                Adults 18 and over.
                {localToday
                  ? ` Age is checked as of ${formatDateText(localToday)} at the chosen facility.`
                  : " Age is checked against the facility's date on the next step."}
              </small>
            </div>
          </div>
        </fieldset>
        <fieldset disabled={busy}>
          <legend>How to address them <small className="optional-mark">Optional</small></legend>
          <div className="form-grid">
            <TextField
              id={fieldId("preferred_name")}
              label="Preferred name"
              value={values.preferred_name}
              hint="Shown as “Goes by” on the chart."
              autoComplete="off"
              onChange={(next) => update("preferred_name", next)}
            />
            <TextField
              id={fieldId("pronouns")}
              label="Pronouns"
              value={values.pronouns}
              suggest={["she/her", "he/him", "they/them"]}
              autoComplete="off"
              onChange={(next) => update("pronouns", next)}
            />
            <TextField
              id={fieldId("preferred_language")}
              label="Preferred language"
              value={values.preferred_language}
              hint="For care conversations."
              suggest={["English", "Spanish"]}
              onChange={(next) => update("preferred_language", next)}
            />
          </div>
        </fieldset>
        <details className="form-disclosure" open={contactOpen} onToggle={(event) => setContactOpen(event.currentTarget.open)}>
          <summary>Contact and address<small>{contactKeys.some((key) => values[key].trim()) ? "On file" : "Optional"}</small></summary>
        <fieldset disabled={busy}>
          <legend>Contact</legend>
          <div className="form-grid">
            <TextField
              id={fieldId("phone")}
              label="Phone"
              type="tel"
              value={values.phone}
              autoComplete="off"
              onChange={(next) => update("phone", next)}
            />
            <TextField
              id={fieldId("email")}
              label="Email"
              type="email"
              value={values.email}
              error={fieldErrors.email}
              autoComplete="off"
              onChange={(next) => update("email", next)}
            />
            <TextField
              id={fieldId("emergency_contact")}
              label="Emergency contact"
              value={values.emergency_contact}
              hint="Name, relationship and number."
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
              label="Street address"
              value={values.address_line1}
              wide
              onChange={(next) => update("address_line1", next)}
            />
            <TextField
              id={fieldId("address_line2")}
              label="Apartment, suite or unit"
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
              label="State"
              value={values.region}
              onChange={(next) => update("region", next)}
            />
            <TextField
              id={fieldId("postal_code")}
              label="ZIP code"
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
        </details>
        </> : step === "assignment" ? <>
        <fieldset disabled={busy}>
          <legend tabIndex={-1} ref={assignmentHeading}>Where care happens and who leads it</legend>
          <p className="registration-kicker">
            {initial?.id
              ? "Editing keeps the current intake stage. If someone else saved a newer version, Nexus tells you and keeps your entries."
              : "A new chart starts as an inquiry or in intake. Admission comes later, from the chart."}
          </p>
          <div className="form-grid">
            <div className="form-field" data-invalid={fieldErrors.facility_id ? "" : undefined}>
              <label htmlFor={fieldId("facility_id")}>
                Facility<span className="required" aria-hidden="true"> *</span>
              </label>
              <Choice
                id={fieldId("facility_id")}
                value={values.facility_id}
                required
                aria-invalid={fieldErrors.facility_id ? true : undefined}
                onChange={(event) => update("facility_id", event.target.value)}
              >
                <option value="">Choose…</option>
                {facilityOptions.map((row) => (
                  <option value={row.id} key={row.id}>
                    {facilityOptionLabel(row)}
                  </option>
                ))}
              </Choice>
              {fieldErrors.facility_id && <span className="nx-field-error" role="alert"><NxIcon name="error" size={14} />{fieldErrors.facility_id}</span>}
            </div>
            <div className="form-field">
              <label htmlFor={fieldId("primary_clinician_id")}>Primary clinician</label>
              <Choice
                id={fieldId("primary_clinician_id")}
                value={values.primary_clinician_id}
                onChange={(event) => update("primary_clinician_id", event.target.value)}
              >
                <option value="">Unassigned</option>
                {clinicianOptions.map((row) => (
                  <option value={row.id} key={row.id}>
                    {staffOptionLabel(row)}
                  </option>
                ))}
              </Choice>
              <small>Can be assigned later. Clinicians see only their own clients.</small>
            </div>
            {initial?.id ? (
              <div className="form-field">
                <span id={fieldId("status-label")}>Intake stage</span>
                <div aria-labelledby={fieldId("status-label")}>
                  <Badge status={initial.status || "not set"} />
                </div>
                <small>Changed from the chart's intake tab, not here.</small>
              </div>
            ) : (
              <div className="form-field">
                <label htmlFor={fieldId("status")}>
                  Intake stage<span className="required" aria-hidden="true"> *</span>
                </label>
                <Choice
                  id={fieldId("status")}
                  value={values.status}
                  required
                  onChange={(event) => update("status", event.target.value)}
                >
                  <option value="inquiry">Inquiry</option>
                  <option value="intake">Intake</option>
                </Choice>
                <small>Inquiry: a first contact. Intake: referral and paperwork are under way.</small>
              </div>
            )}
          </div>
        </fieldset>
        </> : <>
          <h3 className="registration-review-title" tabIndex={-1} ref={reviewHeading}>Check the details</h3>
          <dl className="registration-summary">
            <dt>Legal name</dt><dd>{values.first_name.trim()} {values.last_name.trim()}</dd>
            {values.preferred_name.trim() && <><dt>Goes by</dt><dd>{values.preferred_name.trim()}</dd></>}
            {values.pronouns.trim() && <><dt>Pronouns</dt><dd>{values.pronouns.trim()}</dd></>}
            <dt>Date of birth</dt><dd>{formatDateText(values.birth_date) || values.birth_date}</dd>
            {values.preferred_language.trim() && <><dt>Language</dt><dd>{values.preferred_language.trim()}</dd></>}
            {(values.phone.trim() || values.email.trim()) && <><dt>Contact</dt><dd>{[values.phone.trim(), values.email.trim()].filter(Boolean).join(" · ")}</dd></>}
            {values.emergency_contact.trim() && <><dt>Emergency contact</dt><dd>{values.emergency_contact.trim()}</dd></>}
            {[values.address_line1, values.city, values.region, values.postal_code].some((part) => part.trim()) && <><dt>Address</dt><dd>{[values.address_line1, values.address_line2, values.city, [values.region, values.postal_code].filter((part) => part.trim()).join(" "), values.country].map((part) => part.trim()).filter(Boolean).join(", ")}</dd></>}
            <dt>Facility</dt><dd>{value(facilityOptions.find(row => row.id === values.facility_id), "name") || "Facility unavailable"}</dd>
            <dt>Primary clinician</dt><dd>{value(clinicianOptions.find(row => row.id === values.primary_clinician_id), "name") || "Unassigned"}</dd>
            <dt>Intake stage</dt><dd>{label(initial?.id ? String(initial.status || "not set") : values.status)}</dd>
          </dl>
        {mustReview && (
          <fieldset disabled={busy}>
            <legend>Is there already a chart?</legend>
            <p className="registration-kicker">
              Nexus looks for an existing chart by name, date of birth and contact details.
              Look at any match before creating a second record.
            </p>
            <Button
              variant="secondary"
              onClick={() => void startCheck()}
              disabled={checking || busy}
            >
              {checking ? "Checking" : currentSnapshot ? "Check again" : "Check for an existing chart"}
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
                        {`Open ${title} instead`}
                      </Button>
                    </li>
                  );
                })}
              </ul>
            )}
            {currentSnapshot && currentSnapshot.matches.length > 0 && !currentSnapshot.restricted_matches && (
              <div className="form-field full registration-reason">
                <label htmlFor={fieldId("review-reason")}>
                  Why this is a different person<span className="required" aria-hidden="true"> *</span>
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
                    ? "Needed to create a separate chart. Say why each match shown is not this person. 10 to 2,000 characters."
                    : "Needed to record that this result was reviewed. 10 to 2,000 characters."}
                </small>
              </div>
            )}
          </fieldset>
        )}

        </>}
        {error && (
          <div
            className="error-banner"
            role="alert"
            tabIndex={-1}
            ref={alertRef}
            id={fieldId("error")}
          >
            <NxIcon name="error" size={17} />
            <span>{error}</span>
          </div>
        )}
      </div>
      <div className="modal-foot">
        <span className="form-assurance">
          <Icon name="shield" size={15} /> {step === "review" ? "Checked for an existing chart before saving." : "Nothing is saved until the last step."}
        </span>
        {discardArmed ? (
          <div className="registration-confirm" role="group" aria-label="Confirm discard">
            <p>Discard what you have entered? A saved chart is not affected.</p>
            <Button onClick={discard}>Discard entries</Button>
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
            {initial?.id ? "Discard changes" : "Start over"}
          </Button>
        )}
        {step !== "identity" && <Button variant="secondary" disabled={busy} onClick={() => { setStep(step === "review" ? "assignment" : "identity"); setError(""); }}>Back</Button>}
        <button type="submit" className="button" data-busy={busy ? "" : undefined} disabled={busy || checking}>
          {busy ? <><NxIcon name="sun" size={16} className="nx-spin" />Saving</> : <>{step === "identity" ? "Next: care team" : step === "assignment" ? "Next: review" : initial?.id ? "Save changes" : "Create client"}<Icon name="arrow" size={16} /></>}
        </button>
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
  error,
  suggest,
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
  /** A sentence that says what to enter. The text already typed stays. */
  error?: string;
  /** Common answers; one click fills the field. */
  suggest?: string[];
}) {
  const describedBy = [hint ? `${id}-hint` : "", error ? `${id}-error` : ""].filter(Boolean).join(" ") || undefined;
  return (
    <div className={`form-field${wide ? " full" : ""}`} data-invalid={error ? "" : undefined}>
      <label htmlFor={id}>
        {title}
        {required && <span className="required" aria-hidden="true"> *</span>}
      </label>
      <FieldInput
        id={id}
        type={type}
        value={fieldValue}
        aria-required={required || undefined}
        aria-invalid={error ? true : undefined}
        aria-describedby={describedBy}
        autoComplete={autoComplete}
        inputMode={type === "tel" ? "tel" : type === "email" ? "email" : undefined}
        spellCheck={type === "text" ? false : undefined}
        onChange={(event) => onChange(event.target.value)}
      />
      {suggest && (
        <div className="nx-suggest" role="group" aria-label={`Common answers for ${title.toLowerCase()}`}>
          {suggest.map((item) => (
            <button type="button" key={item} className="nx-chip" aria-pressed={fieldValue === item} onClick={() => onChange(item)}>{item}</button>
          ))}
        </div>
      )}
      {error && <span className="nx-field-error" id={`${id}-error`} role="alert"><NxIcon name="error" size={14} />{error}</span>}
      {hint && <small id={`${id}-hint`}>{hint}</small>}
    </div>
  );
}
