import type { Bootstrap, Resource, Row } from "./api";
import type { Field } from "./forms";
import { claimServiceDefaults } from "./programRouting.ts";
import { facilityWallTime, schedulingZone } from "./schedulingTime.ts";

/** Database money stays in cents until it crosses into a form input. */
export function inputValue(field: Pick<Field, "type">, value: unknown): unknown {
  return field.type === "money" && value !== "" && value != null
    ? Number(value) / 100 : value;
}

export function claimFormDefaults(session: Row | undefined): Record<string, unknown> {
  const defaults = claimServiceDefaults(session);
  return { ...defaults, amount_cents: inputValue({ type: "money" }, defaults.amount_cents) };
}

/** Shared with RecordForm, so preselection and subsequent selection use one units contract. */
export function initialFormValues(
  fields: Field[], data: Bootstrap, initial: Record<string, unknown>, resource?: Resource,
): Record<string, unknown> {
  const scheduling = resource === "sessions" || resource === "groups";
  const defaults = resource === "claims" && !initial.id
    ? claimServiceDefaults(data.sessions.find(row => row.id === initial.session_id)) : {};
  return Object.fromEntries(fields.map(field => {
    let value = inputValue(field, initial[field.key] ?? defaults[field.key] ?? field.default
      ?? (field.type === "fields" ? [] : field.type === "checkbox" ? false : ""));
    if (field.type === "datetime-local" && value) {
      const date = new Date(String(value));
      if (!Number.isNaN(date.getTime())) {
        const offset = date.getTimezoneOffset() * 60000;
        value = scheduling ? facilityWallTime(date.toISOString(), schedulingZone(data, initial, resource))
          : new Date(date.getTime() - offset).toISOString().slice(0, 16);
      }
    }
    if (field.type === "select" && field.required && value === "") value = field.options?.[0] ?? "";
    return [field.key, value];
  }));
}
