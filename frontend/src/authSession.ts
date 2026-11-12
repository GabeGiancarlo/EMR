/**
 * Who the browser is, for both modes.
 *
 * Local mode: a selected development operator, kept in localStorage as before.
 * Hosted mode: an identity token from the provider's own login page, obtained with the
 * authorization code flow and PKCE. Tokens live only in this module's memory. They are
 * never written to localStorage or sessionStorage; sessionStorage holds just the
 * one-time values needed to finish a redirect and a flag that a silent attempt is worth trying.
 */
import { apiUrl } from "./apiBase";
import {
  authorizeUrl, codeChallenge, codeExchange, judgeCallback, logoutUrl, parseAuthConfig,
  randomToken, readCallback, readTokenResponse, refreshExchange,
  type AuthConfig, type OidcConfig, type PendingSignIn, type Session, type TokenOutcome,
} from "./oidc";
import { restoredMidRedirect, type SessionExpiry } from "./sessionRules.ts";

export const actorStorageKey = "nexus.local-operator";
const pendingKey = "nexus.oidc.pending";
const resumeKey = "nexus.oidc.resume";
const REFRESH_EARLY_MS = 60_000;
/** After a refresh the provider did not complete, how long before asking again. */
const REFRESH_RETRY_MS = 15_000;

export type AuthStatus =
  | "loading"
  /** The API could not be asked how to sign in. The workspace shows its reconnect screen. */
  | "unknown"
  | "local"
  | "signed-out"
  | "redirecting"
  | "signed-in"
  | "expired";
export type AuthSnapshot = { status: AuthStatus; problem: string; email: string };

let config: AuthConfig | null = null;
let session: Session | null = null;
let snapshot: AuthSnapshot = { status: "loading", problem: "", email: "" };
let starting: Promise<void> | null = null;
let renewing: Promise<Renewal> | null = null;
let renewalFailedAt = 0;
let expiryTimer: ReturnType<typeof setTimeout> | undefined;
const listeners = new Set<() => void>();

function publish(status: AuthStatus, problem = ""): void {
  snapshot = { status, problem, email: session?.email ?? "" };
  listeners.forEach((listener) => listener());
}
export function subscribeAuth(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
export const authSnapshot = (): AuthSnapshot => snapshot;
export const authMode = (): "local" | "oidc" | "unknown" => config?.mode ?? "unknown";
/**
 * Ask which mode the API runs in without starting or finishing a sign-in. For the patient
 * visit page, which has no staff session and must never be sent to the identity provider.
 */
export async function peekAuthMode(): Promise<"local" | "oidc" | "unknown"> {
  return (config ?? (await loadConfig()))?.mode ?? "unknown";
}
/**
 * When the signed-in session ends and whether it can still be renewed quietly. Null in
 * local mode or when signed out. A renewal the provider has just failed to complete counts
 * as not renewable until one succeeds, so the person is warned rather than surprised.
 */
export function sessionExpiry(): SessionExpiry | null {
  if (!session) return null;
  const stalled = renewalFailedAt > 0 && Date.now() - renewalFailedAt < REFRESH_EARLY_MS;
  return { expiresAt: session.expiresAt, renewable: session.renewable && session.refreshToken !== null && !stalled };
}

function stored(storage: "local" | "session", key: string): string | null {
  try {
    return (storage === "local" ? localStorage : sessionStorage).getItem(key);
  } catch {
    return null;
  }
}
function storeSession(key: string, value: string | null): void {
  try {
    if (value === null) sessionStorage.removeItem(key);
    else sessionStorage.setItem(key, value);
  } catch {
    // Storage can be unavailable in a private window; sign-in then needs the button each time.
  }
}
function takePending(): PendingSignIn | null {
  const raw = stored("session", pendingKey);
  storeSession(pendingKey, null);
  if (!raw) return null;
  try {
    const value = JSON.parse(raw) as Partial<PendingSignIn>;
    return typeof value.state === "string" && typeof value.verifier === "string" && typeof value.nonce === "string" && typeof value.startedAt === "number"
      ? { state: value.state, verifier: value.verifier, nonce: value.nonce, silent: value.silent === true, startedAt: value.startedAt }
      : null;
  } catch {
    return null;
  }
}
const redirectUri = (): string => `${window.location.origin}/`;

async function loadConfig(): Promise<AuthConfig | null> {
  try {
    const response = await fetch(apiUrl("/api/auth/config"), { headers: { Accept: "application/json" }, cache: "no-store" });
    // An API built before sign-in existed has no such route; it can only be local.
    if (response.status === 404 || response.status === 405) return { mode: "local" };
    return response.ok ? parseAuthConfig(await response.json()) : null;
  } catch {
    return null;
  }
}

/**
 * Wake up shortly before the identity token expires if a newer one can be fetched,
 * otherwise exactly when it expires.
 */
function watchExpiry(early = true): void {
  clearTimeout(expiryTimer);
  if (!session) return;
  const lead = early && session.renewable ? REFRESH_EARLY_MS : 0;
  expiryTimer = setTimeout(() => void onExpiryTimer(), Math.max(session.expiresAt - Date.now() - lead, 0));
}

async function exchange(provider: OidcConfig, body: URLSearchParams, nonce: string | null, previous: Session | null): Promise<TokenOutcome> {
  const response = await fetch(provider.token_endpoint, {
    method: "POST",
    headers: { Accept: "application/json", "Content-Type": "application/x-www-form-urlencoded" },
    body,
    cache: "no-store",
    signal: AbortSignal.timeout(15_000),
  });
  const data: unknown = await response.json().catch(() => null);
  if (!response.ok) return { kind: "invalid", problem: "The identity provider did not complete sign-in. Try again." };
  return readTokenResponse(data, { issuer: provider.issuer, clientId: provider.client_id, nonce, now: Date.now() }, previous);
}

async function redirectToProvider(provider: OidcConfig, silent: boolean): Promise<void> {
  const pending: PendingSignIn = { state: randomToken(), verifier: randomToken(), nonce: randomToken(), silent, startedAt: Date.now() };
  storeSession(pendingKey, JSON.stringify(pending));
  const challenge = await codeChallenge(pending.verifier);
  publish("redirecting");
  window.location.assign(authorizeUrl(provider, { redirectUri: redirectUri(), state: pending.state, nonce: pending.nonce, challenge, silent }));
}

async function begin(): Promise<void> {
  config = await loadConfig();
  if (!config) {
    starting = null; // Ask again on the next request or reconnect.
    publish("unknown");
    return;
  }
  if (config.mode === "local") {
    publish("local");
    return;
  }
  const callback = readCallback(window.location.search);
  const pending = takePending();
  if (callback.kind !== "none") {
    // The code is single-use; keep it out of the address bar and history.
    window.history.replaceState(null, "", window.location.pathname);
    const verdict = judgeCallback(callback, pending, Date.now());
    if (verdict.ok && pending) {
      const outcome = await exchange(config, codeExchange(config, { code: verdict.code, verifier: pending.verifier, redirectUri: redirectUri() }), pending.nonce, null)
        .catch((): TokenOutcome => ({ kind: "invalid", problem: "The identity provider could not be reached. Try again." }));
      if (outcome.kind === "session") {
        session = outcome.session;
        storeSession(resumeKey, "1");
        watchExpiry();
        publish("signed-in");
      } else {
        publish("signed-out", outcome.kind === "invalid" ? outcome.problem : "Sign-in could not be completed.");
      }
      return;
    }
    publish(
      "signed-out",
      verdict.ok || verdict.reason === "not-signed-in" ? ""
        : verdict.reason === "provider" ? "The identity provider did not complete sign-in. Try again."
        : "That sign-in attempt is no longer valid. Start again.",
    );
    return;
  }
  // After a reload the token is gone from memory. If this tab was signed in, ask the
  // provider once, without showing its login page, whether its session is still open.
  if (stored("session", resumeKey) === "1") {
    storeSession(resumeKey, null);
    await redirectToProvider(config, true);
    return;
  }
  publish("signed-out");
}

/**
 * Leaving for the identity provider publishes "redirecting". If the person then comes back
 * with the browser's Back button, the browser can restore this page exactly as it was left:
 * still saying "Opening sign-in…", with nothing left to wait for. Load the page afresh so
 * sign-in starts over. Nothing is lost: the workspace is not on screen while redirecting.
 */
let watchingReturn = false;
function watchReturn(): void {
  if (watchingReturn || typeof window === "undefined") return;
  watchingReturn = true;
  window.addEventListener("pageshow", (event) => {
    if (restoredMidRedirect(event.persisted, snapshot.status)) window.location.reload();
  });
}

/** Find out how this workspace signs in, and finish a sign-in that is in flight. */
export function startAuth(): Promise<void> {
  watchReturn();
  return (starting ??= begin());
}

/**
 * "renewed": a newer identity token is in hand. "kept": the provider accepted the
 * refresh but sent no identity token, so the current one stays until it expires and
 * the provider is not asked again. "failed": no answer or a refusal; the current token
 * is untouched.
 */
type Renewal = "renewed" | "kept" | "failed";

async function renew(): Promise<Renewal> {
  if (config?.mode !== "oidc" || !session?.renewable || !session.refreshToken) return "failed";
  const provider = config;
  const current = session;
  renewing ??= exchange(provider, refreshExchange(provider, current.refreshToken!), null, current)
    .then((outcome): Renewal => {
      // Signed out, or replaced, while the provider was answering.
      if (session !== current) return "failed";
      if (outcome.kind === "invalid") {
        renewalFailedAt = Date.now();
        return "failed";
      }
      session = outcome.session;
      if (outcome.kind === "unchanged") {
        watchExpiry();
        return "kept";
      }
      watchExpiry();
      publish("signed-in");
      return "renewed";
    })
    .catch((): Renewal => {
      renewalFailedAt = Date.now();
      return "failed";
    })
    .finally(() => { renewing = null; });
  return renewing;
}

function endSession(): void {
  session = null;
  clearTimeout(expiryTimer);
  storeSession(resumeKey, null);
  publish("expired");
}

async function onExpiryTimer(): Promise<void> {
  if (!session) return;
  if ((await renew()) === "renewed" || !session) return;
  // Not renewed: the current token is still good until its own expiry.
  if (session.expiresAt <= Date.now()) endSession();
  else watchExpiry(false);
}

/**
 * Called when the API answers 401. True means a newer token is in hand and the request
 * can be repeated; false means the person must sign in again.
 */
export async function recoverSession(): Promise<boolean> {
  if (config?.mode !== "oidc") return false;
  if ((await renew()) === "renewed") return true;
  if (session) endSession();
  return false;
}

/** Identity headers for an API request, in whichever mode the API is running. */
export async function authHeaders(): Promise<Record<string, string>> {
  if (!config) await startAuth();
  if (config?.mode === "oidc") {
    const closeToExpiry = session ? session.expiresAt - Date.now() < REFRESH_EARLY_MS : false;
    if (session?.renewable && closeToExpiry && Date.now() - renewalFailedAt > REFRESH_RETRY_MS) await renew();
    return session ? { Authorization: `Bearer ${session.idToken}` } : {};
  }
  const actor = stored("local", actorStorageKey);
  return actor ? { "x-nexus-actor": actor } : {};
}

/** Changes when a different person is using the page, so in-flight results can be dropped. */
export function identityKey(): string {
  return config?.mode === "oidc" ? (session?.email ?? "") : (stored("local", actorStorageKey) ?? "");
}

export function signIn(): void {
  if (config?.mode === "oidc") void redirectToProvider(config, false);
}

export function signOut(): void {
  if (config?.mode !== "oidc") return;
  const idToken = session?.idToken ?? null;
  session = null;
  clearTimeout(expiryTimer);
  storeSession(resumeKey, null);
  storeSession(pendingKey, null);
  const url = logoutUrl(config, { idToken, returnTo: redirectUri() });
  if (url) {
    publish("redirecting");
    window.location.assign(url);
  } else {
    publish("signed-out");
  }
}
