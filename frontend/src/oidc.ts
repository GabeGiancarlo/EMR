/**
 * OpenID Connect authorization code flow with PKCE, as plain functions.
 *
 * Nothing here touches the page, storage or the network, so every rule is unit-tested.
 * The browser never decides who someone is: the API verifies the token's signature and
 * maps its email to a staff member. These checks only stop the page from sending a
 * token it was not meant to receive.
 */
export type OidcConfig = {
  mode: "oidc";
  issuer: string;
  client_id: string;
  scopes: string;
  authorization_endpoint: string;
  token_endpoint: string;
  end_session_endpoint: string | null;
};
export type AuthConfig = { mode: "local" } | OidcConfig;

/** What must survive the round trip to the provider. Not a credential on its own. */
export type PendingSignIn = {
  state: string;
  verifier: string;
  nonce: string;
  /** A background attempt that must not show the provider's login page. */
  silent: boolean;
  startedAt: number;
};

export type Callback =
  | { kind: "none" }
  | { kind: "code"; code: string; state: string }
  | { kind: "error"; error: string; state: string };

export type CallbackVerdict =
  | { ok: true; code: string }
  | { ok: false; reason: "mismatch" | "stale" | "not-signed-in" | "provider"; detail?: string };

const PENDING_LIFETIME_MS = 10 * 60 * 1000;
/** Provider answers to a silent attempt that simply mean "show the sign-in button". */
const NEEDS_INTERACTION = new Set([
  "login_required",
  "interaction_required",
  "consent_required",
  "account_selection_required",
]);

export function base64url(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** 32 random bytes is a 43-character verifier, the minimum RFC 7636 allows. */
export function randomToken(bytes = 32): string {
  return base64url(crypto.getRandomValues(new Uint8Array(bytes)));
}

export async function codeChallenge(verifier: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(verifier));
  return base64url(new Uint8Array(digest));
}

function secureUrl(value: unknown): string | null {
  if (typeof value !== "string") return null;
  try {
    const url = new URL(value);
    const loopback = ["127.0.0.1", "localhost", "[::1]"].includes(url.hostname);
    return url.protocol === "https:" || (url.protocol === "http:" && loopback) ? value : null;
  } catch {
    return null;
  }
}

/** Accept the API's sign-in description only if it is complete and its endpoints are HTTPS. */
export function parseAuthConfig(value: unknown): AuthConfig | null {
  if (typeof value !== "object" || value === null) return null;
  const raw = value as Record<string, unknown>;
  if (raw.mode === "local") return { mode: "local" };
  if (raw.mode !== "oidc") return null;
  const authorization = secureUrl(raw.authorization_endpoint);
  const token = secureUrl(raw.token_endpoint);
  if (!authorization || !token) return null;
  if (typeof raw.issuer !== "string" || typeof raw.client_id !== "string" || !raw.client_id) return null;
  return {
    mode: "oidc",
    issuer: raw.issuer,
    client_id: raw.client_id,
    scopes: typeof raw.scopes === "string" && raw.scopes ? raw.scopes : "openid email profile",
    authorization_endpoint: authorization,
    token_endpoint: token,
    end_session_endpoint: secureUrl(raw.end_session_endpoint),
  };
}

export function authorizeUrl(
  config: OidcConfig,
  request: { redirectUri: string; state: string; nonce: string; challenge: string; silent: boolean },
): string {
  const url = new URL(config.authorization_endpoint);
  url.searchParams.set("response_type", "code");
  url.searchParams.set("client_id", config.client_id);
  url.searchParams.set("redirect_uri", request.redirectUri);
  url.searchParams.set("scope", config.scopes);
  url.searchParams.set("state", request.state);
  url.searchParams.set("nonce", request.nonce);
  url.searchParams.set("code_challenge", request.challenge);
  url.searchParams.set("code_challenge_method", "S256");
  if (request.silent) url.searchParams.set("prompt", "none");
  return url.toString();
}

/** Read the provider's answer from the page's query string. */
export function readCallback(search: string): Callback {
  const params = new URLSearchParams(search);
  const state = params.get("state");
  if (!state) return { kind: "none" };
  const error = params.get("error");
  if (error) return { kind: "error", error, state };
  const code = params.get("code");
  return code ? { kind: "code", code, state } : { kind: "none" };
}

/** An answer counts only if this page asked the question, recently. */
export function judgeCallback(callback: Callback, pending: PendingSignIn | null, now: number): CallbackVerdict {
  if (callback.kind === "none" || !pending || callback.state !== pending.state) return { ok: false, reason: "mismatch" };
  if (now - pending.startedAt > PENDING_LIFETIME_MS) return { ok: false, reason: "stale" };
  if (callback.kind === "error") {
    return pending.silent && NEEDS_INTERACTION.has(callback.error)
      ? { ok: false, reason: "not-signed-in" }
      : { ok: false, reason: "provider", detail: callback.error };
  }
  return { ok: true, code: callback.code };
}

export function codeExchange(config: OidcConfig, request: { code: string; verifier: string; redirectUri: string }): URLSearchParams {
  return new URLSearchParams({
    grant_type: "authorization_code",
    client_id: config.client_id,
    code: request.code,
    code_verifier: request.verifier,
    redirect_uri: request.redirectUri,
  });
}

export function refreshExchange(config: OidcConfig, refreshToken: string): URLSearchParams {
  return new URLSearchParams({
    grant_type: "refresh_token",
    client_id: config.client_id,
    refresh_token: refreshToken,
  });
}

/** Decode a token's claims. This reads them; it does not prove them. */
export function readClaims(token: unknown): Record<string, unknown> | null {
  if (typeof token !== "string") return null;
  const parts = token.split(".");
  if (parts.length !== 3) return null;
  try {
    const padded = parts[1].replace(/-/g, "+").replace(/_/g, "/").padEnd(Math.ceil(parts[1].length / 4) * 4, "=");
    const bytes = Uint8Array.from(atob(padded), (char) => char.charCodeAt(0));
    const claims: unknown = JSON.parse(new TextDecoder().decode(bytes));
    return typeof claims === "object" && claims !== null && !Array.isArray(claims) ? (claims as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

/**
 * Why an ID token should not be used, or null if it can be. `nonce` is the value sent
 * with the sign-in request; pass null for a refreshed token, which carries none.
 */
export function idTokenProblem(
  claims: Record<string, unknown> | null,
  expected: { issuer: string; clientId: string; nonce: string | null; now: number },
): string | null {
  if (!claims) return "The sign-in response did not include an identity token.";
  if (claims.iss !== expected.issuer) return "The identity token came from an unexpected provider.";
  const audience = Array.isArray(claims.aud) ? claims.aud : [claims.aud];
  if (!audience.includes(expected.clientId)) return "The identity token was issued for a different application.";
  if (expected.nonce !== null && claims.nonce !== expected.nonce) return "The identity token does not belong to this sign-in attempt.";
  if (typeof claims.exp !== "number" || claims.exp * 1000 <= expected.now) return "The identity token has already expired.";
  return null;
}

/** A signed-in person, as held in memory. */
export type Session = {
  idToken: string;
  refreshToken: string | null;
  /** When the identity token stops being accepted, in epoch milliseconds. */
  expiresAt: number;
  email: string;
  /** Whether asking the provider again can produce a newer identity token. */
  renewable: boolean;
};

export type TokenOutcome =
  /** A new identity token: after sign-in, or after a refresh that returned one. */
  | { kind: "session"; session: Session }
  /** A refresh the provider accepted without sending a new identity token. */
  | { kind: "unchanged"; session: Session }
  | { kind: "invalid"; problem: string };

/**
 * Turn the provider's token response into a session.
 *
 * `previous` is null after the sign-in redirect and the current session on a refresh.
 * OpenID Connect lets a provider answer a refresh without a new identity token. The API
 * accepts only identity tokens, so in that case the session carries on with the token
 * it has until that token expires, and is marked as not renewable so the provider is
 * not asked again. It is not treated as an error and does not sign the person out early.
 */
export function readTokenResponse(
  data: unknown,
  expected: { issuer: string; clientId: string; nonce: string | null; now: number },
  previous: Session | null,
): TokenOutcome {
  if (typeof data !== "object" || data === null) return { kind: "invalid", problem: "The identity provider did not complete sign-in. Try again." };
  const tokens = data as Record<string, unknown>;
  const refreshToken = typeof tokens.refresh_token === "string" && tokens.refresh_token ? tokens.refresh_token : (previous?.refreshToken ?? null);
  if (previous && (tokens.id_token === undefined || tokens.id_token === null))
    return { kind: "unchanged", session: { ...previous, refreshToken, renewable: false } };
  const claims = readClaims(tokens.id_token);
  const problem = idTokenProblem(claims, expected);
  if (problem || !claims) return { kind: "invalid", problem: problem ?? "Sign-in could not be completed." };
  const email = typeof claims.email === "string" ? claims.email : "";
  if (previous && email !== previous.email) return { kind: "invalid", problem: "The identity provider returned a token for a different account." };
  return {
    kind: "session",
    session: { idToken: tokens.id_token as string, refreshToken, expiresAt: (claims.exp as number) * 1000, email, renewable: refreshToken !== null },
  };
}

/** Where to send the browser to end the provider's session too, when it offers that. */
export function logoutUrl(config: OidcConfig, request: { idToken: string | null; returnTo: string }): string | null {
  if (!config.end_session_endpoint) return null;
  const url = new URL(config.end_session_endpoint);
  url.searchParams.set("client_id", config.client_id);
  url.searchParams.set("post_logout_redirect_uri", request.returnTo);
  if (request.idToken) url.searchParams.set("id_token_hint", request.idToken);
  return url.toString();
}
