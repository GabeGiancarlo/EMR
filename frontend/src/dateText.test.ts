import assert from "node:assert/strict";
import test from "node:test";
import {
  durationText,
  formatDateText,
  formatTimeText,
  parseDateText,
  parseTimeText,
  shiftWallTime,
  wallMinutesBetween,
} from "./dateText.ts";

const today = "2026-11-21";

test("a stored day reads as the brand's short date", () => {
  assert.equal(formatDateText("2026-11-18"), "Nov 18, 2026");
  assert.equal(formatDateText("1985-03-04"), "Mar 4, 1985");
  for (const invalid of ["", "2026-02-30", "11/18/2026", "2026-11-18T09:00"]) assert.equal(formatDateText(invalid), "");
});

test("typed dates are understood in the forms staff use", () => {
  const cases: Record<string, string> = {
    "2026-11-18": "2026-11-18",
    "11/18/2026": "2026-11-18",
    "11-18-2026": "2026-11-18",
    "11.18.26": "2026-11-18",
    "1/5/2026": "2026-01-05",
    "11/18": "2026-11-18",
    "11182026": "2026-11-18",
    "031485": "1985-03-14",
    "Nov 18, 2026": "2026-11-18",
    "nov 18": "2026-11-18",
    "November 18 2026": "2026-11-18",
    "sept 3rd, 2026": "2026-09-03",
    "18 Nov 2026": "2026-11-18",
    "  Mar  4,  1985 ": "1985-03-04",
    today: "2026-11-21",
    Tomorrow: "2026-11-22",
    yesterday: "2026-11-20",
  };
  for (const [text, expected] of Object.entries(cases)) assert.equal(parseDateText(text, today), expected, text);
});

test("a two-digit year is recent unless that would be far in the future", () => {
  assert.equal(parseDateText("3/14/85", today), "1985-03-14");
  assert.equal(parseDateText("3/14/30", today), "2030-03-14");
  assert.equal(parseDateText("3/14/37", today), "1937-03-14");
});

test("text that is not a real date is refused rather than guessed", () => {
  for (const text of ["", "   ", "2/30/2026", "13/1/2026", "nov", "november", "18", "2026", "ma 4", "maybe 4", "0000-01-01", "11/18/20261"]) {
    assert.equal(parseDateText(text, today), null, text);
  }
});

test("a stored time reads with AM or PM", () => {
  assert.equal(formatTimeText("00:05"), "12:05 AM");
  assert.equal(formatTimeText("09:30"), "9:30 AM");
  assert.equal(formatTimeText("12:00"), "12:00 PM");
  assert.equal(formatTimeText("15:45"), "3:45 PM");
  for (const invalid of ["", "9:30", "24:00", "12:60", "3 PM"]) assert.equal(formatTimeText(invalid), "");
});

test("typed times are understood with or without AM and PM", () => {
  const cases: Record<string, string> = {
    "3pm": "15:00",
    "3 PM": "15:00",
    "3:30p": "15:30",
    "12:30 p.m.": "12:30",
    "12am": "00:00",
    "12:15 AM": "00:15",
    "1230p": "12:30",
    "930a": "09:30",
    "15:00": "15:00",
    "1500": "15:00",
    "0730": "07:30",
    noon: "12:00",
    Midnight: "00:00",
    // Without AM or PM: clinic hours.
    "9": "09:00",
    "9:15": "09:15",
    "11": "11:00",
    "12": "12:00",
    "1": "13:00",
    "3:30": "15:30",
    "6": "18:00",
    "7": "07:00",
    "0": "00:00",
  };
  for (const [text, expected] of Object.entries(cases)) assert.equal(parseTimeText(text), expected, text);
});

test("text that is not a time is refused", () => {
  for (const text of ["", "25", "13pm", "0pm", "3:60", "3:5", "half past", "12:30:15", "pm"]) {
    assert.equal(parseTimeText(text), null, text);
  }
});

test("moving a start keeps the visit length, across midnight and month ends", () => {
  assert.equal(wallMinutesBetween("2026-11-18T12:30", "2026-11-18T13:30"), 60);
  assert.equal(shiftWallTime("2026-11-18T13:30", 90), "2026-11-18T15:00");
  assert.equal(shiftWallTime("2026-11-30T23:30", 45), "2026-12-01T00:15");
  assert.equal(shiftWallTime("2026-11-18T00:10", -20), "2026-11-17T23:50");
  assert.equal(shiftWallTime("2026-11-18T", 30), "2026-11-18T");
  assert.ok(Number.isNaN(wallMinutesBetween("2026-11-18T12:30", "")));
});

test("a visit length is stated in minutes only when it is a real same-day span", () => {
  assert.equal(durationText("2026-11-18T12:30", "2026-11-18T13:20"), "50 min");
  assert.equal(durationText("2026-11-18T23:30", "2026-11-19T00:30"), "60 min");
  assert.equal(durationText("2026-11-18T12:30", "2026-11-18T12:30"), "");
  assert.equal(durationText("2026-11-18T12:30", "2026-11-18T11:30"), "");
  assert.equal(durationText("2026-11-18T12:30", "2026-11-20T12:30"), "");
  assert.equal(durationText("2026-11-18T12:30", "2026-11-18T1"), "");
});
