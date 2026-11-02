//! Authoritative billing-readiness rules for a claim already loaded in a transaction.
//! Checklist and mark-ready/record-submission must both call [`evaluate`].
use crate::{access, access::Actor, records, AppState, Result};
use actix_web::{web, HttpRequest, HttpResponse};
use chrono::NaiveDate;
use serde::Serialize;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct Check {
    pub code: &'static str,
    pub title: &'static str,
    pub passed: bool,
    pub message: String,
    pub owner_role: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Readiness {
    pub ready: bool,
    pub checklist: Vec<Check>,
}

fn check(
    checklist: &mut Vec<Check>,
    code: &'static str,
    title: &'static str,
    passed: bool,
    message: impl Into<String>,
    owner_role: &'static str,
) {
    checklist.push(Check {
        code,
        title,
        passed,
        message: message.into(),
        owner_role,
    });
}

fn code_present(value: &Value) -> bool {
    value.as_str().is_some_and(|text| !text.trim().is_empty())
}

fn policy_problem(policy: &Value, date: &str) -> Option<String> {
    let mut reasons = Vec::new();
    if policy["status"] != "active" {
        reasons.push(format!(
            "status is {}",
            policy["status"].as_str().unwrap_or("missing")
        ));
    }
    let effective = policy["effective_date"].as_str().unwrap_or_default();
    if date < effective {
        reasons.push(format!(
            "facility service date {date} is before effective date {effective}"
        ));
    }
    if policy["termination_date"]
        .as_str()
        .is_some_and(|end| date > end)
    {
        reasons.push(format!(
            "facility service date {date} is after termination date {}",
            policy["termination_date"].as_str().unwrap_or_default()
        ));
    }
    if reasons.is_empty() {
        None
    } else {
        Some(format!(
            "The policy must cover the session date ({})",
            reasons.join("; ")
        ))
    }
}

fn authorization_problem(authorization: &Value, claim: &Value, date: &str) -> Option<String> {
    let mut reasons = Vec::new();
    if authorization["policy_id"] != claim["policy_id"] {
        reasons.push("it is for a different policy".to_string());
    }
    if authorization["service_code"] != claim["service_code"] {
        reasons.push(format!(
            "its service code is {} rather than {}",
            authorization["service_code"].as_str().unwrap_or("missing"),
            claim["service_code"].as_str().unwrap_or("missing")
        ));
    }
    if authorization["status"] != "active" {
        reasons.push(format!(
            "its status is {}",
            authorization["status"].as_str().unwrap_or("missing")
        ));
    }
    let starts = authorization["starts_on"].as_str().unwrap_or_default();
    let ends = authorization["ends_on"].as_str().unwrap_or_default();
    if date < starts {
        reasons.push(format!("facility service date {date} is before {starts}"));
    }
    if date > ends {
        reasons.push(format!("facility service date {date} is after {ends}"));
    }
    if reasons.is_empty() {
        None
    } else {
        Some(format!(
            "Authorization must cover this policy, service and service date ({})",
            reasons.join("; ")
        ))
    }
}

fn units_problem(used: i64, units: i64, approved: i64) -> Option<String> {
    if used + units > approved {
        Some(format!(
            "This claim exceeds the remaining authorized units ({used} already reserved, {units} on this claim, {approved} approved)."
        ))
    } else {
        None
    }
}

/// Evaluate readiness using the locked claim and the same related-row locks as claim transition.
/// Callers must already be inside the organization transaction. This re-locks the claim, then the
/// session, policy, and selected authorization, in that order.
pub async fn evaluate(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    claim: &Value,
) -> Result<Readiness> {
    let claim_id = records::uuid(claim, "id")?;
    let claim = records::get(tx, "claims", claim_id, actor.org, true).await?;
    let session_id = records::uuid(&claim, "session_id")?;
    let session = records::get(tx, "sessions", session_id, actor.org, true).await?;

    let mut checklist = Vec::with_capacity(8);
    let completed = session["status"] == "completed";
    check(
        &mut checklist,
        "session_completed",
        "Session completed",
        completed,
        if completed {
            "Session status is completed.".to_string()
        } else {
            format!(
                "Complete the session before marking the claim ready (current status: {}).",
                session["status"].as_str().unwrap_or("missing")
            )
        },
        "clinical",
    );
    match records::ensure_session_ended(&session) {
        Ok(()) => check(
            &mut checklist,
            "session_ended",
            "Session ended",
            true,
            "The session end time has passed.",
            "clinical",
        ),
        Err(err) => check(
            &mut checklist,
            "session_ended",
            "Session ended",
            false,
            err.message,
            "clinical",
        ),
    }

    let signed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM notes WHERE organization_id=$1 AND session_id=$2 AND status='signed')",
    )
    .bind(actor.org)
    .bind(session_id)
    .fetch_one(&mut **tx)
    .await?;
    check(
        &mut checklist,
        "signed_note",
        "Signed session note",
        signed,
        if signed {
            "A signed note exists for this session.".to_string()
        } else {
            "A signed session note is required for billing readiness. No signed note is on file for this session.".to_string()
        },
        "clinical",
    );

    let policy_id = records::uuid(&claim, "policy_id")?;
    let policy = records::get(tx, "policies", policy_id, actor.org, true).await?;
    let service_date: NaiveDate = sqlx::query_scalar(
        "SELECT (s.starts_at AT TIME ZONE f.timezone)::date FROM sessions s JOIN episodes e ON e.id=s.episode_id AND e.organization_id=s.organization_id JOIN facilities f ON f.id=e.facility_id AND f.organization_id=e.organization_id WHERE s.id=$1 AND s.organization_id=$2",
    )
    .bind(session_id)
    .bind(actor.org)
    .fetch_one(&mut **tx)
    .await?;
    let date = service_date.format("%Y-%m-%d").to_string();
    let policy_failure = policy_problem(&policy, &date);
    check(
        &mut checklist,
        "policy_coverage",
        "Policy coverage",
        policy_failure.is_none(),
        policy_failure.unwrap_or_else(|| {
            format!("The policy is active and covers the facility service date {date}.")
        }),
        "billing",
    );

    let service_ok = code_present(&claim["service_code"]);
    check(
        &mut checklist,
        "service_code",
        "Service code",
        service_ok,
        if service_ok {
            format!(
                "Service code {} is present.",
                claim["service_code"].as_str().unwrap_or_default().trim()
            )
        } else {
            "A service code is required for billing readiness.".to_string()
        },
        "billing",
    );
    let diagnosis_ok = code_present(&claim["diagnosis_code"]);
    check(
        &mut checklist,
        "diagnosis_code",
        "Diagnosis code",
        diagnosis_ok,
        if diagnosis_ok {
            format!(
                "Diagnosis code {} is present.",
                claim["diagnosis_code"].as_str().unwrap_or_default().trim()
            )
        } else {
            "A diagnosis code is required for billing readiness.".to_string()
        },
        "billing",
    );

    if claim["authorization_id"].is_null() {
        check(
            &mut checklist,
            "authorization_coverage",
            "Authorization coverage",
            true,
            "No authorization is selected. A selected authorization must match the policy, service code, status, and facility service date.",
            "billing",
        );
        check(
            &mut checklist,
            "authorization_units",
            "Authorized units",
            true,
            "No authorization is selected, so this claim does not reserve units.",
            "billing",
        );
    } else {
        let authorization_id = records::uuid(&claim, "authorization_id")?;
        let authorization =
            records::get(tx, "authorizations", authorization_id, actor.org, true).await?;
        let coverage_failure = authorization_problem(&authorization, &claim, &date);
        check(
            &mut checklist,
            "authorization_coverage",
            "Authorization coverage",
            coverage_failure.is_none(),
            coverage_failure.unwrap_or_else(|| {
                format!(
                    "Authorization {} covers this policy, service code, and facility service date {date}.",
                    authorization["authorization_number"]
                        .as_str()
                        .unwrap_or("")
                        .trim()
                )
            }),
            "billing",
        );
        let used: i64 = sqlx::query_scalar(
            "SELECT COALESCE(sum(units),0)::bigint FROM claims WHERE organization_id=$1 AND authorization_id=$2 AND id<>$3 AND status IN ('ready','submitted','denied','paid')",
        )
        .bind(actor.org)
        .bind(authorization_id)
        .bind(claim_id)
        .fetch_one(&mut **tx)
        .await?;
        let units = claim["units"].as_i64().unwrap_or(1);
        let approved = authorization["units_approved"].as_i64().unwrap_or_default();
        let units_failure = units_problem(used, units, approved);
        check(
            &mut checklist,
            "authorization_units",
            "Authorized units",
            units_failure.is_none(),
            units_failure.unwrap_or_else(|| {
                format!(
                    "{used} units are already reserved and this claim uses {units} of {approved} approved units."
                )
            }),
            "billing",
        );
    }

    let ready = checklist.iter().all(|item| item.passed);
    Ok(Readiness { ready, checklist })
}

pub async fn claim_readiness(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let id = path.into_inner();
    let actor = access::actor(&req, &state.pool).await?;
    access::require(&state.pool, &actor, "claims", false).await?;
    let mut tx = state.pool.begin().await?;
    let claim = records::get(&mut tx, "claims", id, actor.org, true).await?;
    records::scope(&mut tx, &actor, "claims", &claim).await?;
    let report = evaluate(&mut tx, &actor, &claim).await?;
    access::audit(&mut tx, &actor, "readiness", "claims", Some(id)).await?;
    tx.commit().await?;
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store, private"))
        .json(report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn policy_rule_uses_status_and_facility_date_bounds() {
        let covered = json!({
            "status": "active",
            "effective_date": "2026-02-01",
            "termination_date": "2026-02-01"
        });
        assert!(policy_problem(&covered, "2026-02-01").is_none());
        let late = policy_problem(&covered, "2026-02-02").unwrap();
        assert!(late.contains("policy"));
        assert!(late.contains("2026-02-02"));
        let open = json!({
            "status": "active",
            "effective_date": "2026-01-01",
            "termination_date": null
        });
        assert!(policy_problem(&open, "2026-02-01").is_none());
        let inactive = json!({
            "status": "inactive",
            "effective_date": "2026-02-01",
            "termination_date": null
        });
        assert!(policy_problem(&inactive, "2026-02-01")
            .unwrap()
            .contains("inactive"));
    }

    #[test]
    fn authorization_rule_checks_policy_code_status_and_dates() {
        let claim = json!({"policy_id": "p1", "service_code": "QA"});
        let authorization = json!({
            "policy_id": "p1",
            "service_code": "QA",
            "status": "active",
            "starts_on": "2026-02-01",
            "ends_on": "2026-02-01",
            "authorization_number": "QA-PACIFIC"
        });
        assert!(authorization_problem(&authorization, &claim, "2026-02-01").is_none());
        let wrong_code = json!({
            "policy_id": "p1",
            "service_code": "OTHER",
            "status": "active",
            "starts_on": "2026-02-01",
            "ends_on": "2026-02-01"
        });
        let problem = authorization_problem(&wrong_code, &claim, "2026-02-01").unwrap();
        assert!(problem.contains("service"));
        assert!(authorization_problem(&authorization, &claim, "2026-01-31").is_some());
    }

    #[test]
    fn units_rule_reserves_non_draft_units() {
        assert!(units_problem(0, 1, 1).is_none());
        let exceeded = units_problem(1, 1, 1).unwrap();
        assert!(exceeded.contains("authorized units"));
        assert!(exceeded.contains("1 already reserved"));
    }

    #[test]
    fn service_and_diagnosis_codes_must_be_nonempty() {
        assert!(code_present(&json!("QA")));
        assert!(!code_present(&json!("  ")));
        assert!(!code_present(&json!(null)));
    }
}
