//! Audited staff time-away reservations. No patient information belongs in reasons.
use crate::{
    access::{self, Actor},
    scheduling, ApiError, AppState, Result,
};
use actix_web::{web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Create {
    staff_id: Uuid,
    starts_at: String,
    ends_at: String,
    reason: String,
    active: Option<bool>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    starts_at: Option<String>,
    ends_at: Option<String>,
    reason: Option<String>,
    active: Option<bool>,
    _expected_updated_at: Option<String>,
}
fn manages(actor: &Actor, staff: Uuid) -> bool {
    matches!(actor.role.as_str(), "admin" | "clinical_lead" | "support")
        || (actor.role == "clinician" && actor.id == staff)
}
async fn validate(tx: &mut Transaction<'_, Postgres>, actor: &Actor, value: &Value) -> Result<()> {
    let staff = crate::records::uuid(value, "staff_id")?;
    if !manages(actor, staff) {
        return Err(ApiError::denied());
    }
    let person = crate::records::get(tx, "staff", staff, actor.org, true).await?;
    if value["active"] != false
        && (person["active"] != true
            || !matches!(
                person["role"].as_str(),
                Some("admin" | "clinical_lead" | "clinician")
            ))
    {
        return Err(ApiError::conflict("Choose an active clinical team member"));
    }
    let reason = value["reason"].as_str().unwrap_or_default().trim();
    if reason.is_empty() || reason.chars().count() > 200 {
        return Err(ApiError::invalid(
            "Enter a scheduling reason of 1 to 200 characters",
        ));
    }
    let (start, end) = scheduling::interval(value)?;
    if value["active"] == false {
        return Ok(());
    }
    let own = value["id"].as_str().and_then(|id| Uuid::parse_str(id).ok());
    let overlap:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM staff_unavailability WHERE organization_id=$1 AND staff_id=$2 AND active AND starts_at<$4 AND ends_at>$3 AND ($5::uuid IS NULL OR id<>$5)) OR EXISTS(SELECT 1 FROM sessions WHERE organization_id=$1 AND clinician_id=$2 AND group_id IS NULL AND status IN ('scheduled','completed') AND starts_at<$4 AND ends_at>$3) OR EXISTS(SELECT 1 FROM groups WHERE organization_id=$1 AND clinician_id=$2 AND status IN ('scheduled','completed') AND starts_at<$4 AND ends_at>$3)")
        .bind(actor.org).bind(staff).bind(start).bind(end).bind(own).fetch_one(&mut **tx).await?;
    if overlap {
        return Err(ApiError::conflict("This time overlaps existing care or another active availability block. Resolve the conflict before saving."));
    }
    Ok(())
}
pub async fn list(req: HttpRequest, state: web::Data<AppState>) -> Result<HttpResponse> {
    let actor = access::actor(&req, &state.pool).await?;
    access::require(&state.pool, &actor, "sessions", false).await?;
    let mut tx = state.pool.begin().await?;
    let mut rows:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(b) FROM staff_unavailability b WHERE organization_id=$1 ORDER BY starts_at")
        .bind(actor.org).fetch_all(&mut *tx).await?;
    for row in &mut rows {
        if !manages(&actor, crate::records::uuid(row, "staff_id")?) {
            row["reason"] = json!("Unavailable");
        }
    }
    access::audit(&mut tx, &actor, "read", "staff_unavailability", None).await?;
    tx.commit().await?;
    Ok(HttpResponse::Ok().json(rows))
}
pub async fn create(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<Create>,
) -> Result<HttpResponse> {
    let actor = access::actor(&req, &state.pool).await?;
    let mut tx = state.pool.begin().await?;
    let actor = access::lock_mutation(&req, &mut tx, actor.org, "sessions").await?;
    let candidate = json!({"staff_id":body.staff_id,"starts_at":body.starts_at,"ends_at":body.ends_at,"reason":body.reason.trim(),"active":body.active.unwrap_or(true)});
    validate(&mut tx, &actor, &candidate).await?;
    let (start, end) = scheduling::interval(&candidate)?;
    let id = Uuid::new_v4();
    let row:Value=sqlx::query_scalar("INSERT INTO staff_unavailability(id,organization_id,staff_id,starts_at,ends_at,reason,active,created_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8) RETURNING to_jsonb(staff_unavailability)")
        .bind(id).bind(actor.org).bind(body.staff_id).bind(start).bind(end).bind(body.reason.trim()).bind(body.active.unwrap_or(true)).bind(actor.id).fetch_one(&mut *tx).await?;
    access::audit(&mut tx, &actor, "create", "staff_unavailability", Some(id)).await?;
    tx.commit().await?;
    Ok(HttpResponse::Created().json(row))
}
pub async fn update(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<Patch>,
) -> Result<HttpResponse> {
    let actor = access::actor(&req, &state.pool).await?;
    let mut tx = state.pool.begin().await?;
    let actor = access::lock_mutation(&req, &mut tx, actor.org, "sessions").await?;
    let id = path.into_inner();
    let mut row:Value=sqlx::query_scalar("SELECT to_jsonb(b) FROM staff_unavailability b WHERE id=$1 AND organization_id=$2 FOR UPDATE")
        .bind(id).bind(actor.org).fetch_optional(&mut *tx).await?.ok_or_else(ApiError::missing)?;
    if !manages(&actor, crate::records::uuid(&row, "staff_id")?) {
        return Err(ApiError::denied());
    }
    let expected=body._expected_updated_at.as_deref().filter(|s|!s.trim().is_empty()).ok_or_else(||ApiError::precondition("Reload this availability block before saving; the originally loaded updated_at is required."))?;
    let expected = DateTime::parse_from_rfc3339(expected).map_err(|_| {
        ApiError::invalid("The original updated_at must be a timestamp with timezone")
    })?;
    let actual = DateTime::parse_from_rfc3339(row["updated_at"].as_str().unwrap_or_default())
        .map_err(|_| ApiError::conflict("Reload this availability block before saving"))?;
    if expected != actual {
        return Err(ApiError::conflict("This availability block changed after you opened it. Keep your unsaved edits and reload the latest version."));
    }
    if let Some(start) = &body.starts_at {
        row["starts_at"] = json!(start);
    }
    if let Some(end) = &body.ends_at {
        row["ends_at"] = json!(end);
    }
    if let Some(reason) = &body.reason {
        row["reason"] = json!(reason.trim());
    }
    if let Some(active) = body.active {
        row["active"] = json!(active);
    }
    validate(&mut tx, &actor, &row).await?;
    let (start, end) = scheduling::interval(&row)?;
    let updated:Value=sqlx::query_scalar("UPDATE staff_unavailability SET starts_at=$3,ends_at=$4,reason=$5,active=$6,updated_at=$7 WHERE id=$1 AND organization_id=$2 RETURNING to_jsonb(staff_unavailability)")
        .bind(id).bind(actor.org).bind(start).bind(end).bind(row["reason"].as_str().unwrap_or_default()).bind(row["active"].as_bool().unwrap_or(true)).bind(Utc::now()).fetch_one(&mut *tx).await?;
    access::audit(&mut tx, &actor, "update", "staff_unavailability", Some(id)).await?;
    tx.commit().await?;
    Ok(HttpResponse::Ok().json(updated))
}
