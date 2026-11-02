//! Stable registration identifiers and explicit possible-duplicate review.
use crate::{
    access::{self, Actor},
    records, ApiError, AppState, Result,
};
use actix_web::{web, HttpRequest, HttpResponse};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/intake/matches", web::post().to(find_matches));
}
pub(crate) fn fingerprint(client: &Value) -> String {
    let input = json!([
        client["first_name"],
        client["last_name"],
        client["birth_date"],
        client["mrn"]
    ]);
    format!("{:x}", Sha256::digest(input.to_string().as_bytes()))
}
fn normalized(v: &Value, key: &str) -> String {
    v[key]
        .as_str()
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
fn phone(v: &Value) -> String {
    v["phone"]
        .as_str()
        .unwrap_or_default()
        .chars()
        .filter(char::is_ascii_digit)
        .collect()
}
fn reasons(input: &Value, other: &Value) -> Vec<&'static str> {
    if input["birth_date"] != other["birth_date"] {
        return vec![];
    }
    let mut out = Vec::new();
    if normalized(input, "first_name") == normalized(other, "first_name")
        && normalized(input, "last_name") == normalized(other, "last_name")
    {
        out.push("legal_name_and_birth_date");
    }
    let email = normalized(input, "email");
    if !email.is_empty() && email == normalized(other, "email") {
        out.push("email_and_birth_date");
    }
    let number = phone(input);
    if number.len() >= 7 && number == phone(other) {
        out.push("phone_and_birth_date");
    }
    out
}
pub(crate) fn validate_demographics(input: &Value) -> Result<()> {
    for (key, max, required) in [
        ("first_name", 100, true),
        ("last_name", 100, true),
        ("preferred_name", 100, false),
        ("email", 254, false),
        ("phone", 50, false),
        ("emergency_contact", 1000, false),
        ("pronouns", 80, false),
        ("preferred_language", 80, false),
        ("address_line1", 200, false),
        ("address_line2", 200, false),
        ("city", 100, false),
        ("region", 100, false),
        ("postal_code", 30, false),
        ("country", 80, false),
    ] {
        if let Some(value) = input.get(key) {
            let value = value
                .as_str()
                .ok_or_else(|| ApiError::invalid(format!("{key} must be text")))?;
            if value.contains('\0')
                || value.chars().count() > max
                || (required && value.trim().is_empty())
            {
                return Err(ApiError::invalid(format!(
                    "{key} must contain {}–{max} characters",
                    if required { 1 } else { 0 }
                )));
            }
        }
    }
    Ok(())
}
fn intake_role(actor: &Actor) -> Result<()> {
    if matches!(
        actor.role.as_str(),
        "admin" | "clinical_lead" | "clinician" | "support"
    ) {
        Ok(())
    } else {
        Err(ApiError::denied())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MatchInput {
    first_name: String,
    last_name: String,
    birth_date: NaiveDate,
    #[serde(default)]
    email: String,
    #[serde(default)]
    phone: String,
    exclude_client_id: Option<Uuid>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DuplicateReview {
    matched_ids: Vec<Uuid>,
    reason: String,
}
pub(crate) fn take_review(input: &mut Value) -> Result<Option<DuplicateReview>> {
    let object = input
        .as_object_mut()
        .ok_or_else(|| ApiError::invalid("Expected a registration object"))?;
    object
        .remove("_duplicate_review")
        .map(|v| {
            serde_json::from_value(v).map_err(|_| {
                ApiError::invalid(
                    "Provide matched record identifiers and a duplicate-review reason",
                )
            })
        })
        .transpose()
}
async fn candidates(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    input: &Value,
    exclude: Option<Uuid>,
) -> Result<Vec<Value>> {
    validate_demographics(input)?;
    records::text(input, "first_name")?;
    records::text(input, "last_name")?;
    let birth = NaiveDate::parse_from_str(records::text(input, "birth_date")?, "%Y-%m-%d")
        .map_err(|_| ApiError::invalid("Enter a valid birth date"))?;
    let rows:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'mrn',mrn,'first_name',first_name,'last_name',last_name,'preferred_name',preferred_name,'birth_date',birth_date,'email',email,'phone',phone,'facility_id',facility_id,'status',status,'primary_clinician_id',primary_clinician_id) FROM clients WHERE organization_id=$1 AND birth_date=$2 AND ($3::uuid IS NULL OR id<>$3) ORDER BY mrn LIMIT 1001").bind(actor.org).bind(birth).bind(exclude).fetch_all(&mut **tx).await?;
    if rows.len() > 1000 {
        return Err(ApiError::conflict("The duplicate search exceeds the current review limit. No partial match set was returned."));
    }
    Ok(rows
        .into_iter()
        .filter_map(|mut row| {
            let matching = reasons(input, &row);
            if matching.is_empty() {
                None
            } else {
                row["reasons"] = json!(matching);
                Some(row)
            }
        })
        .collect())
}
fn visible(actor: &Actor, row: &Value) -> bool {
    actor.role != "clinician" || row["primary_clinician_id"] == actor.id.to_string()
}
async fn find_matches(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<MatchInput>,
) -> Result<HttpResponse> {
    let actor = access::actor(&req, &state.pool).await?;
    let mut tx = state.pool.begin().await?;
    let actor = access::lock_mutation(&req, &mut tx, actor.org, "clients").await?;
    intake_role(&actor)?;
    if let Some(id) = body.exclude_client_id {
        let row = records::get(&mut tx, "clients", id, actor.org, false).await?;
        records::scope(&mut tx, &actor, "clients", &row).await?;
    }
    let input = json!({"first_name":body.first_name,"last_name":body.last_name,"birth_date":body.birth_date,"email":body.email,"phone":body.phone});
    let all = candidates(&mut tx, &actor, &input, body.exclude_client_id).await?;
    let restricted = all.iter().any(|r| !visible(&actor, r));
    let matches: Vec<Value> = all
        .into_iter()
        .filter(|r| visible(&actor, r))
        .map(|mut row| {
            let obj = row.as_object_mut().unwrap();
            for key in ["primary_clinician_id", "email", "phone"] {
                obj.remove(key);
            }
            row
        })
        .collect();
    access::audit(
        &mut tx,
        &actor,
        "check_duplicates",
        "clients",
        body.exclude_client_id,
    )
    .await?;
    tx.commit().await?;
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(json!({"matches":matches,"restricted_matches":restricted})))
}
pub(crate) async fn check(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    input: &Value,
    before: Option<&Value>,
    review: Option<&DuplicateReview>,
) -> Result<Vec<Uuid>> {
    intake_role(actor)?;
    let identity_changed = before.is_none_or(|old| {
        ["first_name", "last_name", "birth_date", "email", "phone"]
            .iter()
            .any(|key| input[*key] != old[*key])
    });
    if !identity_changed {
        if review.is_some() {
            return Err(ApiError::invalid(
                "No changed registration identifiers need duplicate review",
            ));
        }
        return Ok(vec![]);
    }
    let exclude = before.map(|v| records::uuid(v, "id")).transpose()?;
    let matching_input = json!({"first_name":input["first_name"],"last_name":input["last_name"],"birth_date":input["birth_date"],"email":input["email"].as_str().unwrap_or_default(),"phone":input["phone"].as_str().unwrap_or_default()});
    let all = candidates(tx, actor, &matching_input, exclude).await?;
    if all.iter().any(|row| !visible(actor, row)) {
        return Err(ApiError::conflict("A possible matching record requires review by the admissions team. Registration was not saved."));
    }
    let ids: Vec<Uuid> = all
        .iter()
        .map(|v| records::uuid(v, "id"))
        .collect::<Result<_>>()?;
    if ids.is_empty() {
        if review.is_some() {
            return Err(ApiError::conflict(
                "The possible-match set changed. Run the duplicate check again.",
            ));
        }
        return Ok(ids);
    }
    let Some(review) = review else {
        return Err(ApiError::conflict(
            "Possible matching records exist. Review them before saving a separate patient record.",
        ));
    };
    if review.reason.contains('\0') || !(10..=2000).contains(&review.reason.trim().chars().count())
    {
        return Err(ApiError::invalid(
            "Explain the separate-record decision in 10–2000 characters",
        ));
    }
    let saved: HashSet<_> = review.matched_ids.iter().copied().collect();
    let current: HashSet<_> = ids.iter().copied().collect();
    if saved.len() != review.matched_ids.len() || saved != current {
        return Err(ApiError::conflict(
            "The possible-match set changed. Check current records before saving.",
        ));
    }
    Ok(ids)
}
pub(crate) async fn record_review(
    tx: &mut Transaction<'_, Postgres>,
    actor: &Actor,
    client: Uuid,
    ids: &[Uuid],
    review: Option<&DuplicateReview>,
) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let review = review.ok_or_else(|| ApiError::invalid("Missing duplicate review"))?;
    sqlx::query("INSERT INTO patient_duplicate_reviews(id,organization_id,client_id,actor_id,candidate_ids,reason) VALUES($1,$2,$3,$4,$5,$6)").bind(Uuid::new_v4()).bind(actor.org).bind(client).bind(actor.id).bind(json!(ids)).bind(review.reason.trim()).execute(&mut **tx).await?;
    access::audit(
        tx,
        actor,
        "review_possible_duplicates",
        "clients",
        Some(client),
    )
    .await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_matching_requires_birth_date_and_handles_formatting_without_fuzzy_merges() {
        let a = json!({"first_name":"  Ana  Maria ","last_name":"O'Neil","birth_date":"1990-01-01","email":"ANA@example.invalid","phone":"(212) 555-0100"});
        let mut b = json!({"first_name":"ana maria","last_name":"o'neil","birth_date":"1990-01-01","email":"ana@example.invalid","phone":"2125550100"});
        assert_eq!(reasons(&a, &b).len(), 3);
        b["birth_date"] = json!("1990-01-02");
        assert!(reasons(&a, &b).is_empty());
        let c = json!({"first_name":"Alex","last_name":"Other","birth_date":"1990-01-01","phone":"123"});
        assert!(reasons(&a, &c).is_empty());
    }
    #[test]
    fn identity_check_fingerprint_changes_only_with_identifying_fields() {
        let a = json!({"first_name":"Alex","last_name":"Sample","birth_date":"1990-01-01","mrn":"NX0000000001"});
        let mut b = a.clone();
        b["city"] = json!("New city");
        assert_eq!(fingerprint(&a), fingerprint(&b));
        b["birth_date"] = json!("1991-01-01");
        assert_ne!(fingerprint(&a), fingerprint(&b));
    }
}
