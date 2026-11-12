import assert from "node:assert/strict";
import test from "node:test";
import {
  ApiRequestError, TIMEOUT_MESSAGE, UNREACHABLE_MESSAGE, isUnreachable, requestFailure, sendWithRecovery,
} from "./transport.ts";

const named = (name: string, message: string) => Object.assign(new Error(message), { name });
const answer = (status: number) => ({ status });

test("a rejected fetch becomes a plain message, never the browser's wording", () => {
  for (const raw of [new TypeError("Failed to fetch"), new TypeError("Load failed"), new TypeError("NetworkError when attempting to fetch resource."), "offline", undefined]) {
    const failure = requestFailure(raw);
    assert.ok(failure instanceof ApiRequestError);
    assert.equal(failure.message, UNREACHABLE_MESSAGE);
    assert.equal(failure.status, 0);
    assert.equal(failure.code, "network");
  }
});

test("a timeout is reported as its own unreachable case", () => {
  const failure = requestFailure(named("TimeoutError", "signal timed out"));
  assert.ok(failure instanceof ApiRequestError);
  assert.equal(failure.message, TIMEOUT_MESSAGE);
  assert.equal(failure.status, 0);
  assert.equal(failure.code, "timeout");
});

test("a request the caller cancelled is passed through for the caller to recognise", () => {
  const cancelled = named("AbortError", "The user aborted a request.");
  assert.equal(requestFailure(cancelled), cancelled);
});

test("an error that already carries a status is not rewritten", () => {
  const refused = new ApiRequestError("Not allowed.", 403, "forbidden");
  assert.equal(requestFailure(refused), refused);
});

test("unreachable means no answer or a server fault, not a refusal", () => {
  assert.equal(isUnreachable(new ApiRequestError(UNREACHABLE_MESSAGE, 0, "network")), true);
  assert.equal(isUnreachable(new ApiRequestError("Bad gateway", 502, "")), true);
  assert.equal(isUnreachable(new TypeError("Failed to fetch")), true);
  assert.equal(isUnreachable(new ApiRequestError("No access", 403, "no_access")), false);
  assert.equal(isUnreachable(new ApiRequestError("Sign in", 401, "")), false);
});

test("an accepted request is sent once and the session is left alone", async () => {
  let sent = 0;
  let recovered = 0;
  const response = await sendWithRecovery(async () => { sent++; return answer(200); }, async () => { recovered++; return true; });
  assert.deepEqual([response.status, sent, recovered], [200, 1, 0]);
});

test("a 401 is repeated once after the session is recovered", async () => {
  const statuses = [401, 200];
  let sent = 0;
  const response = await sendWithRecovery(async () => answer(statuses[sent++]), async () => true);
  assert.deepEqual([response.status, sent], [200, 2]);
});

test("a 401 is returned as it is when the session cannot be recovered", async () => {
  let sent = 0;
  const response = await sendWithRecovery(async () => { sent++; return answer(401); }, async () => false);
  assert.deepEqual([response.status, sent], [401, 1]);
});

test("a second 401 is not repeated again", async () => {
  let sent = 0;
  let recovered = 0;
  const response = await sendWithRecovery(async () => { sent++; return answer(401); }, async () => { recovered++; return true; });
  assert.deepEqual([response.status, sent, recovered], [401, 2, 1]);
});

test("other refusals are never repeated", async () => {
  for (const status of [400, 403, 404, 409, 500]) {
    let sent = 0;
    let recovered = 0;
    await sendWithRecovery(async () => { sent++; return answer(status); }, async () => { recovered++; return true; });
    assert.deepEqual([sent, recovered], [1, 0]);
  }
});

test("a connection that fails, first time or on the repeat, is reported plainly", async () => {
  await assert.rejects(
    sendWithRecovery(async () => { throw new TypeError("Failed to fetch"); }, async () => true),
    (error: unknown) => error instanceof ApiRequestError && error.status === 0 && error.code === "network" && error.message === UNREACHABLE_MESSAGE,
  );
  let sent = 0;
  await assert.rejects(
    sendWithRecovery(async () => { if (sent++ === 0) return answer(401); throw new TypeError("Failed to fetch"); }, async () => true),
    (error: unknown) => error instanceof ApiRequestError && error.code === "network",
  );
  assert.equal(sent, 2);
});

test("a cancelled request rejects with the original AbortError", async () => {
  const cancelled = named("AbortError", "The user aborted a request.");
  await assert.rejects(sendWithRecovery(async () => { throw cancelled; }, async () => true), (error: unknown) => error === cancelled);
});
