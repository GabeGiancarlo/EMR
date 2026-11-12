import assert from "node:assert/strict";
import test from "node:test";
import { EXPIRY_WARNING_MS, clockTime, expiryWarning, restoredMidRedirect } from "./sessionRules.ts";

const now = Date.UTC(2026, 10, 12, 15, 40, 0);
const minutes = (count: number) => count * 60_000;

test("the warning window is five minutes", () => {
  assert.equal(EXPIRY_WARNING_MS, minutes(5));
});

test("no session, no warning", () => {
  assert.equal(expiryWarning(null, now), null);
});

test("a session that can be renewed is never warned about, however close its end", () => {
  assert.equal(expiryWarning({ expiresAt: now + minutes(1), renewable: true }, now), null);
  assert.equal(expiryWarning({ expiresAt: now + 1_000, renewable: true }, now), null);
});

test("a session that cannot be renewed is quiet until its last five minutes", () => {
  assert.equal(expiryWarning({ expiresAt: now + minutes(60), renewable: false }, now), null);
  assert.equal(expiryWarning({ expiresAt: now + minutes(5) + 1, renewable: false }, now), null);
});

test("inside the last five minutes the warning names the moment the session ends", () => {
  const expiresAt = now + minutes(5);
  assert.equal(expiryWarning({ expiresAt, renewable: false }, now), expiresAt);
  assert.equal(expiryWarning({ expiresAt, renewable: false }, now + minutes(3)), expiresAt);
  assert.equal(expiryWarning({ expiresAt, renewable: false }, expiresAt - 1), expiresAt);
});

test("once the session has ended there is nothing left to warn about", () => {
  const expiresAt = now + minutes(5);
  assert.equal(expiryWarning({ expiresAt, renewable: false }, expiresAt), null);
  assert.equal(expiryWarning({ expiresAt, renewable: false }, expiresAt + minutes(10)), null);
});

test("the time is written as hours and minutes on the reader's clock", () => {
  const at = Date.UTC(2026, 10, 12, 15, 45, 30);
  assert.equal(clockTime(at, "en-US", "UTC").replace(/\s/g, " "), "3:45 PM");
  assert.equal(clockTime(at, "en-GB", "UTC"), "15:45");
  assert.equal(clockTime(at, "en-US", "America/Chicago").replace(/\s/g, " "), "9:45 AM");
});

test("only a page restored while leaving for sign-in is started again", () => {
  assert.equal(restoredMidRedirect(true, "redirecting"), true);
  assert.equal(restoredMidRedirect(false, "redirecting"), false);
  for (const status of ["loading", "unknown", "local", "signed-out", "signed-in", "expired"])
    assert.equal(restoredMidRedirect(true, status), false);
});
