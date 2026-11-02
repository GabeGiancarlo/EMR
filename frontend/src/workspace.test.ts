import assert from "node:assert/strict";
import test from "node:test";
import type { Bootstrap, Row } from "./api.ts";
import { allowedWorkspaceModes, defaultWorkspaceMode, visibleRows, noteActionsAllowed, flowLedger, currentOutstanding, claimBalance, calendarDateInZone, currentCensus, currentStaffing, upcomingPlanReviews, expiringAuthorizations, claimMatchesFilter, operationalWorkload } from "./workspace.ts";

const row = (id: string, fields: Record<string, unknown> = {}): Row => ({ id, ...fields });
function fixture(role = "admin"): Bootstrap {
  const resources = ["facilities","staff","clients","episodes","sessions","notes","tasks","policies","authorizations","claims","payments","payment_reversals","templates","consents","exceptions","treatment_plans","goals","plan_reviews","groups","group_attendance","form_responses"];
  return { ...Object.fromEntries(resources.map(key => [key, []])), actor:row("actor", {role,facility_id:"facility"}), permissions:Object.fromEntries(resources.map(key=>[key,{read:true,write:role === "admin"}])),
    facilities:[row("facility",{timezone:"America/New_York",active:true,name:"Synthetic facility"})],
    clients:[row("client",{facility_id:"facility"})], episodes:[row("episode",{client_id:"client",facility_id:"facility",level_of_care:"iop",status:"active"})],
    sessions:[row("session",{client_id:"client",episode_id:"episode",starts_at:"2026-03-01T02:00:00Z"})],
    metrics:{draft_notes:0},
  } as unknown as Bootstrap;
}
test("workspace modes never grant record access or note actions",()=>{
  const support = fixture("support");
  support.permissions = {sessions:{read:true,write:true},clients:{read:true,write:true},notes:{read:false,write:false}};
  support.notes=[row("secret",{title:"Not authorized"})];
  assert.deepEqual(allowedWorkspaceModes(support),["clinical"]);
  assert.equal(visibleRows(support,"notes"),null);
  assert.equal(noteActionsAllowed(support),false);
  const billing=fixture("billing"); billing.permissions={claims:{read:true,write:true},notes:{read:false,write:false}};
  assert.deepEqual(allowedWorkspaceModes(billing),["billing"]);
  assert.equal(defaultWorkspaceMode(billing),"billing");
  billing.permissions.notes={read:true,write:false};
  assert.deepEqual(allowedWorkspaceModes(billing),["clinical","billing"]);
  assert.equal(noteActionsAllowed(billing),false);
  const executive=fixture("executive"); executive.permissions={claims:{read:true,write:false}};
  assert.deepEqual(allowedWorkspaceModes(executive),["executive"]);
});
test("ledger flow accounts for reversals in their own period, separately from adjustments",()=>{
  const data=fixture();
  data.claims=[row("claim",{amount_cents:100000,status:"submitted",session_id:"session"})];
  data.payments=[row("old",{claim_id:"claim",source:"payer",amount_cents:60000,received_on:"2026-02-25"}),row("new",{claim_id:"claim",source:"client",amount_cents:30000,received_on:"2026-03-05"}),row("adjust",{claim_id:"claim",source:"adjustment",amount_cents:10000,received_on:"2026-03-05"})];
  data.payment_reversals=[row("reverse",{claim_id:"claim",payment_id:"old",amount_cents:60000,reversed_on:"2026-03-02"})];
  const march=flowLedger(data,"2026-03-01","2026-03-31");
  assert.deepEqual(march.cash,{state:"value",cents:-30000});
  assert.deepEqual(march.adjustments,{state:"value",cents:10000});
  assert.deepEqual(march.charges,{state:"value",cents:0});
  assert.deepEqual(march.outstanding,{state:"value",cents:60000});
  assert.deepEqual(claimBalance(data,data.claims[0]),march.outstanding);
  assert.deepEqual(flowLedger(data,"2026-02-01","2026-02-28").charges,{state:"value",cents:100000});
  assert.deepEqual(flowLedger(data,"2026-03-31","2026-03-01").outstanding,march.outstanding);
  assert.equal(flowLedger(data,"2026-03-31","2026-03-01").cash.state,"unknown");
});
test("missing or malformed financial inputs are distinct from a valid zero",()=>{
  const data=fixture();
  assert.deepEqual(currentOutstanding(data),{state:"value",cents:0});
  data.permissions!.payments!.read=false;
  assert.equal(currentOutstanding(data).state,"unavailable");
  data.permissions!.payments!.read=true;
  data.payments=[row("bad",{source:"payer",amount_cents:10000,received_on:""})];
  assert.equal(flowLedger(data,"2026-03-01","2026-03-31").cash.state,"unknown");
  data.payments=[];
  data.payment_reversals=[row("orphan",{payment_id:"missing",amount_cents:100,reversed_on:"2026-03-02"})];
  assert.equal(flowLedger(data,"2026-03-01","2026-03-31").cash.state,"unknown");
});
test("date bucketing follows facility dates at month and DST boundaries",()=>{
  assert.equal(calendarDateInZone(new Date("2026-03-01T02:00:00Z"),"America/New_York"),"2026-02-28");
  assert.equal(calendarDateInZone(new Date("2026-11-01T06:30:00Z"),"America/New_York"),"2026-11-01");
  assert.equal(calendarDateInZone(new Date("2026-03-01T02:00:00Z"),"invalid"),null);
});
test("review and authorization queues exclude completed plans and inactive approvals",()=>{
  const data=fixture();
  data.treatment_plans=[row("active",{status:"active",next_review_date:"2026-03-10"}),row("old",{status:"superseded",next_review_date:"2026-03-01"}),row("done",{status:"completed",next_review_date:"2026-03-02"})];
  assert.deepEqual(upcomingPlanReviews(data,"2026-03-11").dated.map(row=>row.id),["active"]);
  data.authorizations=[row("active",{status:"active",ends_on:"2026-03-20"}),row("ended",{status:"expired",ends_on:"2026-03-15"}),row("later",{status:"active",ends_on:"2026-05-01"})];
  assert.deepEqual(expiringAuthorizations(data,"2026-03-10").rows.map(row=>row.id),["active"]);
});
test("census counts active episodes and staffing requires explicit active state",()=>{
  const data=fixture();
  data.episodes.push(row("planned",{status:"planned",facility_id:"facility"}),row("second",{status:"active",facility_id:"facility",level_of_care:"php"}));
  assert.equal(currentCensus(data).totalActive,2);
  assert.deepEqual(currentCensus(data).facilities[0].levels,[{level:"iop",count:1},{level:"php",count:1}]);
  data.staff=[row("on",{active:true,role:"clinician"}),row("off",{active:false,role:"clinician"}),row("unknown",{role:"support"})];
  assert.equal(currentStaffing(data).activeTotal,1);
  assert.equal(currentStaffing(data).unclassified,1);
  data.permissions!.episodes!.read=false;
  assert.equal(currentCensus(data).totalActive,null);
});
test("claim drill-down filters distinguish balances from drafts and voids",()=>{
  assert.equal(claimMatchesFilter("draft","balance"),false);
  assert.equal(claimMatchesFilter("submitted","balance"),true);
  assert.equal(claimMatchesFilter("voided","attention"),false);
  assert.equal(claimMatchesFilter("denied","denied"),true);
});
test("denied clinical counts are unavailable even when bootstrap includes a metric",()=>{
  const data=fixture("executive");
  data.permissions!.notes={read:false,write:false};
  data.metrics!.draft_notes=0;
  assert.equal(operationalWorkload(data,"2026-03-10").drafts.available,false);
  assert.equal(operationalWorkload(data,"2026-03-10").drafts.count,null);
});
