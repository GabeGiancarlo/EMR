#!/usr/bin/env node
/**
 * Stops a hosted build while frontend/vercel.json still holds placeholder addresses.
 *
 * vercel.json cannot read environment variables, so two addresses are written into it
 * by hand: the API host (twice, in the rewrites) and the identity provider's origin (in
 * the Content-Security-Policy). Left unset, the site would deploy, load, and then fail
 * every API call with no hint why. This check runs first in the Vercel build command and
 * fails the build with the exact lines to change. Outside Vercel it only prints.
 */
import { readFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";

const PLACEHOLDER = /REPLACE-WITH|\.invalid\b|\bexample\.(com|org|net)\b/i;

function httpsOrigin(value) {
  try {
    const url = new URL(value);
    return url.protocol === "https:" && !PLACEHOLDER.test(url.host) ? url.origin : null;
  } catch {
    return null;
  }
}

/** Every reason this vercel.json is not ready to host, as sentences. Empty when ready. */
export function hostingConfigProblems(config) {
  const problems = [];
  const rewrites = Array.isArray(config?.rewrites) ? config.rewrites : [];
  const origins = [];
  for (const source of ["/api/:path*", "/healthz"]) {
    const rewrite = rewrites.find((item) => item.source === source);
    const origin = rewrite ? httpsOrigin(rewrite.destination) : null;
    if (!rewrite) problems.push(`rewrites: there is no rewrite for ${source}, so the browser cannot reach the API.`);
    else if (!origin) problems.push(`rewrites: the destination for ${source} is "${rewrite.destination}". Replace its host with the API's public HTTPS address.`);
    else origins.push(origin);
  }
  if (origins.length === 2 && origins[0] !== origins[1]) problems.push(`rewrites: /api and /healthz point at different hosts (${origins[0]} and ${origins[1]}). Both must be the API.`);

  const policy = (config?.headers ?? [])
    .flatMap((entry) => entry.headers ?? [])
    .find((header) => header.key?.toLowerCase() === "content-security-policy")?.value;
  const connect = policy?.split(";").map((part) => part.trim().split(/\s+/)).find((part) => part[0] === "connect-src");
  if (!connect) problems.push("headers: the Content-Security-Policy has no connect-src directive.");
  else {
    const sources = connect.slice(1);
    if (!sources.includes("'self'")) problems.push("headers: connect-src must include 'self' so the page can call /api.");
    const others = sources.filter((source) => source !== "'self'");
    if (others.length === 0) problems.push("headers: connect-src must name the identity provider's origin, or sign-in cannot exchange its code.");
    for (const source of others) {
      if (source.includes("*") || !httpsOrigin(source) || new URL(source).origin !== source)
        problems.push(`headers: connect-src contains "${source}". List exact HTTPS origins only: the identity provider's origin (for example https://your-tenant.us.auth0.com), with no wildcard, scheme-only or placeholder entry.`);
    }
  }
  return problems;
}

async function main() {
  const file = fileURLToPath(new URL("../vercel.json", import.meta.url));
  const problems = hostingConfigProblems(JSON.parse(await readFile(file, "utf8")));
  if (problems.length === 0) {
    console.log("Hosting configuration: frontend/vercel.json names an API host and an identity provider.");
    return;
  }
  const hosted = Boolean(process.env.VERCEL) || process.argv.includes("--strict");
  const lines = [
    hosted
      ? "Nexus cannot be deployed yet: frontend/vercel.json is not filled in."
      : "Note: frontend/vercel.json is not filled in for hosting (this does not affect local development).",
    ...problems.map((problem) => `  - ${problem}`),
    "  See docs/deployment/demo-hosting.md, \"Point the frontend at the API and the identity provider\".",
  ];
  console[hosted ? "error" : "log"](lines.join("\n"));
  if (hosted) process.exit(1);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) await main();
