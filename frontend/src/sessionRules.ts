/**
 * Small rules about a signed-in session's lifetime. Pure, so they can be tested without a browser.
 */

/** How long before a session that cannot be renewed ends the person is told to save. */
export const EXPIRY_WARNING_MS = 5 * 60_000;

export type SessionExpiry = {
  /** When the identity token stops being accepted, in epoch milliseconds. */
  expiresAt: number;
  /** Whether a newer identity token can still be fetched without the person signing in. */
  renewable: boolean;
};

/**
 * The moment to name in the "your session ends at…" warning, or null when none is due.
 *
 * A renewable session is never warned about: it is refreshed quietly before it ends.
 * A session that cannot be renewed is warned about in its last five minutes. Once it has
 * ended there is nothing to warn about; the sign-in screen takes over.
 */
export function expiryWarning(expiry: SessionExpiry | null, now: number): number | null {
  if (!expiry || expiry.renewable) return null;
  const left = expiry.expiresAt - now;
  return left > 0 && left <= EXPIRY_WARNING_MS ? expiry.expiresAt : null;
}

/** A time of day as the person's own clock shows it, for example "3:45 PM" or "15:45". */
export function clockTime(at: number, locale?: string, timeZone?: string): string {
  return new Intl.DateTimeFormat(locale, { hour: "numeric", minute: "2-digit", timeZone }).format(new Date(at));
}

/**
 * True when the browser brought this page back from its back/forward cache while it was
 * still waiting to leave for the identity provider. Nothing will ever finish that wait,
 * so the page has to start again.
 */
export function restoredMidRedirect(persisted: boolean, status: string): boolean {
  return persisted && status === "redirecting";
}
