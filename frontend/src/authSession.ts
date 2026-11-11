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
  authorizeUrl, codeChallenge, codeExchange, idTokenProblem, judgeCallback, logoutUrl, parseAuthConfig,
  randomToken, readCallback, readClaims, refreshExchange,
  type AuthConfig, type OidcConfig, type PendingSignIn,
} from "./oidc";

export const actorStorageKey = "nexus.local-operator";
const pendingKey = "nexus.oidc.pending";
const resumeKey = "nexus.oidc.resume";
const REFRESH_EARLY_MS = 60_000;

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

type Session = { idToken: string; refreshToken: string | null; expiresAt: number; email: string };

let config: AuthConfig | null = null;
let session: Session | null = null;
let snapshot: AuthSnapshot = { status: "loading", problem: "", email: "" };
let starting: Promise<void> | null = null;
let refreshing: Promise<boolean> | null = null;
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

function watchExpiry(): void {
  clearTimeout(expiryTimer);
  if (!session) return;
  const wait = session.expiresAt - Date.now() - (session.refreshToken ? REFRESH_EARLY_MS : 0);
  expiryTimer = setTimeout(() => void recoverSession(), Math.max(wait, 0));
}

async function exchange(provider: OidcConfig, body: URLSearchParams, nonce: string | null, previousRefresh: string | null): Promise<Session> {
  const response = await fetch(provider.token_endpoint, {
    method: "POST",
    headers: { Accept: "application/json", "Content-Type": "application/x-www-form-urlencoded" },
    body,
    cache: "no-store",
    signal: AbortSignal.timeout(15_000),
  });
  const data: unknown = await response.json().catch(() => null);
  if (!response.ok || typeof data !== "object" || data === null) throw new Error("The identity provider did not complete sign-in. Try again.");
  const tokens = data as Record<string, unknown>;
  const claims = readClaims(tokens.id_token);
  const problem = idTokenProblem(claims, { issuer: provider.issuer, clientId: provider.client_id, nonce, now: Date.now() });
  if (problem || !claims) throw new Error(problem ?? "Sign-in could not be completed.");
  return {
    idToken: tokens.id_token as string,
    refreshToken: typeof tokens.refresh_token === "string" ? tokens.refresh_token : previousRefresh,
    expiresAt: (claims.exp as number) * 1000,
    email: typeof claims.email === "string" ? claims.email : "",
  };
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
      try {
        session = await exchange(config, codeExchange(config, { code: verdict.code, verifier: pending.verifier, redirectUri: redirectUri() }), pending.nonce, null);
        storeSession(resumeKey, "1");
        watchExpiry();
        publish("signed-in");
      } catch (error) {
        publish("signed-out", error instanceof Error ? error.message : "Sign-in could not be completed.");
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

/** Find out how this workspace signs in, and finish a sign-in that is in flight. */
export function startAuth(): Promise<void> {
  return (starting ??= begin());
}

async function refresh(): Promise<boolean> {
  if (config?.mode !== "oidc" || !session?.refreshToken) return false;
  const provider = config;
  const current = session;
  refreshing ??= exchange(provider, refreshExchange(provider, current.refreshToken!), null, current.refreshToken)
    .then((next) => {
      session = next;
      watchExpiry();
      publish("signed-in");
      return true;
    })
    .catch(() => false)
    .finally(() => { refreshing = null; });
  return refreshing;
}

/**
 * Called when the API answers 401 or the token's lifetime ends. True means a new token
 * is in hand and the request can be repeated; false means the person must sign in again.
 */
export async function recoverSession(): Promise<boolean> {
  if (config?.mode !== "oidc") return false;
  if (await refresh()) return true;
  if (session) {
    session = null;
    clearTimeout(expiryTimer);
    storeSession(resumeKey, null);
    publish("expired");
  }
  return false;
}

/** Identity headers for an API request, in whichever mode the API is running. */
export async function authHeaders(): Promise<Record<string, string>> {
  if (!config) await startAuth();
  if (config?.mode === "oidc") {
    if (session?.refreshToken && session.expiresAt - Date.now() < REFRESH_EARLY_MS) await refresh();
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
