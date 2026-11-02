import { useState } from "react";
import type { FormEvent } from "react";
import { clientName, date, label, value } from "./api";
import type { Bootstrap, FormField, Resource, Row } from "./api";
import { Button, Icon } from "./ui";
import { normalizeFieldValue } from "./serialize";
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
  ref?: Resource;
  hint?: string;
  min?: number;
  max?: number;
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
): Field => f(key, title, { type: "select", ref, required });
const options = (
  key: string,
  title: string,
  values: string[],
  required = true,
): Field => f(key, title, { type: "select", options: values, required });
export const definitions: Record<
  Resource,
  { title: string; description: string; fields: Field[] }
> = {
  clients: {
    title: "Client",
    description:
      "Start a connected care record. Add an episode and coverage after saving.",
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
      "Connect a client, level of care, and facility for this course of treatment.",
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
    description: "Schedule care with a clinician and link it to an episode.",
    fields: [
      reference("client_id", "Client", "clients"),
      reference("episode_id", "Care episode", "episodes"),
      reference("clinician_id", "Clinician", "staff"),
      options("kind", "Session type", [
        "individual",
        "assessment",
        "family",
        "case_management",
      ]),
      f("starts_at", "Start", { type: "datetime-local", required: true }),
      f("ends_at", "End", { type: "datetime-local", required: true }),
      f("location", "Room or location", { required: true }),
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
      "Write and review your documentation. Signing preserves the original record.",
    fields: [
      reference("client_id", "Client", "clients"),
      reference("session_id", "Session", "sessions", false),
      reference("template_id", "Form template", "templates", false),
      f("title", "Note title", { required: true }),
      f("body", "Clinical narrative", {
        type: "textarea",
        required: true,
        hint: "Review the complete narrative before signing. Do not include information beyond the purpose of this record.",
      }),
    ],
  },
  tasks: {
    title: "Task",
    description:
      "Keep follow-through visible across clinical and operational teams.",
    fields: [
      f("title", "Task title", { required: true }),
      reference("client_id", "Client", "clients", false),
      reference("assignee_id", "Assigned to", "staff", false),
      f("due_date", "Due date", { type: "date" }),
      options("priority", "Priority", ["normal", "low", "high", "urgent"]),
      options("status", "Status", ["open", "in_progress", "done"]),
      f("description", "Details", { type: "textarea" }),
    ],
  },
  policies: {
    title: "Insurance policy",
    description:
      "Record coverage used to prepare claims. This does not verify payer eligibility.",
    fields: [
      reference("client_id", "Client", "clients"),
      f("payer_name", "Payer", { required: true }),
      f("member_id", "Member ID", { required: true }),
      f("group_number", "Group number"),
      f("effective_date", "Effective date", { type: "date", required: true }),
      f("termination_date", "Termination date", { type: "date" }),
      options("status", "Status", ["active", "inactive"]),
    ],
  },
  authorizations: {
    title: "Service authorization",
    description:
      "Track the payer approval period and units before a claim is prepared.",
    fields: [
      reference("client_id", "Client", "clients"),
      reference("policy_id", "Policy", "policies"),
      f("authorization_number", "Authorization number", { required: true }),
      f("service_code", "Service code", { required: true }),
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
      "Link completed care and coverage. Readiness checks run before you record external submission.",
    fields: [
      reference("client_id", "Client", "clients"),
      reference("policy_id", "Policy", "policies"),
      reference("session_id", "Session", "sessions"),
      reference("authorization_id", "Authorization", "authorizations", false),
      f("service_code", "Service code", { required: true }),
      f("diagnosis_code", "Diagnosis code", { required: true }),
      f("amount_cents", "Charge amount (USD)", {
        type: "money",
        required: true,
        min: 0.01,
      }),
      f("units", "Units of service", {
        type: "number",
        required: true,
        min: 1,
        default: 1,
      }),
    ],
  },
  payments: {
    title: "Payment",
    description:
      "Append a payment to the claim ledger. Existing financial records remain in history.",
    fields: [
      reference("claim_id", "Claim", "claims"),
      f("amount_cents", "Amount received (USD)", {
        type: "money",
        required: true,
        min: 0.01,
      }),
      f("received_on", "Received on", { type: "date", required: true }),
      f("reference", "Payment reference", { required: true }),
      options("source", "Source", ["payer", "client", "adjustment"]),
    ],
  },
  staff: {
    title: "Team member",
    description:
      "Define responsibilities and local workspace access. Identity-provider sign-in will be connected separately.",
    fields: [
      f("name", "Full name", { required: true }),
      f("email", "Work email", { type: "email", required: true }),
      options("role", "Workspace role", [
        "clinician",
        "clinical_lead",
        "billing",
        "executive",
        "support",
        "admin",
      ]),
      reference("facility_id", "Facility", "facilities"),
      f("active", "Active team member", { type: "checkbox", default: true }),
    ],
  },
  facilities: {
    title: "Facility",
    description: "Organize care and operations by location.",
    fields: [
      f("name", "Facility name", { required: true }),
      f("timezone", "IANA timezone", {
        required: true,
        default: "America/New_York",
      }),
      f("active", "Active facility", { type: "checkbox", default: true }),
    ],
  },
  templates: {
    title: "Form template",
    description:
      "Create your organization’s own form structure. Your clinical lead should approve content before use.",
    fields: [
      f("name", "Template name", { required: true }),
      options("kind", "Purpose", [
        "intake",
        "progress",
        "assessment",
        "discharge",
        "consent",
      ]),
      f("description", "When to use this form", { type: "textarea" }),
      f("applies_to_stage", "Applies to client stage", {
        type: "select",
        options: ["any", "intake", "active", "discharged"],
        required: true,
        default: "any",
        hint: "Any matches every client stage. Set this rule according to your organization's workflow.",
      }),
      f("applies_to_care_level", "Applies to care level", {
        type: "select",
        options: ["any", "outpatient", "iop", "php"],
        required: true,
        default: "any",
        hint: "Residential and detox episodes match only Any.",
      }),
      f("required_for_stage", "Required when this rule matches", {
        type: "checkbox",
        default: false,
        hint: "Leave unchecked to suggest the form. This does not create a legal or clinical requirement.",
      }),
      f("fields", "Form fields", { type: "fields", required: true }),
      f("active", "Available for documentation", {
        type: "checkbox",
        default: true,
      }),
    ],
  },
  consents: {
    title: "Consent record",
    description:
      "Record the scope and dates of a consent attestation. Keep the original executed document in your approved records system.",
    fields: [
      reference("client_id", "Client", "clients"),
      f("kind", "Consent type", {
        required: true,
        hint: "For example, treatment consent or a specific release of information.",
      }),
      f("granted_on", "Granted on", { type: "date", required: true }),
      f("expires_on", "Expires on", { type: "date" }),
      f("scope", "Permitted scope", { type: "textarea", required: true }),
    ],
  },
  exceptions: {
    title: "Access exception",
    description:
      "Grant temporary resource access with a reason and expiry. Administrative privileges cannot be granted here.",
    fields: [
      reference("staff_id", "Team member", "staff"),
      options("resource", "Resource", [
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
      options("access_mode", "Access level", ["read", "write"]),
      f("reason", "Reason for exception", { type: "textarea", required: true }),
      f("expires_at", "Expires at", { type: "datetime-local", required: true }),
      f("revoked", "Revoked", { type: "checkbox", default: false }),
    ],
  },
  treatment_plans: {
    title: "Treatment plan",
    description:
      "Define the client’s priorities and strengths. Add measurable goals before signing the plan.",
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
      "Create a measurable goal while the treatment plan is in draft.",
    fields: [
      reference("plan_id", "Treatment plan", "treatment_plans"),
      f("description", "Goal", { type: "textarea", required: true }),
      f("measure", "How progress will be measured", { required: true }),
      f("baseline", "Starting point", { required: true }),
      f("target", "Target outcome", { required: true }),
      f("target_date", "Target date", { type: "date" }),
    ],
  },
  plan_reviews: {
    title: "Plan review",
    description:
      "Record progress against the signed plan without changing its original goals.",
    fields: [
      reference("plan_id", "Treatment plan", "treatment_plans"),
      f("summary", "Review summary", { type: "textarea", required: true }),
      f("next_review_date", "Next review", { type: "date" }),
    ],
  },
  form_responses: {
    title: "Form response",
    description:
      "Complete a preserved form definition and review before signing.",
    fields: [],
  },
  groups: {
    title: "Group encounter",
    description:
      "Schedule one shared encounter, then add participants and resolve attendance individually.",
    fields: [
      f("title", "Group title", { required: true }),
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
      f("capacity", "Participant capacity", {
        type: "number",
        required: true,
        min: 1,
        max: 100,
        default: 12,
      }),
      f("location", "Room or location", { required: true }),
    ],
  },
  group_attendance: {
    title: "Group participant",
    description:
      "Enroll a client in a shared group. Nexus creates a linked individual session for their notes and billing.",
    fields: [
      reference("group_id", "Group encounter", "groups"),
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

function optionName(resource: Resource, row: Row, data: Bootstrap) {
  const client = data.clients.find((c) => c.id === value(row, "client_id"));
  if (resource === "clients") return clientName(row);
  if (resource === "episodes")
    return `${clientName(client)} · ${label(value(row, "level_of_care"))} · ${value(row, "start_date")}`;
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
      {fields.map((field, i) => (
        <div className="field-builder-row" key={i}>
          <span className="field-number">{i + 1}</span>
          <input
            aria-label={`Field ${i + 1} label`}
            value={field.label}
            required
            placeholder="Field label"
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
          <select
            aria-label={`Field ${i + 1} type`}
            value={field.type}
            onChange={(e) => update(i, { type: e.target.value })}
          >
            <option value="textarea">Long answer</option>
            <option value="text">Short answer</option>
            <option value="date">Date</option>
            <option value="number">Number</option>
            <option value="checkbox">Checkbox</option>
            <option value="select">Selection</option>
          </select>
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
            aria-label={`Remove field ${i + 1}`}
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
        <Icon name="plus" size={16} /> Add field
      </Button>
    </div>
  );
}
export function RecordForm({
  resource,
  data,
  initial = {},
  onSubmit,
  onCancel,
  fields: customFields,
  submitLabel = "Save record",
}: {
  resource?: Resource;
  data: Bootstrap;
  initial?: Record<string, unknown>;
  onSubmit: (body: Record<string, unknown>) => Promise<void>;
  onCancel: () => void;
  fields?: Field[];
  submitLabel?: string;
}) {
  const fields = customFields || (resource ? definitions[resource].fields : []);
  const [values, setValues] = useState<Record<string, unknown>>(() =>
    Object.fromEntries(
      fields.map((f) => {
        let v =
          initial[f.key] ??
          f.default ??
          (f.type === "fields" ? [] : f.type === "checkbox" ? false : "");
        if (f.type === "money" && v !== "") v = Number(v) / 100;
        if (f.type === "datetime-local" && v) {
          const d = new Date(String(v));
          if (!Number.isNaN(d.getTime())) {
            const offset = d.getTimezoneOffset() * 60000;
            v = new Date(d.getTime() - offset).toISOString().slice(0, 16);
          }
        }
        if (f.type === "select" && f.required && v === "")
          v = f.options?.[0] ?? "";
        return [f.key, v];
      }),
    ),
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const set = (key: string, v: unknown) =>
    setValues((prev) => ({ ...prev, [key]: v }));
  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      const body: Record<string, unknown> = {};
      for (const f of fields) {
        let v = values[f.key];
        if (f.type === "fields") {
          const fs = v as FormField[];
          if (!fs.length)
            throw new Error("Add at least one field to this template.");
          if (new Set(fs.map((x) => x.key)).size !== fs.length)
            throw new Error("Use a different label for each field.");
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
              "Add at least one choice for each selection field.",
            );
        }
        body[f.key] = normalizeFieldValue(f, v);
      }
      await onSubmit(body);
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : "Unable to save. Please try again.",
      );
    } finally {
      setBusy(false);
    }
  }
  return (
    <form onSubmit={submit}>
      <div className="form-body">
        <div className="form-grid">
          {fields.map((f) => {
            let choices = f.ref ? data[f.ref] || [] : [];
            if (
              f.ref &&
              ["episodes", "sessions", "policies", "authorizations"].includes(
                f.ref,
              ) &&
              values.client_id
            )
              choices = choices.filter(
                (r) => value(r, "client_id") === values.client_id,
              );
            if (f.ref === "staff")
              choices = choices.filter(
                (r) =>
                  r.active !== false &&
                  (!["clinician_id", "primary_clinician_id"].includes(f.key) ||
                    ["clinician", "clinical_lead", "admin"].includes(
                      value(r, "role"),
                    )),
              );
            if (f.ref === "facilities")
              choices = choices.filter((r) => r.active !== false);
            if (f.ref === "templates")
              choices = choices.filter((r) => r.active !== false);
            const id = `field-${f.key}`;
            return (
              <div
                className={`form-field ${["textarea", "fields"].includes(f.type || "") ? "full" : ""} ${f.type === "checkbox" ? "checkbox-field" : ""}`}
                key={f.key}
              >
                {f.type === "checkbox" ? (
                  <label htmlFor={id}>
                    <input
                      id={id}
                      type="checkbox"
                      checked={Boolean(values[f.key])}
                      onChange={(e) => set(f.key, e.target.checked)}
                    />
                    {f.label}
                  </label>
                ) : (
                  <>
                    <label htmlFor={id}>
                      {f.label}
                      {f.required && <span className="required"> *</span>}
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
                        value={String(values[f.key] ?? "")}
                        onChange={(e) => set(f.key, e.target.value)}
                        required={f.required}
                      />
                    ) : f.type === "select" ? (
                      <select
                        id={id}
                        value={String(values[f.key] ?? "")}
                        onChange={(e) => set(f.key, e.target.value)}
                        required={f.required}
                      >
                        <option value="">
                          {f.required ? "Choose…" : "Not linked"}
                        </option>
                        {f.options?.map((o) => (
                          <option key={o} value={o}>
                            {label(o)}
                          </option>
                        ))}
                        {choices.map((row) => (
                          <option value={row.id} key={row.id}>
                            {optionName(f.ref!, row, data)}
                          </option>
                        ))}
                      </select>
                    ) : (
                      <input
                        id={id}
                        type={f.type === "money" ? "number" : f.type || "text"}
                        aria-describedby={f.hint ? `${id}-hint` : undefined}
                        value={String(values[f.key] ?? "")}
                        onChange={(e) => set(f.key, e.target.value)}
                        onInput={
                          f.type === "date" || f.type === "datetime-local"
                            ? (e) => set(f.key, e.currentTarget.value)
                            : undefined
                        }
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
                        step={
                          f.type === "money"
                            ? "0.01"
                            : f.type === "number"
                              ? "1"
                              : undefined
                        }
                        maxLength={
                          ["text", "email", "tel"].includes(f.type || "text")
                            ? 500
                            : undefined
                        }
                      />
                    )}
                  </>
                )}
                {f.hint && <small id={`${id}-hint`}>{f.hint}</small>}
              </div>
            );
          })}
        </div>
        {error && (
          <div className="error-banner" role="alert">
            {error}
          </div>
        )}
      </div>
      <div className="modal-foot">
        <span className="form-assurance">
          <Icon name="shield" size={15} /> Changes are recorded in the audit
          trail
        </span>
        <Button variant="secondary" onClick={onCancel} disabled={busy}>
          Cancel
        </Button>
        <Button type="submit" disabled={busy}>
          {busy ? "Saving…" : submitLabel}
          <Icon name="arrow" size={16} />
        </Button>
      </div>
    </form>
  );
}
