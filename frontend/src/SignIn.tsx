import { useEffect, useSyncExternalStore, type ReactNode } from "react";
import { authSnapshot, signIn, signOut, startAuth, subscribeAuth, type AuthSnapshot } from "./authSession";
import { LeafCard, NxBadge, NxButton, NxIcon } from "./nexus-ui";
import ThemeControl from "./ThemeControl";
import { Brand } from "./ui";
import "./signIn.css";

export function useAuth(): AuthSnapshot {
  return useSyncExternalStore(subscribeAuth, authSnapshot);
}

function EntryPage({ children }: { children: ReactNode }) {
  return (
    <div className="loading-page">
      <div className="entry-appearance">
        <ThemeControl />
      </div>
      <Brand />
      {children}
    </div>
  );
}

function SyntheticNotice() {
  return (
    <p className="sign-in-notice">
      <NxIcon name="shield" size={16} />
      <span>
        This is a demonstration workspace. Every record in it is synthetic. Do not enter information about real people.
      </span>
    </p>
  );
}

function SignInScreen({ expired, problem }: { expired: boolean; problem: string }) {
  return (
    <EntryPage>
      <LeafCard as="section" size="lg" className="sign-in-card" aria-labelledby="sign-in-heading">
        <NxBadge tone="info" icon="info">Demonstration workspace</NxBadge>
        <h1 id="sign-in-heading">{expired ? "Your session ended." : "Sign in to Nexus."}</h1>
        <p>
          {expired
            ? "For your security you were signed out. Sign in again to pick up where you left off; anything you had not saved will need to be entered again."
            : "You will be taken to your organization's sign-in page and brought straight back."}
        </p>
        {problem && (
          <div className="error-banner" role="alert">
            {problem}
          </div>
        )}
        <NxButton icon="lock" onClick={signIn}>
          {expired ? "Sign in again" : "Continue to sign in"}
        </NxButton>
        <SyntheticNotice />
      </LeafCard>
    </EntryPage>
  );
}

/** Signed in at the provider, but the email matches no active staff member here. */
export function NoAccessScreen({ email, onRetry }: { email: string; onRetry: () => void }) {
  return (
    <EntryPage>
      <LeafCard as="section" size="lg" className="sign-in-card" aria-labelledby="no-access-heading">
        <NxBadge tone="warn" icon="lock">No access</NxBadge>
        <h1 id="no-access-heading">This account is not set up here.</h1>
        <p>
          {email ? <>You are signed in as <strong>{email}</strong>, but that </> : "Your "}
          account has not been given access to this workspace. Ask the workspace administrator to add you, or sign in with a different account.
        </p>
        <div className="sign-in-actions">
          <NxButton icon="logout" onClick={signOut}>Sign out</NxButton>
          <NxButton variant="secondary" icon="refresh" onClick={onRetry}>Try again</NxButton>
        </div>
      </LeafCard>
    </EntryPage>
  );
}

/**
 * Decides whether the workspace may render. Local mode and an unreachable API pass
 * straight through to the workspace, which behaves as it always has.
 */
export default function AuthBoundary({ children }: { children: ReactNode }) {
  const auth = useAuth();
  useEffect(() => {
    void startAuth();
  }, []);
  if (auth.status === "loading" || auth.status === "redirecting")
    return (
      <EntryPage>
        <div className="spinner" />
        <p>{auth.status === "redirecting" ? "Opening sign-in…" : "Connecting your workspace…"}</p>
      </EntryPage>
    );
  if (auth.status === "signed-out" || auth.status === "expired")
    return <SignInScreen expired={auth.status === "expired"} problem={auth.problem} />;
  return <>{children}</>;
}
