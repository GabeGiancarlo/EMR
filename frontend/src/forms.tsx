import { Choice, DateTimeField, FieldInput } from "./nexus-ui/FieldControls";
import { NxIcon } from "./nexus-ui/Icon";
import { useRef, useState } from "react";
import type { FormEvent } from "react";
import { clientName, date, label, value } from "./api";
import type { Bootstrap, FormField, Resource, Row } from "./api";
import { Button, Icon } from "./ui";
import { normalizeFieldValue } from "./serialize";
import { claimFormDefaults, initialFormValues } from "./formValues";
import { availableServices, serviceDurationError, serviceSnapshot } from "./programRouting";
import { facilityWallTime, occurrenceLabel, resolveFacilityTime, schedulingZone, wallTimeCandidates } from "./schedulingTime";
import { durationText, shiftWallTime, wallMinutesBetween, zoneLabel } from "./dateText";
import {
  ADULT_PROGRAM_HINT,
  facilityLocalToday,
  latestEligibleBirthDate,
} from "./adult.ts";

type InputKind =
  | "text"
  | "email"
  | "tel"
  | "date"
  | "datetime-local"
  | "number"
  | "textarea"
  | "select"
  | "checkbox"
  | "money"
  | "fields";
export type Field = {
  key: string;
  label: string;
  type?: InputKind;
  required?: boolean;
  options?: string[];
  /** Wording for options whose stored value is not what people read. */
  optionLabels?: Record<string, string>;
  ref?: Resource;
  catalog?: "services";
  hint?: string;
  placeholder?: string;
  /** Common answers offered under a text field; one click fills it. */
  suggest?: string[] | "locations";
  min?: number;
  max?: number;
  /** "today" fills the current date when the form opens for a new record. */
  default?: unknown;
};
const f = (key: string, title: string, extra: Partial<Field> = {}): Field => ({
  key,
  label: title,
  ...extra,
});
const reference = (
  key: string,
  title: string,
  ref: Resource,
  required = true,
  extra: Partial<Field> = {},
): Field => f(key, title, { type: "select", ref, required, ...extra });
const options = (
  key: string,
  title: string,
  values: string[],
  required = true,
  extra: Partial<Field> = {},
): Field => f(key, title, { type: "select", options: values, required, ...extra });

const ZONES: Record<string, string> = {
  "America/New_York": "Eastern · New York",
  "America/Chicago": "Central · Chicago",
  "America/Denver": "Mountain · Denver",
  "America/Phoenix": "Mountain, no daylight time · Phoenix",
  "America/Los_Angeles": "Pacific · Los Angeles",
  "America/Anchorage": "Alaska · Anchorage",
  "Pacific/Honolulu": "Hawaii · Honolulu",
};

/**
 * Every record the shared form can open. `title` names one record in a sentence ("New session").
 * Descriptions follow the brand voice: what this is, what it means, then stop.
 */
export const definitions: Record<
  Resource,
  { title: string; description: string; fields: Field[] }
> = {
  clients: {
    title: "Client",
    description:
      "Register the person, then continue to referral review and admission.",
    fields: [
      f("first_name", "First name", { required: true }),
      f("last_name", "Last name", { required: true }),
      f("preferred_name", "Preferred name"),
      f("birth_date", "Date of birth", {
        type: "date",
        required: true,
        hint: ADULT_PROGRAM_HINT,
      }),
      f("email", "Email", { type: "email" }),
      f("phone", "Phone", { type: "tel" }),
      reference("facility_id", "Facility", "facilities"),
      reference("primary_clinician_id", "Primary clinician", "staff", false),
      options("status", "Intake stage", ["inquiry", "intake", "active"]),
      f("emergency_contact", "Emergency contact", {
        hint: "Name, relationship, and contact number.",
      }),
    ],
  },
  episodes: {
    title: "Care episode",
    description:
      "One course of treatment: a client, a level of care and a facility.",
    fields: [
      reference("client_id", "Client", "clients"),
      reference("facility_id", "Facility", "facilities"),
      options("level_of_care", "Level of care", [
        "outpatient",
        "iop",
        "php",
        "residential",
        "detox",
      ]),
      f("start_date", "Start date", { type: "date", required: true }),
      options("status", "Status", ["planned", "active"]),
    ],
  },
  sessions: {
    title: "Session",
    description: "Put a visit on the calendar. It stays linked to the client's care episode, notes and billing.",
    fields: [
      reference("client_id", "Client", "clients"),
      reference("episode_id", "Care episode", "episodes", true, {
        hint: "The course of care this visit belongs to.",
      }),
      reference("clinician_id", "Clinician", "staff"),
      options("kind", "Session type", [
        "individual",
        "assessment",
        "family",
        "case_management",
      ]),
      f("starts_at", "Start", { type: "datetime-local", required: true }),
      f("ends_at", "End", { type: "datetime-local", required: true }),
      f("location", "Room or location", { required: true, suggest: "locations", placeholder: "Room 2, or Telehealth" }),
      options("status", "Status", [
        "scheduled",
        "completed",
        "cancelled",
        "no_show",
      ]),
    ],
  },
  notes: {
    title: "Clinical note",
    description:
      "Write and review the note. Signing keeps the original; later changes are amendments.",
    fields: [
      reference("client_id", "Client", "clients"),
      reference("session_id", "Session", "sessions", false),
      reference("template_id", "Form template", "templates", false),
      f("title", "Note title", { required: true }),
      f("body", "Clinical narrative", {
        type: "textarea",
        required: true,
        hint: "Read the whole narrative before signing. Keep to what this record is for.",
      }),
    ],
  },
  tasks: {
    title: "Task",
    description:
      "One next step, with an owner and a date, visible to clinical and operations staff.",
    fields: [
      f("title", "Task", { required: true, placeholder: "Call the payer about authorization" }),
      f("description", "Details", { type: "textarea", hint: "Anything the next person needs to pick this up." }),
      options("priority", "Priority", ["normal", "low", "high", "urgent"]),
      reference("assignee_id", "Assigned to", "staff", false),
      f("due_date", "Due date", { type: "date" }),
      reference("client_id", "About client", "clients", false, { hint: "Optional. Links the task to a chart." }),
      options("status", "Status", ["open", "in_progress", "done"]),
    ],
  },
  policies: {
    title: "Insurance policy",
    description:
      "The coverage used to prepare claims. Recording it here does not verify eligibility with the payer.",
    fields: [
      reference("client_id", "Client", "clients"),
      f("payer_name", "Payer", { required: true, placeholder: "Plan or payer name" }),
      f("member_id", "Member ID", { required: true, hint: "As printed on the card." }),
      f("group_number", "Group number"),
      f("effective_date", "Effective date", { type: "date", required: true }),
      f("termination_date", "Termination date", { type: "date", hint: "Leave blank while coverage continues." }),
      options("status", "Status", ["active", "inactive"]),
    ],
  },
  authorizations: {
    title: "Service authorization",
    description:
      "The payer's approval: which service, how many units, and for how long.",
    fields: [
      reference("client_id", "Client", "clients"),
      reference("policy_id", "Policy", "policies"),
      f("authorization_number", "Authorization number", { required: true, hint: "The number from the payer letter." }),
      f("service_code", "Service code", { required: true, placeholder: "90837" }),
      f("units_approved", "Approved units", {
        type: "number",
        required: true,
        min: 1,
      }),
      f("starts_on", "Valid from", { type: "date", required: true }),
      f("ends_on", "Valid through", { type: "date", required: true }),
      options("status", "Status", ["active", "expired", "exhausted"]),
    ],
  },
  claims: {
    title: "Claim",
    description:
      "Link a completed session to coverage. Readiness checks run before you record a submission.",
    fields: [
      reference("client_id", "Client", "clients"),
      reference("session_id", "Session", "sessions", true, { hint: "No-show sessions cannot be billed." }),
      reference("policy_id", "Policy", "policies"),
      reference("authorization_id", "Authorization", "authorizations", false),
      f("service_code", "Service code", { required: true, hint: "CPT or HCPCS. Filled from the session's service." }),
      f("diagnosis_code", "Diagnosis code", { required: true, placeholder: "F10.20", hint: "ICD-10-CM." }),
      f("units", "Units of service", {
        type: "number",
        required: true,
        min: 1,
        default: 1,
      }),
      f("amount_cents", "Charge", {
        type: "money",
        required: true,
        min: 0.01,
      }),
    ],
  },
  payments: {
    title: "Payment",
    description:
      "Add a payment to the claim's ledger. Payments are never edited; a mistake is reversed.",
    fields: [
      reference("claim_id", "Claim", "claims"),
      f("amount_cents", "Amount received", {
        type: "money",
        required: true,
        min: 0.01,
      }),
      f("received_on", "Received on", { type: "date", required: true, default: "today" }),
      options("source", "Source", ["payer", "client", "adjustment"]),
      f("reference", "Payment reference", { required: true, hint: "Check, EFT trace or receipt number." }),
    ],
  },
  staff: {
    title: "Team member",
    description:
      "Who they are and what they can do here. Sign-in is matched by work email.",
    fields: [
      f("name", "Full name", { required: true }),
      f("email", "Work email", { type: "email", required: true, hint: "Sign-in matches this address." }),
      options("role", "Workspace role", [
        "clinician",
        "clinical_lead",
        "billing",
        "executive",
        "support",
        "admin",
      ], true, { hint: "Sets what this person can read and change." }),
      reference("facility_id", "Facility", "facilities"),
      f("active", "Active team member", { type: "checkbox", default: true }),
    ],
  },
  facilities: {
    title: "Facility",
    description: "A location where care happens. Its time zone sets session times.",
    fields: [
      f("name", "Facility name", { required: true }),
      f("timezone", "Time zone", {
        type: "select",
        options: Object.keys(ZONES),
        optionLabels: ZONES,
        required: true,
        default: "America/New_York",
        hint: "Sessions at this facility are scheduled in this time.",
      }),
      f("active", "Active facility", { type: "checkbox", default: true }),
    ],
  },
  templates: {
    title: "Form template",
    description:
      "Build your organization's own form. Have your clinical lead approve the content before it is used.",
    fields: [
      f("name", "Form name", { required: true }),
      options("kind", "Purpose", [
        "intake",
        "progress",
        "assessment",
        "discharge",
        "consent",
      ]),
      f("description", "When to use this form", { type: "textarea" }),
      f("applies_to_stage", "Client stage", {
        type: "select",
        options: ["any", "intake", "active", "discharged"],
        required: true,
        default: "any",
        hint: "Any matches every stage.",
      }),
      f("applies_to_care_level", "Care level", {
        type: "select",
        options: ["any", "outpatient", "iop", "php"],
        required: true,
        default: "any",
        hint: "Residential and detox episodes match only Any.",
      }),
      f("required_for_stage", "Required when this rule matches", {
        type: "checkbox",
        default: false,
        hint: "Unticked, the form is suggested. This does not create a legal or clinical requirement.",
      }),
      f("fields", "Questions", { type: "fields", required: true }),
      f("active", "Available for documentation", {
        type: "checkbox",
        default: true,
      }),
    ],
  },
  consents: {
    title: "Consent record",
    description:
      "What the client agreed to and when. Keep the signed original in your approved records system.",
    fields: [
      reference("client_id", "Client", "clients"),
      f("kind", "Consent type", {
        required: true,
        suggest: ["treatment", "recording", "processing", "release of information"],
        hint: "Treatment is checked at admission. Recording and processing are checked before a visit is recorded or transcribed.",
      }),
      f("scope", "What it permits", { type: "textarea", required: true }),
      f("granted_on", "Granted on", { type: "date", required: true, default: "today" }),
      f("expires_on", "Expires on", { type: "date", hint: "Leave blank if it has no end date." }),
    ],
  },
  exceptions: {
    title: "Access exception",
    description:
      "Temporary access to one kind of record, with a reason and an end. Administrative access cannot be granted here.",
    fields: [
      reference("staff_id", "Team member", "staff"),
      options("resource", "Records", [
        "clients",
        "episodes",
        "sessions",
        "notes",
        "tasks",
        "policies",
        "authorizations",
        "claims",
        "payments",
        "templates",
        "consents",
        "treatment_plans",
        "goals",
        "plan_reviews",
        "form_responses",
        "groups",
        "group_attendance",
      ]),
      options("access_mode", "Access", ["read", "write"]),
      f("reason", "Reason", { type: "textarea", required: true, hint: "Shown in the audit trail." }),
      f("expires_at", "Ends", { type: "datetime-local", required: true }),
      f("revoked", "Revoked", { type: "checkbox", default: false }),
    ],
  },
  treatment_plans: {
    title: "Treatment plan",
    description:
      "The client's priorities and strengths. Add measurable goals before the plan is signed.",
    fields: [
      reference("client_id", "Client", "clients"),
      reference("episode_id", "Care episode", "episodes"),
      f("title", "Plan title", { required: true }),
      f("presenting_needs", "Presenting needs", {
        type: "textarea",
        required: true,
      }),
      f("strengths", "Strengths and resources", {
        type: "textarea",
        required: true,
      }),
      f("next_review_date", "Next review", { type: "date" }),
    ],
  },
  goals: {
    title: "Treatment goal",
    description:
      "One measurable goal. Goals can be added while the plan is a draft.",
    fields: [
      reference("plan_id", "Treatment plan", "treatment_plans"),
      f("description", "Goal", { type: "textarea", required: true }),
      f("measure", "How progress is measured", { required: true }),
      f("baseline", "Starting point", { required: true }),
      f("target", "Target", { required: true }),
      f("target_date", "Target date", { type: "date" }),
    ],
  },
  plan_reviews: {
    title: "Plan review",
    description:
      "Progress against the signed plan. The plan's original goals stay as they were signed.",
    fields: [
      reference("plan_id", "Treatment plan", "treatment_plans"),
      f("summary", "Review summary", { type: "textarea", required: true }),
      f("next_review_date", "Next review", { type: "date" }),
    ],
  },
  form_responses: {
    title: "Form response",
    description:
      "Answer a saved form, review it, then sign.",
    fields: [],
  },
  groups: {
    title: "Group session",
    description:
      "One shared session. Add participants afterwards; each gets their own attendance, note and billing.",
    fields: [
      f("title", "Group name", { required: true, placeholder: "Thursday skills group" }),
      reference("facility_id", "Facility", "facilities"),
      reference("clinician_id", "Facilitator", "staff"),
      options("kind", "Group purpose", [
        "process",
        "psychoeducation",
        "skills",
        "family",
        "support",
      ]),
      f("starts_at", "Start", { type: "datetime-local", required: true }),
      f("ends_at", "End", { type: "datetime-local", required: true }),
      f("capacity", "Places", {
        type: "number",
        required: true,
        min: 1,
        max: 100,
        default: 12,
      }),
      f("location", "Room or location", { required: true, suggest: "locations" }),
    ],
  },
  group_attendance: {
    title: "Group participant",
    description:
      "Add a client to the group. Nexus creates their own linked session for notes and billing.",
    fields: [
      reference("group_id", "Group", "groups"),
      reference("client_id", "Client", "clients"),
      reference("episode_id", "Active care episode", "episodes"),
      options("status", "Attendance", [
        "scheduled",
        "attended",
        "no_show",
        "excused",
      ]),
    ],
  },
};

type Section = { title: string; hint?: string; keys: string[] };
/** How each record's fields are grouped and ordered. Fields not listed follow, untitled. */
const sectionPlan: Partial<Record<Resource, Section[]>> = {
  sessions: [
    { title: "Client and care team", keys: ["client_id", "episode_id", "clinician_id"] },
    { title: "Session details", keys: ["program_service_id", "kind", "starts_at", "ends_at", "location", "status"] },
  ],
  groups: [
    { title: "Group and facilitator", keys: ["title", "facility_id", "clinician_id"] },
    { title: "Session details", keys: ["program_service_id", "kind", "starts_at", "ends_at", "capacity", "location"] },
  ],
  tasks: [
    { title: "What needs doing", keys: ["title", "description", "priority"] },
    { title: "Who and when", keys: ["assignee_id", "due_date", "client_id", "status"] },
  ],
  policies: [
    { title: "Coverage", keys: ["client_id", "payer_name", "member_id", "group_number"] },
    { title: "Dates", keys: ["effective_date", "termination_date", "status"] },
  ],
  authorizations: [
    { title: "Coverage", keys: ["client_id", "policy_id"] },
    { title: "What was approved", keys: ["authorization_number", "service_code", "units_approved"] },
    { title: "Valid period", keys: ["starts_on", "ends_on", "status"] },
  ],
  claims: [
    { title: "Care and coverage", keys: ["client_id", "session_id", "policy_id", "authorization_id"] },
    { title: "Service and charge", keys: ["service_code", "diagnosis_code", "units", "amount_cents"] },
  ],
  staff: [
    { title: "Person", keys: ["name", "email"] },
    { title: "Role and location", keys: ["role", "facility_id", "active"] },
  ],
  templates: [
    { title: "About this form", keys: ["name", "kind", "description"] },
    { title: "When it applies", keys: ["applies_to_stage", "applies_to_care_level", "required_for_stage"] },
    { title: "Questions", keys: ["fields", "active"] },
  ],
  consents: [
    { title: "Consent", keys: ["client_id", "kind", "scope"] },
    { title: "Dates", keys: ["granted_on", "expires_on"] },
  ],
  exceptions: [
    { title: "Who and what", keys: ["staff_id", "resource", "access_mode"] },
    { title: "Why and how long", keys: ["reason", "expires_at", "revoked"] },
  ],
  treatment_plans: [
    { title: "Plan", keys: ["client_id", "episode_id", "title", "next_review_date"] },
    { title: "Needs and strengths", keys: ["presenting_needs", "strengths"] },
  ],
  goals: [
    { title: "Goal", keys: ["plan_id", "description"] },
    { title: "How it is measured", keys: ["measure", "baseline", "target", "target_date"] },
  ],
};

/** The verb on the button that creates each record. Editing always says "Save changes". */
const createLabel: Partial<Record<Resource, string>> = {
  sessions: "Schedule session",
  groups: "Schedule group",
  tasks: "Create task",
  policies: "Add policy",
  authorizations: "Add authorization",
  claims: "Create claim",
  payments: "Record payment",
  staff: "Add team member",
  facilities: "Add facility",
  templates: "Create form",
  consents: "Record consent",
  exceptions: "Grant access",
  treatment_plans: "Create plan",
  goals: "Add goal",
  plan_reviews: "Record review",
  group_attendance: "Add participant",
  episodes: "Save episode",
};

/** A new record always starts in its first state, so the form does not ask. */
const settledOnCreate: Partial<Record<Resource, string[]>> = {
  sessions: ["status"],
  tasks: ["status"],
  exceptions: ["revoked"],
  group_attendance: ["status"],
};

/** Fields that depend on the chosen client, and what to say before one is chosen. */
const followsClient = ["episodes", "sessions", "policies", "authorizations"];

function optionName(resource: Resource, row: Row, data: Bootstrap) {
  const client = data.clients.find((c) => c.id === value(row, "client_id"));
  if (resource === "clients") return clientName(row);
  if (resource === "episodes")
    return `${label(value(row, "level_of_care"))} · from ${date(row.start_date)} · ${label(value(row, "status"))}`;
  if (resource === "sessions")
    return `${clientName(client)} · ${label(value(row, "kind"))} · ${date(row.starts_at, true)}`;
  if (resource === "groups")
    return `${value(row, "title")} · ${date(row.starts_at, true)}`;
  if (resource === "policies")
    return `${value(row, "payer_name")} · ${value(row, "member_id")}`;
  if (resource === "authorizations")
    return `${value(row, "authorization_number")} · ${value(row, "service_code")}`;
  if (resource === "claims")
    return `${clientName(client)} · ${value(row, "service_code")} · ${row.id.slice(0, 8)}`;
  return value(row, "name") || value(row, "title") || row.id;
}
function FieldBuilder({
  fields,
  onChange,
}: {
  fields: FormField[];
  onChange: (v: FormField[]) => void;
}) {
  const update = (i: number, patch: Partial<FormField>) =>
    onChange(fields.map((f, j) => (i === j ? { ...f, ...patch } : f)));
  return (
    <div className="field-builder">
      {fields.length === 0 && <p className="field-builder-empty">No questions yet. Add the first one below.</p>}
      {fields.map((field, i) => (
        <div className="field-builder-row" key={i}>
          <span className="nx-step" aria-hidden="true">{i + 1}</span>
          <input
            aria-label={`Question ${i + 1}`}
            value={field.label}
            required
            placeholder="Question"
            onChange={(e) =>
              update(i, {
                label: e.target.value,
                key:
                  e.target.value
                    .toLowerCase()
                    .replace(/[^a-z0-9]+/g, "_")
                    .replace(/^_|_$/g, "") || `field_${i + 1}`,
              })
            }
          />
          <Choice
            aria-label={`Answer type for question ${i + 1}`}
            value={field.type}
            onChange={(e) => update(i, { type: e.target.value })}
          >
            <option value="textarea">Long answer</option>
            <option value="text">Short answer</option>
            <option value="date">Date</option>
            <option value="number">Number</option>
            <option value="checkbox">Tick box</option>
            <option value="select">Choose one</option>
          </Choice>
          <label className="check-inline">
            <input
              type="checkbox"
              checked={field.required}
              onChange={(e) => update(i, { required: e.target.checked })}
            />
            Required
          </label>
          <button
            type="button"
            className="icon-button"
            aria-label={`Remove question ${i + 1}`}
            title="Remove question"
            onClick={() => onChange(fields.filter((_, j) => j !== i))}
          >
            <Icon name="close" size={16} />
          </button>
          {field.type === "select" && (
            <div className="field-options">
              <label htmlFor={`field-options-${i}`}>
                Choices, one per line
              </label>
              <textarea
                id={`field-options-${i}`}
                rows={3}
                required
                value={(field.options || []).join("\n")}
                onChange={(e) =>
                  update(i, { options: e.target.value.split("\n") })
                }
              />
            </div>
          )}
        </div>
      ))}
      <Button
        variant="secondary small"
        onClick={() =>
          onChange([
            ...fields,
            {
              key: `field_${fields.length + 1}`,
              label: "",
              type: "textarea",
              required: true,
            },
          ])
        }
      >
        <Icon name="plus" size={16} /> Add question
      </Button>
    </div>
  );
}

const lowerFirst = (text: string) => /^[A-Z]{2}/.test(text) ? text : text.charAt(0).toLowerCase() + text.slice(1);
const article = (text: string) => (/^[aeiou]/i.test(text) ? "an" : "a");
const numberWord = (count: number) => ["No", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine"][count] ?? String(count);
const completeWall = (wall: unknown) => /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(String(wall ?? ""));
/** What to put in an optional menu's empty row, so "nothing" reads as a choice. */
const emptyChoice: Record<string, string> = {
  assignee_id: "Unassigned",
  primary_clinician_id: "Unassigned",
  client_id: "No client",
  authorization_id: "No authorization",
  session_id: "No session",
  template_id: "No form",
};

/** A sentence that says what to enter, for a control the browser reports as invalid. */
function fieldMessage(field: Field | undefined, control: HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement): string {
  const validity = control.validity;
  const name = lowerFirst(control.getAttribute("aria-label") || field?.label || "this field");
  if (field?.type === "fields") return "Give every question its wording, and every choose-one question its choices.";
  if (validity.customError) return control.closest(".nx-date, .nx-time") ? "" : control.validationMessage;
  if (validity.valueMissing) {
    return control instanceof HTMLSelectElement ? `Choose ${article(name)} ${name}.` : `Enter ${article(name)} ${name}.`;
  }
  if (validity.typeMismatch) return field?.type === "email" ? "Enter an email address, like name@example.org." : `Check the ${name}.`;
  if (validity.badInput) return "Enter a number.";
  if (validity.rangeUnderflow) return field?.type === "money" ? "Enter an amount above zero." : `Enter ${(control as HTMLInputElement).min} or more.`;
  if (validity.rangeOverflow) return `Enter ${(control as HTMLInputElement).max} or fewer.`;
  if (validity.stepMismatch) return field?.type === "money" ? "Use dollars and cents, like 125.00." : "Enter a whole number.";
  if (validity.tooLong) return `Shorten this to ${(control as HTMLInputElement).maxLength} characters.`;
  return control.validationMessage;
}

export function RecordForm({
  resource,
  data,
  initial = {},
  onSubmit,
  onCancel,
  fields: customFields,
  submitLabel,
}: {
  resource?: Resource;
  data: Bootstrap;
  initial?: Record<string, unknown>;
  onSubmit: (body: Record<string, unknown>) => Promise<void>;
  onCancel: () => void;
  fields?: Field[];
  submitLabel?: string;
}) {
  const creating = !initial.id;
  const scheduling = resource === "sessions" || resource === "groups";
  const baseFields = customFields || (resource ? definitions[resource].fields : []);
  const fields = data.program_configuration && scheduling && !customFields
    ? [...baseFields.slice(0, 2), f("program_service_id", "Service", {type:"select",catalog:"services",required:creating || !!initial.program_service_id,hint:"Sets the allowed length, the note template and the billing defaults."}), ...baseFields.slice(2)] : baseFields;
  // A new record starts in its first state. Those fields keep their default and are not asked.
  const settled = creating && resource && !customFields ? settledOnCreate[resource] ?? [] : [];
  const shown = fields.filter((field) => !settled.includes(field.key));
  const plan = resource && !(!creating && customFields && customFields.length < 3) ? sectionPlan[resource] : undefined;
  const groups = (plan ?? [])
    .map((section) => ({ title: section.title, hint: section.hint, fields: section.keys.flatMap((key) => shown.filter((field) => field.key === key)) }))
    .filter((group) => group.fields.length);
  const remaining = shown.filter((field) => !plan?.some((section) => section.keys.includes(field.key)));
  if (remaining.length) groups.push({ title: "", hint: undefined, fields: remaining });
  const numbered = groups.filter((group) => group.title).length > 1;
  const action = submitLabel || (creating && resource ? createLabel[resource] ?? `Save ${definitions[resource].title.toLowerCase()}` : "Save changes");
  const hasClientField = fields.some((field) => field.key === "client_id");

  function choicesFor(field: Field, current: Record<string, unknown>): Row[] {
    let choices = field.ref ? data[field.ref] || [] : [];
    if (field.catalog && scheduling) {
      choices = availableServices(data, resource, { ...initial, ...current });
      const saved = serviceSnapshot(initial as Row);
      if (saved && !choices.some((row) => row.id === saved.id) && initial.episode_id === current.episode_id && (resource !== "groups" || initial.facility_id === current.facility_id)) choices = [{ ...saved, _historical: true }, ...choices];
    }
    if (field.ref && followsClient.includes(field.ref) && current.client_id)
      choices = choices.filter((row) => value(row, "client_id") === current.client_id);
    // Active episodes and coverage first: they are what a new record almost always belongs to.
    if (field.ref === "episodes" || field.ref === "policies")
      choices = [...choices].sort((a, b) => Number(b.status === "active") - Number(a.status === "active"));
    if (field.ref === "staff")
      choices = choices.filter(
        (row) =>
          row.active !== false &&
          (!["clinician_id", "primary_clinician_id"].includes(field.key) ||
            ["clinician", "clinical_lead", "admin"].includes(value(row, "role"))),
      );
    if (field.ref === "facilities" || field.ref === "templates")
      choices = choices.filter((row) => row.active !== false);
    return choices;
  }
  /** When a new record has exactly one possible episode, policy or service, choose it. */
  function fillSingles(next: Record<string, unknown>) {
    if (!creating) return;
    for (const field of fields) {
      if (!field.required || next[field.key]) continue;
      if (["episode_id", "policy_id"].includes(field.key) && hasClientField && next.client_id) {
        // One choice, or one that is active among several (a withdrawn intake leaves its episode behind).
        const rows = choicesFor(field, next);
        const active = rows.filter((row) => row.status === "active");
        if (rows.length === 1) next[field.key] = rows[0].id;
        else if (active.length === 1) next[field.key] = active[0].id;
      } else if (field.catalog) {
        const rows = choicesFor(field, next);
        if (rows.length === 1) {
          next[field.key] = rows[0].id;
          if (resource === "sessions") next.kind = rows[0].session_kind;
        }
      } else if (field.key === "clinician_id") {
        // The client's own clinician, or whoever is scheduling when they see clients themselves.
        const rows = choicesFor(field, next);
        const primary = data.clients.find((row) => row.id === next.client_id)?.primary_clinician_id;
        const self = data.actor?.id;
        const pick = [primary, rows.length === 1 ? rows[0].id : undefined, data.actor?.role === "clinician" ? self : undefined]
          .find((id) => id && rows.some((row) => row.id === id));
        if (pick && (resource !== "sessions" || next.client_id)) next[field.key] = pick;
      }
    }
  }
  const [values, setValues] = useState<Record<string, unknown>>(() => {
    const today = new Date();
    const localToday = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, "0")}-${String(today.getDate()).padStart(2, "0")}`;
    const prepared = fields.map((field) => field.default === "today" ? { ...field, default: creating ? localToday : undefined } : field);
    const start = initialFormValues(prepared, data, initial, resource);
    fillSingles(start);
    return start;
  });
  const zone = schedulingZone(data, { ...initial, ...values }, resource);
  const [occurrences, setOccurrences] = useState<Record<string, string>>(() => Object.fromEntries(
    fields.filter(f => scheduling && !initial._require_time_occurrence && f.type === "datetime-local" && initial[f.key]).map(f => {
      const instant = new Date(String(initial[f.key]));
      return [f.key, Number.isFinite(instant.getTime()) ? new Date(Math.floor(instant.getTime() / 60_000) * 60_000).toISOString() : ""];
    }),
  ));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
  const formRef = useRef<HTMLFormElement>(null);
  const submitRef = useRef<HTMLButtonElement>(null);
  const alertRef = useRef<HTMLDivElement>(null);
  // Until someone sets the end by hand, a new visit's end follows its start and its service.
  const endFollows = useRef(creating);
  const claimSession = resource === "claims"
    ? data.sessions.find(row => row.id === (initial.id ? initial.session_id : values.session_id))
    : undefined;
  const noShowClaim = claimSession?.status === "no_show";
  const serviceOf = (current: Record<string, unknown>) => !scheduling ? undefined
    : current.program_service_id === initial.program_service_id ? serviceSnapshot(initial as Row)
    : data.program_services?.find(row => row.id === current.program_service_id);
  const selectedService = serviceOf(values);
  const set = (key: string, v: unknown) => {
    const prev = values;
    const next = { ...prev, [key]: v };
    if (key === "client_id" && v !== prev.client_id) {
      for (const field of fields) if (["episode_id", "session_id", "policy_id", "authorization_id"].includes(field.key)) next[field.key] = "";
    }
    if (scheduling && ["client_id","episode_id","facility_id"].includes(key) && v !== prev[key]) next.program_service_id="";
    if (["client_id", "episode_id", "facility_id"].includes(key)) fillSingles(next);
    if (key === "program_service_id") {
      const service=data.program_services?.find(row=>row.id===v);
      if (service && resource === "sessions") next.kind=service.session_kind;
    }
    if (resource === "claims" && key === "session_id" && v !== prev.session_id) Object.assign(next,claimFormDefaults(data.sessions.find(row=>row.id===v)));
    if (scheduling && ["facility_id", "episode_id", "client_id"].includes(key)) {
      const newZone = schedulingZone(data, { ...initial, ...next }, resource);
      if (zone !== newZone) {
        const selected: Record<string, string> = {};
        for (const field of ["starts_at", "ends_at"]) {
          const wall = String(prev[field] || "");
          const candidates = wallTimeCandidates(wall, zone);
          const instant = candidates.length === 1 ? candidates[0] : occurrences[field];
          if (candidates.length > 1 && !candidates.includes(instant)) {
            setError("Choose which of the two repeated times to use before changing the facility.");
            return;
          }
          if (!instant || !candidates.includes(instant)) continue;
          next[field] = facilityWallTime(instant, newZone);
          selected[field] = instant;
        }
        setOccurrences(selected);
      }
    } else if (key === "starts_at" || key === "ends_at") {
      setOccurrences(old => ({ ...old, [key]: "" }));
    }
    if (scheduling && key === "ends_at") endFollows.current = false;
    if (scheduling && key === "starts_at" && completeWall(v)) {
      // Moving the start keeps the visit's length. A visit with no end yet gets the service's usual length.
      const held = wallMinutesBetween(String(prev.starts_at ?? ""), String(prev.ends_at ?? ""));
      const usual = Number(serviceOf(next)?.default_minutes) || 60;
      if (completeWall(prev.ends_at) ? held > 0 : true) {
        next.ends_at = shiftWallTime(String(v), completeWall(prev.ends_at) && held > 0 ? held : usual);
        setOccurrences(old => ({ ...old, ends_at: "" }));
      }
    }
    if (scheduling && creating && endFollows.current && next.program_service_id && next.program_service_id !== prev.program_service_id && completeWall(next.starts_at)) {
      const usual = Number(serviceOf(next)?.default_minutes);
      if (usual > 0) {
        next.ends_at = shiftWallTime(String(next.starts_at), usual);
        setOccurrences(old => ({ ...old, ends_at: "" }));
      }
    }
    setValues(next);
    // Anything that just changed, by hand or by filling itself in, no longer shows its old error.
    const changed = Object.keys(next).filter((name) => next[name] !== prev[name]);
    if (changed.some((name) => name in fieldErrors))
      setFieldErrors((old) => Object.fromEntries(Object.entries(old).filter(([name]) => !changed.includes(name))));
  };
  async function submit(e: FormEvent) {
    e.preventDefault();
    if (noShowClaim || busy) return;
    const found: Record<string, string> = {};
    let first: HTMLElement | null = null;
    for (const control of formRef.current?.querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>("input, select, textarea") ?? []) {
      if (control.disabled || control.validity.valid) continue;
      const key = control.closest<HTMLElement>("[data-field]")?.dataset.field;
      if (!key) continue;
      first ??= control;
      if (!(key in found)) found[key] = fieldMessage(fields.find((field) => field.key === key), control);
    }
    const count = Object.keys(found).length;
    if (count) {
      setFieldErrors(found);
      const names = Object.keys(found).map((key) => lowerFirst(fields.find((field) => field.key === key)?.label ?? key));
      setError(`${numberWord(count)} ${count === 1 ? "thing" : "things"} to fix before this can be saved: ${names.join(", ")}.`);
      first?.focus();
      return;
    }
    if (submitRef.current) submitRef.current.style.minWidth = submitRef.current.offsetWidth + "px";
    setBusy(true);
    setError("");
    setFieldErrors({});
    try {
      const body: Record<string, unknown> = {};
      for (const f of fields) {
        let v = values[f.key];
        if (f.type === "fields") {
          const fs = v as FormField[];
          if (!fs.length)
            throw new Error("Add at least one question to this form.");
          if (new Set(fs.map((x) => x.key)).size !== fs.length)
            throw new Error("Two questions have the same wording. Change one of them.");
          v = fs.map((field) =>
            field.type === "select"
              ? {
                  ...field,
                  options: [
                    ...new Set(
                      (field.options || [])
                        .map((o) => o.trim())
                        .filter(Boolean),
                    ),
                  ],
                }
              : field,
          );
          if (
            (v as FormField[]).some(
              (field) => field.type === "select" && !field.options?.length,
            )
          )
            throw new Error(
              "Add at least one choice to each choose-one question.",
            );
        }
        body[f.key] = scheduling && f.type === "datetime-local" && v
          ? resolveFacilityTime(String(v), zone, occurrences[f.key], typeof initial[f.key] === "string" ? initial[f.key] as string : undefined) : normalizeFieldValue(f, v);
      }
      if (scheduling && body.starts_at && body.ends_at && Date.parse(String(body.ends_at)) <= Date.parse(String(body.starts_at))) {
        setFieldErrors({ ends_at: "The end must be after the start." });
        throw new Error("This session ends before it starts. Check the end time.");
      }
      if (scheduling && values.program_service_id) {
        const durationError=serviceDurationError(serviceOf(values),String(body.starts_at),String(body.ends_at));
        if (durationError) throw Error(durationError);
      }
      await onSubmit(body);
    } catch (err) {
      setError(
        err instanceof Error && err.message
          ? err.message
          : "That did not save. Your entries are still here. Try again.",
      );
      requestAnimationFrame(() => alertRef.current?.scrollIntoView({ block: "nearest" }));
    } finally {
      setBusy(false);
    }
  }
  const locations = (() => {
    const counts = new Map<string, number>();
    for (const row of [...data.sessions, ...data.groups]) {
      const place = value(row, "location").trim();
      if (place) counts.set(place, (counts.get(place) ?? 0) + 1);
    }
    const common = [...counts.entries()].sort((a, b) => b[1] - a[1]).slice(0, 3).map(([place]) => place);
    return common.some((place) => /telehealth/i.test(place)) ? common : [...common, "Telehealth"];
  })();
  const length = scheduling ? durationText(String(values.starts_at ?? ""), String(values.ends_at ?? "")) : "";
  return (
    <form onSubmit={submit} ref={formRef} noValidate>
      {/* A confirmation with nothing to fill in has no body: the title and description say it all. */}
      <div className="form-body" hidden={!groups.length && !error && !scheduling && !claimSession && !(resource && initial.client_id)}>
        {!creating && resource && Boolean(initial.client_id) && !shown.some((field) => field.key === "client_id") && (
          <p className="form-note form-note--who"><NxIcon name="clients" size={16} /><span>For <strong>{clientName(data.clients.find((row) => row.id === initial.client_id))}</strong>. The client on a saved record cannot be changed.</span></p>
        )}
        {scheduling && <p className="form-note"><NxIcon name="clock" size={16} /><span>Times are {zone === "UTC" ? "in UTC until a facility is chosen" : <><strong>{zoneLabel(zone)}</strong>, from the {resource === "sessions" ? "client's care episode or facility" : "facility"}</>}.</span></p>}
        {selectedService && <div className="form-note"><NxIcon name="info" size={16} /><span><strong>{value(selectedService,"name")}</strong> runs {value(selectedService,"min_minutes")} to {value(selectedService,"max_minutes")} min. Usual length {value(selectedService,"default_minutes")} min.</span> <Button variant="text small" disabled={busy || !values.starts_at} onClick={()=>{try{const start=resolveFacilityTime(String(values.starts_at),zone,occurrences.starts_at,typeof initial.starts_at==="string"?initial.starts_at:undefined);const end=new Date(Date.parse(start)+Number(selectedService.default_minutes)*60000).toISOString();setValues(prev=>({...prev,ends_at:facilityWallTime(end,zone)}));setOccurrences(prev=>({...prev,ends_at:end}));setError("");}catch(error){setError(error instanceof Error?error.message:"Check the start time, then set the length.");}}}>Use {value(selectedService,"default_minutes")} min</Button></div>}
        {claimSession && Boolean(initial.id) && <p className="form-note"><NxIcon name="encounter" size={16} /><span>Session attendance: <strong>{label(value(claimSession, "status"))}</strong></span></p>}
        {noShowClaim ? <p className="form-note is-warning"><NxIcon name="warning" size={16} /><span>This session was a no-show. A service claim cannot be saved for it.</span></p>
          : claimSession && serviceSnapshot(claimSession) && <p className="form-note"><NxIcon name="info" size={16} /><span>The service code and charge come from the session's saved service. Check the amount, units, diagnosis and payer rules before saving.</span></p>}
        {groups.map((group, index) => <fieldset className="record-section" key={group.title} disabled={busy}>
          {group.title && <legend>{numbered && <span className="nx-step" aria-hidden="true">{groups.slice(0, index + 1).filter((g) => g.title).length}</span>}{group.title}{group.hint && <small>{group.hint}</small>}</legend>}
          <div className="form-grid">
          {group.fields.map((f) => {
            const choices = f.type === "select" ? choicesFor(f, values) : [];
            const id = `field-${f.key}`;
            const problem = fieldErrors[f.key];
            const describedBy = [f.hint ? `${id}-hint` : "", problem ? `${id}-error` : ""].filter(Boolean).join(" ") || undefined;
            const waitsForClient = Boolean(f.ref && followsClient.includes(f.ref) && hasClientField && !values.client_id);
            const noneOnFile = Boolean(f.ref && followsClient.includes(f.ref) && hasClientField && values.client_id && !choices.length);
            const suggestions = f.suggest === "locations" ? locations : f.suggest;
            return (
              <div
                className={`form-field ${["textarea", "fields"].includes(f.type || "") ? "full" : ""} ${f.type === "checkbox" ? "checkbox-field" : ""}`}
                key={f.key}
                data-field={f.key}
                data-invalid={problem !== undefined ? "" : undefined}
              >
                {f.type === "checkbox" ? (
                  <label htmlFor={id}>
                    <input
                      id={id}
                      type="checkbox"
                      aria-describedby={describedBy}
                      checked={Boolean(values[f.key])}
                      onChange={(e) => set(f.key, e.target.checked)}
                    />
                    {f.label}
                  </label>
                ) : (
                  <>
                    <label htmlFor={id}>
                      {f.label}
                      {f.required && <span className="required" aria-hidden="true"> *</span>}
                    </label>
                    {f.type === "fields" ? (
                      <FieldBuilder
                        fields={(values[f.key] || []) as FormField[]}
                        onChange={(v) => set(f.key, v)}
                      />
                    ) : f.type === "textarea" ? (
                      <textarea
                        id={id}
                        rows={f.key === "body" ? 9 : 3}
                        aria-describedby={describedBy}
                        aria-invalid={problem ? true : undefined}
                        placeholder={f.placeholder}
                        value={String(values[f.key] ?? "")}
                        onChange={(e) => set(f.key, e.target.value)}
                        required={f.required}
                      />
                    ) : f.type === "select" ? (
                      <Choice
                        id={id}
                        aria-describedby={describedBy}
                        aria-invalid={problem ? true : undefined}
                        value={String(values[f.key] ?? "")}
                        onChange={(e) => set(f.key, e.target.value)}
                        required={f.required}
                        disabled={waitsForClient || (resource === "sessions" && f.key === "kind" && !!values.program_service_id)}
                      >
                        <option value="">
                          {waitsForClient ? "Choose a client first" : noneOnFile ? `No ${lowerFirst(f.label)} on file` : f.required ? "Choose…" : emptyChoice[f.key] ?? "None"}
                        </option>
                        {f.options?.map((o) => (
                          <option key={o} value={o}>
                            {f.optionLabels?.[o] ?? label(o)}
                          </option>
                        ))}
                        {f.optionLabels && Boolean(values[f.key]) && !f.options?.includes(String(values[f.key])) && <option value={String(values[f.key])}>{String(values[f.key])}</option>}
                        {choices.map((row) => (
                          <option value={row.id} key={row.id} disabled={resource === "claims" && f.ref === "sessions" && row.status === "no_show"}>
                            {f.catalog ? `${value(row,"service_code")} · ${value(row,"name")} · ${value(data.care_programs?.find(p=>p.id===row.program_id),"name")}${row._historical ? " · saved with this session" : ""}` : optionName(f.ref!, row, data)}
                            {resource === "claims" && f.ref === "sessions" && ` · ${label(value(row, "status"))}`}
                          </option>
                        ))}
                      </Choice>
                    ) : f.type === "datetime-local" ? (
                      <DateTimeField id={id} label={f.label} required={f.required} value={String(values[f.key] ?? "")} onChange={next => set(f.key, next)} />
                    ) : f.type === "money" ? (
                      <span className="nx-affix">
                        <span aria-hidden="true">$</span>
                        <input
                          id={id}
                          type="number"
                          inputMode="decimal"
                          aria-describedby={describedBy}
                          aria-invalid={problem ? true : undefined}
                          placeholder="0.00"
                          value={String(values[f.key] ?? "")}
                          onChange={(e) => set(f.key, e.target.value)}
                          required={f.required}
                          min={f.min}
                          max={f.max}
                          step="0.01"
                        />
                      </span>
                    ) : (
                      <FieldInput
                        id={id}
                        type={f.type || "text"}
                        aria-describedby={describedBy}
                        aria-invalid={problem ? true : undefined}
                        placeholder={f.placeholder}
                        value={String(values[f.key] ?? "")}
                        onChange={(e) => set(f.key, e.target.value)}
                        required={f.required}
                        min={f.min}
                        max={
                          resource === "clients" && f.key === "birth_date" &&
                          (!initial.id || values.birth_date !== initial.birth_date)
                            ? latestEligibleBirthDate(
                                facilityLocalToday(
                                  data.facilities.find(
                                    (row) => row.id === values.facility_id,
                                  )?.timezone as string | undefined,
                                ),
                              ) || f.max
                            : f.max
                        }
                        step={f.type === "number" ? "1" : undefined}
                        maxLength={
                          ["text", "email", "tel"].includes(f.type || "text")
                            ? 500
                            : undefined
                        }
                      />
                    )}
                  </>
                )}
                {suggestions && suggestions.length > 0 && (
                  <div className="nx-suggest" role="group" aria-label={`Common answers for ${lowerFirst(f.label)}`}>
                    {suggestions.map((item) => (
                      <button type="button" key={item} className="nx-chip" aria-pressed={values[f.key] === item} onClick={() => set(f.key, item)}>{item}</button>
                    ))}
                  </div>
                )}
                {problem && <span className="nx-field-error" id={`${id}-error`} role="alert"><NxIcon name="error" size={14} />{problem}</span>}
                {f.key === "ends_at" && length && <small className="nx-length"><NxIcon name="clock" size={13} />{length}</small>}
                {noneOnFile && <small>{f.ref === "episodes" ? "Start an intake from the client's chart to open a care episode." : f.ref === "policies" ? "Add the client's insurance policy first." : f.ref === "sessions" ? "Schedule a session for this client first." : "Nothing recorded for this client yet."}</small>}
                {f.hint && !noneOnFile && <small id={`${id}-hint`}>{f.hint}</small>}
                {scheduling && f.type === "datetime-local" && wallTimeCandidates(String(values[f.key] || ""), zone).length > 1 && (
                  <label className="scheduling-occurrence">The clocks go back, so this time happens twice. Which one?
                    <Choice value={occurrences[f.key] || ""} required onChange={event => setOccurrences(prev => ({ ...prev, [f.key]: event.target.value }))}>
                      <option value="">Choose first or second…</option>
                      {wallTimeCandidates(String(values[f.key]), zone).map((instant, i) => <option key={instant} value={instant}>{occurrenceLabel(instant, zone, i)}</option>)}
                    </Choice>
                  </label>
                )}
              </div>
            );
          })}
          </div>
        </fieldset>)}
        {error && (
          <div className="error-banner" role="alert" ref={alertRef}>
            <NxIcon name="error" size={17} />
            <span>{error}</span>
          </div>
        )}
      </div>
      <div className="modal-foot">
        <span className="form-assurance">
          <Icon name="shield" size={15} /> Changes go in the audit trail.
        </span>
        <Button variant="secondary" onClick={onCancel} disabled={busy}>
          Cancel
        </Button>
        <button type="submit" ref={submitRef} className="button" data-busy={busy ? "" : undefined} disabled={busy || noShowClaim}>
          {busy ? <><NxIcon name="sun" size={16} className="nx-spin" />Saving</> : <>{action}<Icon name="arrow" size={16} /></>}
        </button>
      </div>
    </form>
  );
}
