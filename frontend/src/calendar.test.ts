import assert from "node:assert/strict";
import test from "node:test";
import {
  ALL,
  addDays,
  addMonths,
  createInitial,
  crossesMidnight,
  defaultFacilityId,
  facilityYmd,
  formatYmd,
  intervalOverlapsDay,
  monthGrid,
  pad2,
  selectCalendarItems,
  visibleParticipantNames,
  weekDates,
  weekdayIndex,
  zonedLocalToUtc,
} from "./calendar.ts";
import type { CalendarFilter, Schedulable } from "./calendar.ts";

test("zero padded dates stay sortable calendar keys", () => {
  assert.equal(pad2(0), "00");
  assert.equal(pad2(3), "03");
  assert.equal(pad2(12), "12");
  assert.equal(formatYmd(2026, 1, 5), "2026-01-05");
  assert.equal(formatYmd(2026, 11, 9), "2026-11-09");
});

test("month and year rollover uses calendar dates, not millisecond drift", () => {
  assert.equal(addDays("2026-12-31", 1), "2027-01-01");
  assert.equal(addDays("2026-01-01", -1), "2025-12-31");
  assert.equal(addDays("2026-03-01", -1), "2026-02-28");
  assert.equal(addMonths("2026-12-15", 1), "2027-01-15");
  assert.equal(addMonths("2026-01-31", 1), "2026-02-28");
  assert.equal(addMonths("2026-01-15", -1), "2025-12-15");
});

test("weeks and months always expose all seven weekdays", () => {
  const week = weekDates("2026-03-10");
  assert.equal(week.length, 7);
  assert.deepEqual(week.map(weekdayIndex), [0, 1, 2, 3, 4, 5, 6]);
  assert.equal(week[0], "2026-03-08");
  assert.equal(week[6], "2026-03-14");
  for (const anchor of ["2026-02-15", "2026-12-15", "2027-01-01"]) {
    const grid = monthGrid(anchor);
    assert.equal(grid.length % 7, 0);
    assert.equal(weekdayIndex(grid[0]), 0);
    assert.equal(weekdayIndex(grid[6]), 6);
    for (let index = 1; index < grid.length; index += 1) {
      assert.equal(grid[index], addDays(grid[index - 1], 1));
    }
  }
  const february = monthGrid("2026-02-01");
  assert.ok(february.includes("2026-02-01"));
  assert.ok(february.includes("2026-02-28"));
  const december = monthGrid("2026-12-15");
  assert.ok(december.includes("2026-12-01"));
  assert.ok(december.includes("2026-12-31"));
  assert.ok(december.includes("2027-01-01"));
});

test("facility dates follow DST spring and fall transitions", () => {
  const zone = "America/New_York";
  assert.equal(facilityYmd(new Date("2026-03-08T06:59:00Z"), zone), "2026-03-08");
  assert.equal(facilityYmd(new Date("2026-03-08T07:00:00Z"), zone), "2026-03-08");
  assert.equal(facilityYmd(new Date("2026-03-08T04:30:00Z"), zone), "2026-03-07");
  assert.equal(facilityYmd(new Date("2026-11-01T03:30:00Z"), zone), "2026-10-31");
  assert.equal(facilityYmd(new Date("2026-11-01T05:30:00Z"), zone), "2026-11-01");
  assert.equal(facilityYmd(new Date("2026-11-01T06:30:00Z"), zone), "2026-11-01");
  assert.equal(zonedLocalToUtc("2026-03-08", 1, 30, zone), "2026-03-08T06:30:00.000Z");
  assert.equal(zonedLocalToUtc("2026-03-08", 9, 0, zone), "2026-03-08T13:00:00.000Z");
  assert.equal(zonedLocalToUtc("2026-11-01", 9, 0, zone), "2026-11-01T14:00:00.000Z");
});

test("cross-midnight intervals overlap both facility-local days", () => {
  const zone = "America/Chicago";
  const start = zonedLocalToUtc("2026-06-15", 22, 0, zone);
  const end = zonedLocalToUtc("2026-06-16", 2, 0, zone);
  assert.equal(crossesMidnight(start, end, zone), true);
  assert.equal(intervalOverlapsDay(start, end, "2026-06-15", zone), true);
  assert.equal(intervalOverlapsDay(start, end, "2026-06-16", zone), true);
  assert.equal(intervalOverlapsDay(start, end, "2026-06-14", zone), false);
  const sameStart = zonedLocalToUtc("2026-06-15", 9, 0, zone);
  const sameEnd = zonedLocalToUtc("2026-06-15", 10, 0, zone);
  assert.equal(crossesMidnight(sameStart, sameEnd, zone), false);
  assert.equal(intervalOverlapsDay(sameStart, sameEnd, "2026-06-16", zone), false);
});

test("new appointment initials are UTC for the picked cell, facility, and clinician", () => {
  const initial = createInitial({
    kind: "session",
    day: "2026-03-08",
    hour: 9,
    facilityId: "fac-east",
    clinicianId: "clin-1",
    timeZone: "America/New_York",
  });
  assert.equal(initial.starts_at, "2026-03-08T13:00:00.000Z");
  assert.equal(initial.ends_at, "2026-03-08T14:00:00.000Z");
  assert.equal(String(initial.starts_at).endsWith("Z"), true);
  assert.equal(initial.facility_id, "fac-east");
  assert.equal(initial.clinician_id, "clin-1");
  assert.equal(initial.kind, "individual");
  assert.equal(initial.status, "scheduled");
  const group = createInitial({
    kind: "group",
    day: "2026-11-01",
    hour: 9,
    facilityId: "fac-east",
    clinicianId: "clin-1",
    timeZone: "America/New_York",
  });
  assert.equal(group.starts_at, "2026-11-01T14:00:00.000Z");
  assert.equal(group.kind, "process");
});
test("nonexistent appointment slots fail and default duration uses elapsed time across DST", () => {
  assert.throws(() => zonedLocalToUtc("2026-03-08", 2, 0, "America/New_York"), /does not exist/);
  const initial = createInitial({ kind:"session", day:"2026-03-08", hour:1, minute:30, facilityId:"f", clinicianId:"c", timeZone:"America/New_York" });
  assert.equal(Date.parse(String(initial.ends_at)) - Date.parse(String(initial.starts_at)), 3_600_000);
  assert.equal(initial._require_time_occurrence, true);
});

test("group dedup and filters use synthetic authorized rows only", () => {
  const items: Schedulable[] = [
    {
      id: "g1",
      source: "group",
      facilityId: "f1",
      clinicianId: "c1",
      type: "process",
      status: "scheduled",
      startsAt: "2026-06-15T15:00:00.000Z",
      endsAt: "2026-06-15T16:00:00.000Z",
      linkedSessionIds: ["s1", "s2"],
    },
    {
      id: "s1",
      source: "session",
      facilityId: "f1",
      clinicianId: "c1",
      type: "individual",
      status: "scheduled",
      startsAt: "2026-06-15T15:00:00.000Z",
      endsAt: "2026-06-15T16:00:00.000Z",
    },
    {
      id: "s2",
      source: "session",
      facilityId: "f1",
      clinicianId: "c2",
      type: "individual",
      status: "scheduled",
      startsAt: "2026-06-15T15:00:00.000Z",
      endsAt: "2026-06-15T16:00:00.000Z",
    },
    {
      id: "s3",
      source: "session",
      facilityId: "f1",
      clinicianId: "c1",
      type: "assessment",
      status: "completed",
      startsAt: "2026-06-15T17:00:00.000Z",
      endsAt: "2026-06-15T18:00:00.000Z",
    },
    {
      id: "s4",
      source: "session",
      facilityId: "f2",
      clinicianId: "c1",
      type: "individual",
      status: "scheduled",
      startsAt: "2026-06-15T17:00:00.000Z",
      endsAt: "2026-06-15T18:00:00.000Z",
    },
  ];
  const all: CalendarFilter = { facilityId: ALL, clinicianId: ALL, type: ALL, status: ALL };
  assert.deepEqual(selectCalendarItems(items, all).map((item) => item.id), ["g1", "s3", "s4"]);
  assert.deepEqual(
    selectCalendarItems(items, { ...all, clinicianId: "c2" }).map((item) => item.id),
    ["s2"],
  );
  assert.deepEqual(
    selectCalendarItems(items, { ...all, type: "assessment" }).map((item) => item.id),
    ["s3"],
  );
  assert.deepEqual(
    selectCalendarItems(items, { ...all, status: "completed" }).map((item) => item.id),
    ["s3"],
  );
  assert.deepEqual(
    selectCalendarItems(items, { ...all, facilityId: "f2" }).map((item) => item.id),
    ["s4"],
  );
  assert.deepEqual(
    selectCalendarItems(items, { ...all, facilityId: "f9" }).map((item) => item.id),
    [],
  );
  const disclosure = visibleParticipantNames(["visible", "hidden-id"], {
    visible: "Pat Example",
  });
  assert.deepEqual(disclosure.names, ["Pat Example"]);
  assert.equal(disclosure.hiddenCount, 1);
  assert.equal(JSON.stringify(disclosure).includes("hidden-id"), false);
  assert.equal(
    defaultFacilityId("actor-fac", [
      { id: "other", active: true },
      { id: "actor-fac", active: false },
    ]),
    "actor-fac",
  );
  assert.equal(defaultFacilityId("", [{ id: "only", active: true }]), "only");
  assert.equal(defaultFacilityId("", []), ALL);
});


test("a group suppresses its child only inside the displayed calendar window", () => {
  const all = { facilityId: ALL, clinicianId: ALL, type: ALL, status: ALL };
  const parent: Schedulable = { id: "group", source: "group", facilityId: "f", clinicianId: "c",
    type: "process", status: "scheduled", startsAt: "2026-06-14T23:00:00Z", endsAt: "2026-06-15T00:00:00Z", linkedSessionIds: ["child"] };
  const child: Schedulable = { ...parent, id: "child", source: "session", type: "individual",
    startsAt: "2026-06-15T00:00:00Z", endsAt: "2026-06-15T01:00:00Z" };
  const unrelated = { ...child, id: "unrelated" };
  const items = [parent, child, unrelated];
  const select = (days: string[], filters = all) => selectCalendarItems(items, filters, {days, timeZone: "UTC"});
  const day = select(["2026-06-15"]);
  assert.deepEqual(day.map(x => x.id), ["child", "unrelated"]);
  assert.equal(day.find(x => x.id === "child"), child); // Same collection supplies detail lookup.
  assert.deepEqual(select(["2026-06-14"]).map(x => x.id), ["group"]);
  for (const days of [weekDates("2026-06-15"), monthGrid("2026-06-15")]) {
    assert.deepEqual(select(days).map(x => x.id), ["group", "unrelated"]);
    assert.deepEqual(select(days, {...all, type: "individual"}).map(x => x.id), ["child", "unrelated"]);
  }
  assert.deepEqual(selectCalendarItems([child], all, {days:["2026-06-15"], timeZone:"UTC"}), [child]);
  assert.deepEqual(select(["2026-06-16"]), []);
});
