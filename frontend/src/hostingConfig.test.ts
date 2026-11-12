import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
// @ts-expect-error The build script is plain JavaScript.
import { hostingConfigProblems } from "../scripts/check-hosting-config.mjs";

type Config = { rewrites: { source: string; destination: string }[]; headers: { source: string; headers: { key: string; value: string }[] }[] };
const tracked = await readFile(new URL("../vercel.json", import.meta.url), "utf8");
const policy = (config: Config) => config.headers.flatMap((entry) => entry.headers).find((header) => header.key === "Content-Security-Policy")!;
/**
 * The tracked file with example addresses in place of whatever it holds, placeholder or
 * real, so these tests keep their meaning before and after the file is filled in.
 */
const filled = (): Config => {
  const config: Config = JSON.parse(tracked);
  for (const rewrite of config.rewrites) rewrite.destination = rewrite.destination.replace(/^https:\/\/[^/]+/, "https://nexus-api.onrender.com");
  policy(config).value = policy(config).value.replace(/connect-src [^;]*/, "connect-src 'self' https://nexus-demo.us.auth0.com");
  return config;
};

test("the tracked vercel.json is ready once its addresses are written in", () => {
  assert.deepEqual(hostingConfigProblems(filled()), []);
});

test("a placeholder API host or provider origin is reported, each by name", () => {
  const unset = filled();
  for (const rewrite of unset.rewrites) rewrite.destination = rewrite.destination.replace("nexus-api.onrender.com", "REPLACE-WITH-API-HOST.example.invalid");
  policy(unset).value = policy(unset).value.replace("nexus-demo.us.auth0.com", "REPLACE-WITH-IDENTITY-PROVIDER-HOST.example.invalid");
  const problems: string[] = hostingConfigProblems(unset);
  assert.equal(problems.length, 3, problems.join("\n"));
  assert.ok(problems.some((problem) => problem.includes("/api/:path*")));
  assert.ok(problems.some((problem) => problem.includes("/healthz")));
  assert.ok(problems.some((problem) => problem.includes("connect-src")));
});

test("the policy may reach only the page's own origin and named HTTPS origins", () => {
  for (const loose of ["https:", "*", "https://*.auth0.com", "http://nexus-demo.us.auth0.com", "https://nexus-demo.us.auth0.com/oauth/token"]) {
    const config = filled();
    policy(config).value = policy(config).value.replace("https://nexus-demo.us.auth0.com", loose);
    assert.equal(hostingConfigProblems(config).length, 1, loose);
  }
  const none = filled();
  policy(none).value = policy(none).value.replace(" https://nexus-demo.us.auth0.com", "");
  assert.equal(hostingConfigProblems(none).length, 1, "no provider origin");
  const two = filled();
  policy(two).value = policy(two).value.replace("https://nexus-demo.us.auth0.com", "https://nexus-demo.us.auth0.com https://nexus-api.onrender.com");
  assert.deepEqual(hostingConfigProblems(two), [], "a second exact origin is allowed, for direct API calls");
});

test("the API rewrites must be HTTPS and agree on one host", () => {
  const plain = filled();
  plain.rewrites[0].destination = "http://nexus-api.onrender.com/api/:path*";
  assert.equal(hostingConfigProblems(plain).length, 1);
  const split = filled();
  split.rewrites[1].destination = "https://another-host.onrender.com/healthz";
  assert.equal(hostingConfigProblems(split).length, 1);
  const missing = filled();
  missing.rewrites = missing.rewrites.filter((rewrite) => rewrite.source !== "/healthz");
  assert.equal(hostingConfigProblems(missing).length, 1);
});
