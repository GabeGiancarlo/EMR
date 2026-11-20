import { initials } from "../api";
import type { Row } from "../api";
import portraits from "../brand/demo-portraits.jpg";

const demoRoster = ["alex", "jordan", "taylor"];

/** Fictional portraits belong only to the seed-demo roster. All other charts get a local monogram. */
export function Avatar({
  name,
  id,
  patient,
}: {
  name: string;
  id: string;
  patient?: Row;
}) {
  const tone =
    Array.from(id).reduce((sum, c) => (sum * 31 + c.charCodeAt(0)) >>> 0, 0) %
    6;
  const slot = demoRoster.findIndex(
    (first) =>
      patient?.email === first + ".synthetic@example.invalid" &&
      patient.last_name === "Synthetic" &&
      patient.emergency_contact ===
        "Synthetic demonstration record; no real contact.",
  );
  return (
    <span
      aria-hidden="true"
      className={`person-avatar tone-${tone}`}
      style={
        slot >= 0
          ? {
              backgroundImage: `url(${portraits})`,
              backgroundSize: "300% 100%",
              backgroundPosition: `${slot * 50}% center`,
            }
          : undefined
      }
    >
      {slot < 0 && <b>{initials(name)}</b>}
    </span>
  );
}
