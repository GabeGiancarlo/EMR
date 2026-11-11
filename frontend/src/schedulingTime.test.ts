import assert from "node:assert/strict";
import test from "node:test";
import { facilityWallTime, resolveFacilityTime, wallTimeCandidates } from "./schedulingTime.ts";

test("facility appointment time is independent of the browser timezone", () => {
  assert.equal(facilityWallTime("2026-11-02T15:30:00Z", "America/Los_Angeles"), "2026-11-02T07:30");
  assert.equal(resolveFacilityTime("2026-11-02T07:30", "America/Los_Angeles"), "2026-11-02T15:30:00.000Z");
  assert.equal(resolveFacilityTime("2026-06-15T09:00", "Asia/Kathmandu"), "2026-06-15T03:15:00.000Z");
});
test("nonexistent times and invalid dates cannot roll into another appointment time", () => {
  assert.deepEqual(wallTimeCandidates("2026-03-08T02:30", "America/New_York"), []);
  assert.throws(() => resolveFacilityTime("2026-03-08T02:30", "America/New_York"), /does not exist/);
  assert.deepEqual(wallTimeCandidates("2026-02-30T12:00", "UTC"), []);
  assert.deepEqual(wallTimeCandidates("2026-11-02T25:00", "UTC"), []);
});
test("repeated time requires a choice, and preserves the chosen occurrence when edited", () => {
  const candidates = wallTimeCandidates("2026-11-01T01:30", "America/New_York");
  assert.deepEqual(candidates, ["2026-11-01T05:30:00.000Z", "2026-11-01T06:30:00.000Z"]);
  assert.throws(() => resolveFacilityTime("2026-11-01T01:30", "America/New_York"), /occurs twice/);
  assert.equal(resolveFacilityTime("2026-11-01T01:30", "America/New_York", candidates[1]), candidates[1]);
  assert.throws(() => resolveFacilityTime("2026-11-01T01:30", "America/New_York", "stale choice"), /occurs twice/);
});
test("half-hour DST shifts and skipped calendar dates are handled", () => {
  assert.equal(wallTimeCandidates("2026-04-05T01:45", "Australia/Lord_Howe").length, 2);
  assert.equal(wallTimeCandidates("2011-12-30T12:00", "Pacific/Apia").length, 0);
});
test("editing an unrelated field preserves original timestamp precision", () => {
  assert.equal(resolveFacilityTime("2026-11-02T10:00", "America/New_York", undefined, "2026-11-02T15:00:32.123Z"), "2026-11-02T15:00:32.123Z");
  assert.equal(resolveFacilityTime("2026-11-02T10:01", "America/New_York", undefined, "2026-11-02T15:00:32.123Z"), "2026-11-02T15:01:00.000Z");
});
