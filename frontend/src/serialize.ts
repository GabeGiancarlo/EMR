type InputField = {
  key: string;
  label: string;
  type?: string;
  required?: boolean;
  ref?: string;
  catalog?: string;
};

/** Keep empty textual columns distinct from nullable relationships and dates. */
export function normalizeFieldValue(
  field: InputField,
  input: unknown,
): unknown {
  let value = typeof input === "string" ? input.trim() : input;
  if (field.type === "money" || field.type === "number") {
    if (value === "" || value == null) value = null;
    else {
      const number = Number(value);
      if (!Number.isFinite(number))
        throw new Error(`${field.label} must be a valid number.`);
      value = field.type === "money" ? Math.round(number * 100) : number;
    }
  } else if (field.type === "datetime-local") {
    value = value ? new Date(String(value)).toISOString() : null;
  } else if (field.type === "date" || field.ref || field.catalog) {
    if (value === "" || value == null) value = null;
  } else if (
    ["text", "textarea", "email", "tel"].includes(field.type || "text")
  ) {
    value = value == null ? "" : String(value);
  }
  if (field.required && (value === null || value === undefined || value === ""))
    throw new Error(`${field.label} is required.`);
  return value;
}
