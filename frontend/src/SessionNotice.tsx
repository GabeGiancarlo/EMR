import { useEffect, useReducer, useState, useSyncExternalStore } from "react";
import { authSnapshot, sessionExpiry, signIn, subscribeAuth } from "./authSession";
import { NxButton, NxIcon } from "./nexus-ui";
import { clockTime, expiryWarning } from "./sessionRules.ts";

const CHECK_EVERY_MS = 15_000;

/**
 * A reminder shown in the last minutes of a signed-in session that cannot be renewed, so
 * the person can save before the sign-in screen replaces the workspace. It never blocks
 * the page. `inline` is the form used inside a dialog, where the page-level reminder
 * sits behind the dialog's backdrop and cannot be reached.
 *
 * Renders nothing in local mode, when signed out, or while the session can still be
 * renewed quietly.
 */
export default function SessionNotice({ inline = false }: { inline?: boolean }) {
  useSyncExternalStore(subscribeAuth, authSnapshot);
  const [, recheck] = useReducer((count: number) => count + 1, 0);
  const [dismissed, setDismissed] = useState(0);
  const expiry = sessionExpiry();
  const signedIn = expiry !== null;
  useEffect(() => {
    if (!signedIn) return;
    const timer = setInterval(recheck, CHECK_EVERY_MS);
    return () => clearInterval(timer);
  }, [signedIn]);
  const endsAt = expiryWarning(expiry, Date.now());
  if (endsAt === null || dismissed === endsAt) return null;
  return (
    <div className={`nx-session-notice${inline ? " nx-session-notice--inline" : ""}`} role="status">
      <NxIcon name="clock" size={18} />
      <span>
        Your session ends at <strong>{clockTime(endsAt)}</strong>. Save your work now. Signing in again reloads this page.
      </span>
      <NxButton size="sm" variant="secondary" icon="lock" onClick={signIn}>
        Sign in again
      </NxButton>
      {!inline && (
        <button type="button" className="nx-session-dismiss" aria-label="Dismiss session reminder" onClick={() => setDismissed(endsAt)}>
          <NxIcon name="close" size={16} />
        </button>
      )}
    </div>
  );
}
