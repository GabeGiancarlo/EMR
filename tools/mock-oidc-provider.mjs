#!/usr/bin/env node
/**
 * A stand-in identity provider for trying hosted mode on this machine.
 *
 * It signs an identity token for whatever email you type, so it proves nothing about
 * anyone. It exists to exercise Nexus's sign-in code without an identity-provider
 * account. It listens on loopback only, redirects only to loopback pages, and makes a
 * new signing key every time it starts. Never point a hosted Nexus at it.
 *
 *   node tools/mock-oidc-provider.mjs [--port 9400] [--client-id nexus-local-demo] [--token-seconds 3600]
 *                                     [--refresh-without-id-token]
 *
 * When the sign-in request asks for the offline_access scope it also issues a rotating
 * refresh token. --refresh-without-id-token makes the refresh answer omit the identity
 * token, which OpenID Connect allows and some providers do.
 *
 * Then start the API with NEXUS_AUTH_MODE=oidc, NEXUS_OIDC_ISSUER=http://127.0.0.1:9400,
 * NEXUS_OIDC_AUDIENCE and NEXUS_OIDC_CLIENT_ID set to the client id.
 */
import { createHash, createSign, generateKeyPairSync, randomBytes } from "node:crypto";
import { createServer } from "node:http";

const option = (name, fallback) => {
  const index = process.argv.indexOf(`--${name}`);
  return index > 0 && process.argv[index + 1] ? process.argv[index + 1] : fallback;
};
const port = Number(option("port", "9400"));
const clientId = option("client-id", "nexus-local-demo");
const tokenSeconds = Number(option("token-seconds", "3600"));
const refreshOmitsIdToken = process.argv.includes("--refresh-without-id-token");
const issuer = `http://127.0.0.1:${port}`;

const { publicKey, privateKey } = generateKeyPairSync("rsa", { modulusLength: 2048 });
const kid = randomBytes(8).toString("hex");
const jwk = { ...publicKey.export({ format: "jwk" }), kid, use: "sig", alg: "RS256" };
const encode = (value) => Buffer.from(typeof value === "string" ? value : JSON.stringify(value)).toString("base64url");
function sign(claims) {
  const body = `${encode({ alg: "RS256", typ: "JWT", kid })}.${encode(claims)}`;
  return `${body}.${createSign("RSA-SHA256").update(body).sign(privateKey).toString("base64url")}`;
}

/** One-time codes waiting to be exchanged: code -> what was asked for. */
const codes = new Map();
/** Refresh tokens in force: token -> who it was issued to. Each is single-use. */
const refreshTokens = new Map();
function issueRefreshToken(grant) {
  const token = randomBytes(24).toString("base64url");
  refreshTokens.set(token, { email: grant.email, verified: grant.verified });
  return token;
}
function idToken(grant, nonce) {
  const now = Math.floor(Date.now() / 1000);
  return sign({
    iss: issuer, aud: clientId, sub: `mock|${grant.email}`, iat: now, exp: now + tokenSeconds, ...(nonce ? { nonce } : {}),
    email: grant.email, email_verified: grant.verified, name: grant.email.split("@")[0],
  });
}
const loopback = (value) => {
  try {
    const url = new URL(value);
    return url.protocol === "http:" && ["127.0.0.1", "localhost", "[::1]"].includes(url.hostname);
  } catch {
    return false;
  }
};
const escapeHtml = (text) => String(text).replace(/[&<>"']/g, (char) => `&#${char.charCodeAt(0)};`);
const cookie = (req) => Object.fromEntries((req.headers.cookie || "").split(";").map((part) => part.trim().split("=")).filter((pair) => pair.length === 2));
function send(res, status, body, headers = {}) {
  const json = typeof body !== "string";
  res.writeHead(status, { "Content-Type": json ? "application/json" : "text/html; charset=utf-8", "Cache-Control": "no-store", ...headers });
  res.end(json ? JSON.stringify(body) : body);
}
function redirect(res, target, params, headers = {}) {
  const url = new URL(target);
  for (const [key, value] of Object.entries(params)) url.searchParams.set(key, value);
  res.writeHead(302, { Location: url.toString(), "Cache-Control": "no-store", ...headers });
  res.end();
}
function issueCode(res, request, email, verified) {
  const code = randomBytes(24).toString("base64url");
  codes.set(code, { ...request, email, verified, expires: Date.now() + 60_000 });
  const session = encode({ email, verified });
  redirect(res, request.redirect_uri, { code, state: request.state }, { "Set-Cookie": `mock_session=${session}; Path=/; HttpOnly; SameSite=Lax` });
}
async function form(req) {
  let raw = "";
  for await (const chunk of req) {
    raw += chunk;
    if (raw.length > 16_384) throw new Error("too large");
  }
  return Object.fromEntries(new URLSearchParams(raw));
}

const server = createServer(async (req, res) => {
  const url = new URL(req.url, issuer);
  const cors = { "Access-Control-Allow-Origin": "*", "Access-Control-Allow-Headers": "content-type,accept" };
  try {
    if (req.method === "OPTIONS") return send(res, 204, "", cors);
    if (url.pathname === "/.well-known/openid-configuration")
      return send(res, 200, {
        issuer, authorization_endpoint: `${issuer}/authorize`, token_endpoint: `${issuer}/token`, jwks_uri: `${issuer}/jwks`,
        end_session_endpoint: `${issuer}/logout`, response_types_supported: ["code"], subject_types_supported: ["public"],
        id_token_signing_alg_values_supported: ["RS256"], code_challenge_methods_supported: ["S256"],
      }, cors);
    if (url.pathname === "/jwks") return send(res, 200, { keys: [jwk] }, cors);

    if (url.pathname === "/authorize") {
      const request = req.method === "POST" ? await form(req) : Object.fromEntries(url.searchParams);
      if (request.client_id !== clientId || !loopback(request.redirect_uri) || request.response_type !== "code" ||
          request.code_challenge_method !== "S256" || !request.code_challenge || !request.state)
        return send(res, 400, "<h1>Mock provider</h1><p>This request is not one the mock provider accepts (client id, loopback redirect, code flow and an S256 challenge are required).</p>");
      if (req.method === "POST") return issueCode(res, request, String(request.email || "").trim(), request.verified === "yes");
      const saved = cookie(req).mock_session;
      if (request.prompt === "none") {
        if (!saved) return redirect(res, request.redirect_uri, { error: "login_required", state: request.state });
        const session = JSON.parse(Buffer.from(saved, "base64url").toString());
        return issueCode(res, request, session.email, session.verified);
      }
      const hidden = ["client_id", "redirect_uri", "response_type", "scope", "state", "nonce", "code_challenge", "code_challenge_method"]
        .map((name) => `<input type="hidden" name="${name}" value="${escapeHtml(request[name] ?? "")}">`).join("");
      return send(res, 200, `<!doctype html><meta charset="utf-8"><title>Mock sign-in</title>
<body style="font:16px system-ui;max-width:30rem;margin:4rem auto;padding:0 1rem">
<h1>Mock sign-in</h1>
<p>Local testing only. This page signs a token for any email you type.</p>
<form method="post" action="/authorize">${hidden}
<p><label>Email<br><input name="email" type="email" required autofocus style="width:100%;font:inherit;padding:.4rem"></label></p>
<p><label><input type="checkbox" name="verified" value="yes" checked> Email is verified</label></p>
<p><button style="font:inherit;padding:.5rem 1rem">Sign in</button></p></form></body>`);
    }

    if (url.pathname === "/token" && req.method === "POST") {
      const request = await form(req);
      if (request.grant_type === "refresh_token") {
        const holder = refreshTokens.get(request.refresh_token);
        refreshTokens.delete(request.refresh_token);
        if (!holder || request.client_id !== clientId) return send(res, 400, { error: "invalid_grant" }, cors);
        return send(res, 200, {
          token_type: "Bearer", expires_in: tokenSeconds, access_token: randomBytes(16).toString("hex"),
          refresh_token: issueRefreshToken(holder), ...(refreshOmitsIdToken ? {} : { id_token: idToken(holder, null) }),
        }, cors);
      }
      const grant = codes.get(request.code);
      codes.delete(request.code);
      const challenge = createHash("sha256").update(String(request.code_verifier || "")).digest("base64url");
      if (request.grant_type !== "authorization_code" || !grant || grant.expires < Date.now() || request.client_id !== clientId ||
          request.redirect_uri !== grant.redirect_uri || challenge !== grant.code_challenge)
        return send(res, 400, { error: "invalid_grant" }, cors);
      const offline = String(grant.scope || "").split(/\s+/).includes("offline_access");
      return send(res, 200, {
        token_type: "Bearer", expires_in: tokenSeconds, id_token: idToken(grant, grant.nonce), access_token: randomBytes(16).toString("hex"),
        ...(offline ? { refresh_token: issueRefreshToken(grant) } : {}),
      }, cors);
    }

    if (url.pathname === "/logout") {
      const clear = { "Set-Cookie": "mock_session=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax" };
      const back = url.searchParams.get("post_logout_redirect_uri");
      return back && loopback(back) ? redirect(res, back, {}, clear) : send(res, 200, "<h1>Signed out of the mock provider.</h1>", clear);
    }
    send(res, 404, { error: "not_found" });
  } catch {
    send(res, 400, { error: "invalid_request" });
  }
});
server.listen(port, "127.0.0.1", () => {
  console.log(`Mock identity provider (local testing only): ${issuer}  client id: ${clientId}  token lifetime: ${tokenSeconds}s${refreshOmitsIdToken ? "  refresh answers carry no identity token" : ""}`);
});
