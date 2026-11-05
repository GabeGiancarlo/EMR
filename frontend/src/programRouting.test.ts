import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { availableServices, claimServiceDefaults, serviceDurationError, serviceSnapshot } from "./programRouting.ts";
import type { Bootstrap, Row } from "./api";
const service: Row = { id:"service",program_id:"program",active:true,session_kind:"individual",service_code:"ORG-1",min_minutes:30,max_minutes:60,default_charge_cents:12500 };
describe("program service routing", () => {
  it("suggests billing from the saved encounter, including zero fees, without inventing units", () => {
    const session={id:"visit",program_service_id:"service",service_snapshot:service};
    assert.deepEqual(claimServiceDefaults(session),{service_code:"ORG-1",amount_cents:12500});
    assert.deepEqual(claimServiceDefaults({...session,service_snapshot:{...service,default_charge_cents:0}}),{service_code:"ORG-1",amount_cents:0});
    assert.equal(serviceSnapshot({...session,program_service_id:"other"}),undefined);
  });
  it("uses exact elapsed duration and inclusive bounds", () => {
    assert.equal(serviceDurationError(service,"2026-01-01T12:00:00Z","2026-01-01T12:30:00Z"),undefined);
    assert.match(serviceDurationError(service,"2026-01-01T12:00:00Z","2026-01-01T12:29:59Z") || "",/30–60/);
    assert.match(serviceDurationError(service,"2026-01-01T12:00:00Z","2026-01-01T13:00:01Z") || "",/30–60/);
  });
  it("filters by active episode program or group facility", () => {
    const data={episodes:[{id:"episode",program_id:"program"}],facilities:[{id:"facility",active:true}],care_programs:[{id:"program",facility_id:"facility",active:true}],program_services:[service,{...service,id:"group",session_kind:"group"},{...service,id:"retired",active:false}]} as Bootstrap;
    assert.deepEqual(availableServices(data,"sessions",{episode_id:"episode"}).map(s=>s.id),["service"]);
    assert.deepEqual(availableServices(data,"sessions",{episode_id:"other"}),[]);
    assert.deepEqual(availableServices(data,"groups",{facility_id:"facility"}).map(s=>s.id),["group"]);
  });
});
