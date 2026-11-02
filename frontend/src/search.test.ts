import assert from "node:assert/strict";
import test from "node:test";
import type { Bootstrap } from "./api.ts";
import { searchWorkspace, visibleViews } from "./search.ts";

function fixture(): Bootstrap {
  return { actor: { id: "actor" }, clients: [{ id: "client", first_name: "Zoë", last_name: "Synthetic", preferred_name: "Z", status: "active" }],
    notes: [{ id: "note", client_id: "client", title: "Progress review", body: "forbidden narrative keyword", status: "draft" }],
    claims: [{ id: "claim", client_id: "client", service_code: "H0015", external_reference: "REF-10", status: "denied" }],
    tasks: [{ id: "task", client_id: "client", title: "Review plan" }],
    permissions: { clients: { read: true, write: false }, tasks: { read: true, write: true }, notes: { read: false, write: false }, claims: { read: true, write: false } },
  } as unknown as Bootstrap;
}
test("global search denies missing/false permission even if a stale array contains records", () => {
  const data = fixture();
  assert.deepEqual(searchWorkspace(data, "progress"), []);
  delete data.permissions!.claims;
  assert.deepEqual(searchWorkspace(data, "REF-10"), []);
  assert.equal(visibleViews(data).includes("notes"), false);
  assert.equal(visibleViews(data).includes("billing"), false);
});
test("global search normalizes names and ANDs tokens across authorized record context", () => {
  const data = fixture();
  assert.equal(searchWorkspace(data, "zoe synthetic")[0].resource, "clients");
  assert.equal(searchWorkspace(data, "zoe denied")[0].resource, "claims");
  assert.equal(searchWorkspace(data, "REF-10")[0].row.id, "claim");
  assert.deepEqual(searchWorkspace(data, "zoe absent"), []);
  assert.deepEqual(searchWorkspace(data, "  "), []);
});
test("search never indexes narratives and cannot enrich with an unauthorized client name", () => {
  const data = fixture();
  data.permissions!.notes = { read: true, write: true };
  assert.deepEqual(searchWorkspace(data, "forbidden"), []);
  data.permissions!.clients!.read = false;
  assert.equal(searchWorkspace(data, "zoe denied").length, 0);
  const result = searchWorkspace(data, "REF-10")[0];
  assert.ok(!result.title.includes("Synthetic"));
  assert.ok(!result.detail.includes("Synthetic"));
});
test("search is empty after leaving the workspace", () => {
  const data = fixture(); data.actor = null;
  assert.deepEqual(searchWorkspace(data, "zoe"), []);
});
