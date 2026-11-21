//! Superbills: an itemized statement of one billed session that a client can keep or
//! send to their own payer.
//!
//! A superbill is not stored. It is assembled from the claim and the records the claim is
//! linked to, on request, so it can never drift from them. It lists only care that was
//! delivered and charges that passed the readiness checks: the session must be completed
//! and the claim past draft and not voided. Money is integer cents throughout, and the
//! balance is worked out here, on the server, from the payment ledger and its reversals.
//!
//! It puts demographics, a diagnosis and a charge on one page, so only the roles that
//! prepare claims may open one, and every opening is audited.
use crate::{
    access::{self, Actor},
    records, ApiError, AppState, Result,
};
use actix_web::{web, HttpRequest, HttpResponse};
use serde_json::{json, Value};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// What a payer usually expects on a superbill that Nexus does not hold yet. Stated on the
/// document, so nobody takes its absence for an oversight in their own records.
const NOT_ON_FILE: [&str; 5] = [
    "Rendering provider NPI and license",
    "Practice tax ID and billing address",
    "Diagnosis description",
    "Place of service code",
    "Procedure modifiers",
];

/// One entry in a claim's payment ledger, reduced to what the totals need.
#[derive(Clone, Copy, Debug)]
struct Entry {
    amount_cents: i64,
    adjustment: bool,
    reversed: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct Totals {
    charge_cents: i64,
    received_cents: i64,
    adjustment_cents: i64,
    balance_cents: i64,
}

/// A reversed entry counts for nothing: it is in the ledger and in the reversal table.
fn totals(charge_cents: i64, ledger: &[Entry]) -> Option<Totals> {
    let mut received = 0_i64;
    let mut adjustment = 0_i64;
    for entry in ledger.iter().filter(|entry| !entry.reversed) {
        if entry.adjustment {
            adjustment = adjustment.checked_add(entry.amount_cents)?;
        } else {
            received = received.checked_add(entry.amount_cents)?;
        }
    }
    Some(Totals {
        charge_cents,
        received_cents: received,
        adjustment_cents: adjustment,
        balance_cents: charge_cents
            .checked_sub(received)?
            .checked_sub(adjustment)?,
    })
}

fn text(row: &Value, key: &str) -> String {
    row[key].as_str().unwrap_or_default().trim().to_owned()
}

async fn build(tx: &mut Transaction<'_, Postgres>, actor: &Actor, claim: &Value) -> Result<Value> {
    let claim_id = records::uuid(claim, "id")?;
    match claim["status"].as_str() {
        Some("draft") => {
            return Err(ApiError::conflict(
                "Mark this claim ready first. A superbill lists charges that passed the readiness checks.",
            ))
        }
        Some("voided") => {
            return Err(ApiError::conflict(
                "This claim was voided. A superbill cannot be made from it.",
            ))
        }
        _ => {}
    }
    let session = records::get(
        tx,
        "sessions",
        records::uuid(claim, "session_id")?,
        actor.org,
        false,
    )
    .await?;
    if session["status"] != "completed" {
        return Err(ApiError::conflict(
            "A superbill lists care that was delivered. This session is not marked completed.",
        ));
    }
    let client = records::get(
        tx,
        "clients",
        records::uuid(claim, "client_id")?,
        actor.org,
        false,
    )
    .await?;
    let policy = records::get(
        tx,
        "policies",
        records::uuid(claim, "policy_id")?,
        actor.org,
        false,
    )
    .await?;
    let authorization = match claim["authorization_id"].as_str() {
        Some(id) => Some(
            records::get(
                tx,
                "authorizations",
                Uuid::parse_str(id).map_err(|_| ApiError::missing())?,
                actor.org,
                false,
            )
            .await?,
        ),
        None => None,
    };
    // Where and when, in the facility's own time, and who delivered it.
    let visit: Value = sqlx::query_scalar(
        "SELECT jsonb_build_object(
            'facility', f.name,
            'time_zone', f.timezone,
            'organization', o.name,
            'provider', s.name,
            'provider_role', s.role,
            'service_date', to_char(v.starts_at AT TIME ZONE f.timezone, 'YYYY-MM-DD'),
            'start_time', to_char(v.starts_at AT TIME ZONE f.timezone, 'HH24:MI'),
            'end_time', to_char(v.ends_at AT TIME ZONE f.timezone, 'HH24:MI'),
            'minutes', (extract(epoch FROM v.ends_at - v.starts_at) / 60)::bigint)
         FROM sessions v
         JOIN episodes e ON e.id=v.episode_id AND e.organization_id=v.organization_id
         JOIN facilities f ON f.id=e.facility_id AND f.organization_id=v.organization_id
         JOIN organizations o ON o.id=v.organization_id
         JOIN staff s ON s.id=v.clinician_id AND s.organization_id=v.organization_id
         WHERE v.id=$1 AND v.organization_id=$2",
    )
    .bind(records::uuid(&session, "id")?)
    .bind(actor.org)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(ApiError::missing)?;
    let ledger: Vec<(String, String, String, i64, bool)> = sqlx::query_as(
        "SELECT p.received_on::text, p.source, p.reference, p.amount_cents,
                EXISTS(SELECT 1 FROM payment_reversals r WHERE r.payment_id=p.id AND r.organization_id=p.organization_id)
         FROM payments p WHERE p.claim_id=$1 AND p.organization_id=$2 ORDER BY p.received_on, p.created_at, p.id",
    )
    .bind(claim_id)
    .bind(actor.org)
    .fetch_all(&mut **tx)
    .await?;
    let entries: Vec<Entry> = ledger
        .iter()
        .map(|(_, source, _, amount_cents, reversed)| Entry {
            amount_cents: *amount_cents,
            adjustment: source == "adjustment",
            reversed: *reversed,
        })
        .collect();
    let charge = claim["amount_cents"]
        .as_i64()
        .ok_or_else(|| ApiError::conflict("This claim has no charge amount."))?;
    let sums = totals(charge, &entries)
        .ok_or_else(|| ApiError::conflict("This claim's ledger could not be totalled."))?;
    // The service's name as it was saved with the session, when the code still matches.
    let snapshot = &session["service_snapshot"];
    let service_name =
        if text(snapshot, "service_code").eq_ignore_ascii_case(&text(claim, "service_code")) {
            text(snapshot, "name")
        } else {
            String::new()
        };
    let reference = claim_id.simple().to_string()[..8].to_uppercase();
    let locality = [
        text(&client, "city"),
        text(&client, "region"),
        text(&client, "postal_code"),
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(", ");
    let address: Vec<String> = [
        text(&client, "address_line1"),
        text(&client, "address_line2"),
        locality,
    ]
    .into_iter()
    .filter(|line| !line.is_empty())
    .collect();
    let authorization_number = authorization
        .as_ref()
        .map(|row| row["authorization_number"].clone());
    let payments: Vec<Value> = ledger
        .iter()
        .map(|(received_on, source, reference, amount_cents, reversed)| {
            json!({
                "received_on": received_on,
                "source": source,
                "reference": reference,
                "amount_cents": amount_cents,
                "reversed": reversed,
            })
        })
        .collect();
    Ok(json!({
        "number": format!("SB-{reference}"),
        "generated_by": text(&actor.value, "name"),
        "organization": visit["organization"],
        "facility": visit["facility"],
        "time_zone": visit["time_zone"],
        "client": {
            "id": client["id"],
            "name": format!("{} {}", text(&client, "first_name"), text(&client, "last_name")).trim(),
            "birth_date": client["birth_date"],
            "mrn": client["mrn"],
            "address": address,
        },
        "provider": { "name": visit["provider"], "role": visit["provider_role"] },
        "session": {
            "id": session["id"],
            "kind": session["kind"],
            "location": session["location"],
            "service_date": visit["service_date"],
            "start_time": visit["start_time"],
            "end_time": visit["end_time"],
            "minutes": visit["minutes"],
        },
        "service": {
            "code": claim["service_code"],
            "name": service_name,
            "units": claim["units"],
            "charge_cents": charge,
        },
        "diagnosis_code": claim["diagnosis_code"],
        "coverage": {
            "payer_name": policy["payer_name"],
            "member_id": policy["member_id"],
            "group_number": policy["group_number"],
            "authorization_number": authorization_number,
        },
        "claim": {
            "id": claim["id"],
            "status": claim["status"],
            "external_reference": claim["external_reference"],
        },
        "payments": payments,
        "totals": {
            "charge_cents": sums.charge_cents,
            "received_cents": sums.received_cents,
            "adjustment_cents": sums.adjustment_cents,
            "balance_cents": sums.balance_cents,
        },
        "not_on_file": NOT_ON_FILE,
    }))
}

/// `GET /api/claims/{id}/superbill`
pub async fn show(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let id = path.into_inner();
    let actor = access::actor(&req, &state.pool).await?;
    // Like the bulk exports: the roles that prepare claims, and no temporary exception
    // widens it. Reading claims does not by itself allow issuing a document from one.
    if !matches!(actor.role.as_str(), "admin" | "billing") {
        return Err(ApiError::denied());
    }
    let mut tx = state.pool.begin().await?;
    access::require(&mut *tx, &actor, "claims", false).await?;
    access::require(&mut *tx, &actor, "clients", false).await?;
    let claim = records::get(&mut tx, "claims", id, actor.org, false).await?;
    let statement = build(&mut tx, &actor, &claim).await?;
    access::audit(&mut tx, &actor, "superbill", "claims", Some(id)).await?;
    tx.commit().await?;
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store, private"))
        .json(statement))
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn paid(amount_cents: i64) -> Entry {
        Entry {
            amount_cents,
            adjustment: false,
            reversed: false,
        }
    }
    const fn adjusted(amount_cents: i64) -> Entry {
        Entry {
            amount_cents,
            adjustment: true,
            reversed: false,
        }
    }

    #[test]
    fn the_balance_is_the_charge_less_what_was_received_and_written_off() {
        assert_eq!(
            totals(18_000, &[]),
            Some(Totals {
                charge_cents: 18_000,
                received_cents: 0,
                adjustment_cents: 0,
                balance_cents: 18_000
            })
        );
        assert_eq!(
            totals(18_000, &[paid(12_000), adjusted(3_500), paid(2_500)]),
            Some(Totals {
                charge_cents: 18_000,
                received_cents: 14_500,
                adjustment_cents: 3_500,
                balance_cents: 0
            })
        );
    }

    #[test]
    fn a_reversed_entry_is_listed_but_never_counted() {
        let reversed = Entry {
            amount_cents: 12_000,
            adjustment: false,
            reversed: true,
        };
        let written_off_in_error = Entry {
            amount_cents: 6_000,
            adjustment: true,
            reversed: true,
        };
        assert_eq!(
            totals(18_000, &[reversed, written_off_in_error, paid(5_000)]),
            Some(Totals {
                charge_cents: 18_000,
                received_cents: 5_000,
                adjustment_cents: 0,
                balance_cents: 13_000
            })
        );
    }

    #[test]
    fn totals_stay_in_whole_cents_and_refuse_to_overflow() {
        // No floating point anywhere: one cent survives.
        assert_eq!(totals(1, &[]).unwrap().balance_cents, 1);
        assert_eq!(
            totals(10_001, &[paid(3_334), paid(3_334), paid(3_333)])
                .unwrap()
                .balance_cents,
            0
        );
        assert_eq!(totals(1, &[paid(i64::MAX), paid(1)]), None);
        assert_eq!(totals(i64::MIN, &[paid(1)]), None);
    }
}
