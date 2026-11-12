import assert from "node:assert/strict";
import test from "node:test";
import {
  authorizeUrl, base64url, codeChallenge, codeExchange, idTokenProblem, judgeCallback, logoutUrl,
  parseAuthConfig, randomToken, readCallback, readClaims, readTokenResponse, refreshExchange,
  type OidcConfig, type PendingSignIn, type Session,
} from "./oidc.ts";

const config: OidcConfig = {
  mode: "oidc", issuer: "https://tenant.example.com/", client_id: "client-123", scopes: "openid email profile",
  authorization_endpoint: "https://tenant.example.com/authorize", token_endpoint: "https://tenant.example.com/oauth/token",
  end_session_endpoint: "https://tenant.example.com/oidc/logout",
};
const pending: PendingSignIn = { state: "state-1", verifier: "verifier-1", nonce: "nonce-1", silent: false, startedAt: 1_000 };
const token = (claims: Record<string, unknown>) =>
  `${base64url(new TextEncoder().encode('{"alg":"RS256"}'))}.${base64url(new TextEncoder().encode(JSON.stringify(claims)))}.signature`;

test("PKCE challenge matches the RFC 7636 example", async () => {
  assert.equal(await codeChallenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
});

test("verifiers are long, URL-safe and never repeat", () => {
  const seen = new Set(Array.from({ length: 50 }, () => randomToken()));
  assert.equal(seen.size, 50);
  for (const value of seen) assert.match(value, /^[A-Za-z0-9_-]{43}$/);
});

test("the sign-in request asks for a code with an S256 challenge and never a token", () => {
  const url = new URL(authorizeUrl(config, { redirectUri: "https://demo.example.com/", state: "s", nonce: "n", challenge: "c", silent: false }));
  assert.equal(url.origin + url.pathname, "https://tenant.example.com/authorize");
  assert.deepEqual(Object.fromEntries(url.searchParams), {
    response_type: "code", client_id: "client-123", redirect_uri: "https://demo.example.com/", scope: "openid email profile",
    state: "s", nonce: "n", code_challenge: "c", code_challenge_method: "S256",
  });
  const silent = new URL(authorizeUrl(config, { redirectUri: "https://demo.example.com/", state: "s", nonce: "n", challenge: "c", silent: true }));
  assert.equal(silent.searchParams.get("prompt"), "none");
});

test("only an answer to this page's own recent request is accepted", () => {
  assert.deepEqual(readCallback(""), { kind: "none" });
  assert.deepEqual(readCallback("?code=abc"), { kind: "none" }, "a code without state is not a callback");
  assert.deepEqual(readCallback("?view=billing&state=x"), { kind: "none" });
  const answer = readCallback("?code=abc&state=state-1");
  assert.deepEqual(judgeCallback(answer, pending, 2_000), { ok: true, code: "abc" });
  assert.deepEqual(judgeCallback(answer, null, 2_000), { ok: false, reason: "mismatch" }, "no request was pending");
  assert.deepEqual(judgeCallback(readCallback("?code=abc&state=forged"), pending, 2_000), { ok: false, reason: "mismatch" });
  assert.deepEqual(judgeCallback(answer, pending, 1_000 + 11 * 60 * 1000), { ok: false, reason: "stale" });
});

test("a refused silent attempt is 'not signed in'; any other refusal is reported", () => {
  const refused = readCallback("?error=login_required&state=state-1");
  assert.deepEqual(judgeCallback(refused, { ...pending, silent: true }, 2_000), { ok: false, reason: "not-signed-in" });
  assert.deepEqual(judgeCallback(refused, pending, 2_000), { ok: false, reason: "provider", detail: "login_required" });
  assert.deepEqual(judgeCallback(readCallback("?error=access_denied&state=state-1"), { ...pending, silent: true }, 2_000),
    { ok: false, reason: "provider", detail: "access_denied" });
});

test("the code is exchanged with its verifier and no client secret", () => {
  assert.deepEqual(Object.fromEntries(codeExchange(config, { code: "abc", verifier: "v", redirectUri: "https://demo.example.com/" })), {
    grant_type: "authorization_code", client_id: "client-123", code: "abc", code_verifier: "v", redirect_uri: "https://demo.example.com/",
  });
  assert.deepEqual(Object.fromEntries(refreshExchange(config, "r")), { grant_type: "refresh_token", client_id: "client-123", refresh_token: "r" });
});

test("an identity token must be for this provider, this application, this attempt and still valid", () => {
  const good = { iss: config.issuer, aud: "client-123", nonce: "nonce-1", exp: 5_000, email: "clinician@example.invalid" };
  const expected = { issuer: config.issuer, clientId: "client-123", nonce: "nonce-1", now: 1_000_000 };
  assert.equal(idTokenProblem(readClaims(token(good)), expected), null);
  assert.equal(idTokenProblem(readClaims(token({ ...good, aud: ["other", "client-123"] })), expected), null);
  assert.equal(idTokenProblem(readClaims(token({ ...good, nonce: undefined })), { ...expected, nonce: null }), null, "refreshed tokens carry no nonce");
  assert.match(idTokenProblem(readClaims(token({ ...good, iss: "https://evil.example.com/" })), expected)!, /unexpected provider/);
  assert.match(idTokenProblem(readClaims(token({ ...good, aud: "other" })), expected)!, /different application/);
  assert.match(idTokenProblem(readClaims(token({ ...good, nonce: "replayed" })), expected)!, /this sign-in attempt/);
  assert.match(idTokenProblem(readClaims(token({ ...good, exp: 999 })), expected)!, /expired/);
  assert.match(idTokenProblem(readClaims(token({ ...good, exp: "5000" })), expected)!, /expired/);
  for (const broken of [undefined, "", "a.b", "a.%%%.c", `x.${base64url(new TextEncoder().encode("[1]"))}.y`])
    assert.match(idTokenProblem(readClaims(broken), expected)!, /did not include/);
});

test("claims with non-ASCII text decode correctly", () => {
  assert.equal(readClaims(token({ name: "Zoë Müller" }))?.name, "Zoë Müller");
});

test("the API's sign-in description is rejected unless complete and HTTPS", () => {
  assert.deepEqual(parseAuthConfig({ mode: "local" }), { mode: "local" });
  assert.deepEqual(parseAuthConfig({ ...config, extra: 1 }), config);
  assert.equal(parseAuthConfig({ ...config, end_session_endpoint: undefined })?.mode, "oidc");
  assert.equal((parseAuthConfig({ ...config, end_session_endpoint: "http://tenant.example.com/logout" }) as OidcConfig).end_session_endpoint, null);
  assert.equal(parseAuthConfig({ ...config, authorization_endpoint: "http://tenant.example.com/authorize" }), null);
  assert.equal(parseAuthConfig({ ...config, token_endpoint: "javascript:alert(1)" }), null);
  assert.equal(parseAuthConfig({ ...config, client_id: "" }), null);
  assert.equal(parseAuthConfig({ mode: "something-else" }), null);
  assert.equal(parseAuthConfig(null), null);
  assert.equal(parseAuthConfig("oidc"), null);
  assert.equal(parseAuthConfig({ ...config, authorization_endpoint: "http://127.0.0.1:9000/authorize", token_endpoint: "http://localhost:9000/token" })?.mode, "oidc");
});

test("sign-out ends the provider session when the provider offers it", () => {
  const url = new URL(logoutUrl(config, { idToken: "id-token", returnTo: "https://demo.example.com/" })!);
  assert.deepEqual(Object.fromEntries(url.searchParams), { client_id: "client-123", post_logout_redirect_uri: "https://demo.example.com/", id_token_hint: "id-token" });
  assert.equal(logoutUrl({ ...config, end_session_endpoint: null }, { idToken: "id-token", returnTo: "https://demo.example.com/" }), null);
});

const signInExpectation = { issuer: config.issuer, clientId: "client-123", nonce: "nonce-1", now: 1_000_000 };
const refreshExpectation = { ...signInExpectation, nonce: null };
const claims = { iss: config.issuer, aud: "client-123", nonce: "nonce-1", exp: 5_000, email: "clinician@example.invalid" };
const current: Session = { idToken: token(claims), refreshToken: "refresh-1", expiresAt: 5_000_000, email: "clinician@example.invalid", renewable: true };

test("a sign-in response becomes a session; without an identity token it is refused", () => {
  const outcome = readTokenResponse({ id_token: token(claims), refresh_token: "refresh-1", access_token: "ignored" }, signInExpectation, null);
  assert.deepEqual(outcome, { kind: "session", session: current });
  const noRefresh = readTokenResponse({ id_token: token(claims) }, signInExpectation, null);
  assert.equal(noRefresh.kind === "session" && noRefresh.session.renewable, false, "nothing to renew with");
  assert.equal(readTokenResponse({ access_token: "only" }, signInExpectation, null).kind, "invalid");
  assert.equal(readTokenResponse(null, signInExpectation, null).kind, "invalid");
  assert.equal(readTokenResponse({ id_token: token({ ...claims, nonce: "other" }) }, signInExpectation, null).kind, "invalid");
});

test("a refresh that returns a new identity token replaces the session", () => {
  const later = { ...claims, nonce: undefined, exp: 9_000 };
  const outcome = readTokenResponse({ id_token: token(later), refresh_token: "refresh-2" }, refreshExpectation, current);
  assert.deepEqual(outcome, { kind: "session", session: { ...current, idToken: token(later), refreshToken: "refresh-2", expiresAt: 9_000_000 } });
  const sameRefresh = readTokenResponse({ id_token: token(later) }, refreshExpectation, current);
  assert.equal(sameRefresh.kind === "session" && sameRefresh.session.refreshToken, "refresh-1", "the refresh token is kept when not rotated");
});

test("a refresh without an identity token keeps the session to its expiry and stops asking", () => {
  for (const response of [{ access_token: "new", refresh_token: "refresh-2" }, { access_token: "new", id_token: null }]) {
    const outcome = readTokenResponse(response, refreshExpectation, current);
    assert.equal(outcome.kind, "unchanged");
    if (outcome.kind !== "unchanged") return;
    assert.equal(outcome.session.idToken, current.idToken);
    assert.equal(outcome.session.expiresAt, current.expiresAt, "the session is not cut short");
    assert.equal(outcome.session.renewable, false, "the provider is not asked again");
  }
  const rotated = readTokenResponse({ refresh_token: "refresh-2" }, refreshExpectation, current);
  assert.equal(rotated.kind === "unchanged" && rotated.session.refreshToken, "refresh-2");
});

test("a refresh is refused if its identity token is wrong or for someone else", () => {
  const later = { ...claims, nonce: undefined, exp: 9_000 };
  assert.equal(readTokenResponse({ id_token: token({ ...later, email: "someone-else@example.invalid" }) }, refreshExpectation, current).kind, "invalid");
  assert.equal(readTokenResponse({ id_token: token({ ...later, aud: "other" }) }, refreshExpectation, current).kind, "invalid");
  assert.equal(readTokenResponse({ id_token: token({ ...later, exp: 1 }) }, refreshExpectation, current).kind, "invalid");
  assert.equal(readTokenResponse({ id_token: "garbage" }, refreshExpectation, current).kind, "invalid");
});
