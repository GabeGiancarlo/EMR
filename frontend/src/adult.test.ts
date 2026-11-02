import assert from "node:assert/strict";
import test from "node:test";
import { adultAnniversary, facilityLocalToday, hasCompletedAdultYears, latestEligibleBirthDate } from "./adult.ts";
test("adult eligibility changes on the eighteenth birthday, including leap-day policy", () => {
  assert.equal(hasCompletedAdultYears("2008-06-15", "2026-06-14"), false);
  assert.equal(hasCompletedAdultYears("2008-06-15", "2026-06-15"), true);
  assert.equal(adultAnniversary("2004-02-29"), "2022-03-01");
  assert.equal(hasCompletedAdultYears("2004-02-29", "2022-02-28"), false);
  assert.equal(hasCompletedAdultYears("2004-02-29", "2022-03-01"), true);
  assert.equal(hasCompletedAdultYears("2005-02-29", "2026-06-15"), false);
});
test("latest eligible date agrees with the age gate across calendar boundaries", () => {
  for (const [today, cutoff] of [
    ["2026-06-15", "2008-06-15"], ["2022-02-28", "2004-02-28"],
    ["2022-03-01", "2004-03-01"], ["2024-02-29", "2006-02-28"], ["2027-01-01", "2009-01-01"],
  ]) {
    assert.equal(latestEligibleBirthDate(today), cutoff);
    assert.equal(hasCompletedAdultYears(cutoff, today), true);
    const next = new Date(cutoff + "T00:00:00Z");
    next.setUTCDate(next.getUTCDate() + 1);
    assert.equal(hasCompletedAdultYears(next.toISOString().slice(0, 10), today), false);
  }
});
test("facility calendars can have different eligibility dates at the same instant", () => {
  const instant = new Date("2026-06-15T02:30:00Z");
  const ahead = facilityLocalToday("Pacific/Kiritimati", instant);
  const behind = facilityLocalToday("Etc/GMT+12", instant);
  assert.equal(ahead, "2026-06-15");
  assert.equal(behind, "2026-06-14");
  assert.equal(hasCompletedAdultYears("2008-06-15", ahead), true);
  assert.equal(hasCompletedAdultYears("2008-06-15", behind), false);
});
