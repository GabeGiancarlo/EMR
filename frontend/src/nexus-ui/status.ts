import type { NexusIconName } from "../brand/icons";

export type Tone = "neutral" | "ok" | "warn" | "bad" | "info" | "live";

/** Status is a word, an icon and a colour, in that order. This maps a record status to the last two. */
const STATUS: Record<string, { tone: Tone; icon: NexusIconName }> = {
  // settled, good
  active: { tone: "ok", icon: "success" }, completed: { tone: "ok", icon: "success" }, done: { tone: "ok", icon: "success" },
  signed: { tone: "ok", icon: "success" }, paid: { tone: "ok", icon: "success" }, posted: { tone: "ok", icon: "success" },
  ready: { tone: "ok", icon: "success" }, achieved: { tone: "ok", icon: "success" }, attended: { tone: "ok", icon: "success" },
  on_track: { tone: "ok", icon: "success" }, normal: { tone: "neutral", icon: "check" },
  // waiting on someone
  draft: { tone: "warn", icon: "edit" }, in_progress: { tone: "warn", icon: "clock" }, partially_paid: { tone: "warn", icon: "payment" },
  at_risk: { tone: "warn", icon: "warning" }, high: { tone: "warn", icon: "warning" }, intake: { tone: "warn", icon: "consent" },
  inquiry: { tone: "info", icon: "info" }, submitted: { tone: "info", icon: "arrow-right" }, open: { tone: "info", icon: "info" },
  planned: { tone: "info", icon: "clock" }, scheduled: { tone: "neutral", icon: "clock" },
  // needs attention
  urgent: { tone: "bad", icon: "error" }, denied: { tone: "bad", icon: "error" }, expired: { tone: "bad", icon: "error" },
  no_show: { tone: "bad", icon: "error" }, revoked: { tone: "bad", icon: "error" }, not_connected: { tone: "bad", icon: "error" },
  // closed without being an error
  cancelled: { tone: "neutral", icon: "close" }, discharged: { tone: "neutral", icon: "history" }, discontinued: { tone: "neutral", icon: "history" },
  excused: { tone: "neutral", icon: "check" }, inactive: { tone: "neutral", icon: "clock" }, reversed: { tone: "neutral", icon: "history" },
  voided: { tone: "neutral", icon: "history" }, superseded: { tone: "neutral", icon: "history" }, clinician: { tone: "neutral", icon: "clients" },
};

export function statusStyle(status: string): { tone: Tone; icon: NexusIconName } {
  return STATUS[status] ?? { tone: "neutral", icon: "info" };
}
