//! Explicit local downloads for financial reconciliation and internal claim review.
//! Exports never transmit data, submit claims, or change payment/claim state.

use crate::{access, ApiError, AppState, Result};
use actix_web::{http::header, web, HttpRequest, HttpResponse};
use chrono::{Days, NaiveDate, Utc};
use serde::Deserialize;
use sqlx::FromRow;
use uuid::Uuid;

const MAX_WINDOW_DAYS: i64 = 366;
const MAX_CLAIMS: usize = 10_000;
const ACCOUNTING_HEADER: &[&str] = &[
    "received_on",
    "source",
    "entry_count",
    "cash_received_cents",
    "adjustment_cents",
];
const CLAIMS_HEADER: &[&str] = &[
    "export_purpose",
    "claim_id",
    "client_id",
    "policy_id",
    "session_id",
    "authorization_id",
    "service_code",
    "diagnosis_code",
    "units",
    "amount_cents",
    "status",
    "external_reference",
    "denial_reason",
    "void_reason",
    "created_at_utc",
    "updated_at_utc",
];

// Only payment events and their source classification enter this aggregate.
// References, reversal reasons, actors, claims, and client records are not projected.
const ACCOUNTING_SQL: &str = "WITH entries AS (\
    SELECT received_on,source,amount_cents FROM payments \
    WHERE organization_id=$1 AND received_on >= $2 AND received_on <= $3 \
    UNION ALL \
    SELECT r.reversed_on AS received_on,p.source,-r.amount_cents AS amount_cents \
    FROM payment_reversals r JOIN payments p ON p.id=r.payment_id AND p.organization_id=r.organization_id \
    WHERE r.organization_id=$1 AND r.reversed_on >= $2 AND r.reversed_on <= $3) \
    SELECT received_on, source, count(*)::bigint AS entry_count, \
    sum(CASE WHEN source IN ('payer','client') THEN amount_cents ELSE 0 END)::bigint AS cash_received_cents, \
    sum(CASE WHEN source='adjustment' THEN amount_cents ELSE 0 END)::bigint AS adjustment_cents \
    FROM entries \
    GROUP BY received_on,source ORDER BY received_on,source";

/// Inclusive business-date filter. Supplying only one bound is invalid.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportWindow {
    from: Option<String>,
    to: Option<String>,
}

fn parse_date(value: &str) -> Result<NaiveDate> {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || value.starts_with("0000")
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(i, b)| !matches!(i, 4 | 7) && !b.is_ascii_digit())
    {
        return Err(ApiError::invalid("Export dates must use YYYY-MM-DD."));
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| ApiError::invalid("Export date is not a valid calendar date."))
}

fn window(
    query: &ExportWindow,
    require_bounds: bool,
    today: NaiveDate,
) -> Result<(NaiveDate, NaiveDate)> {
    let dates = match (&query.from, &query.to) {
        (Some(from), Some(to)) => (parse_date(from)?, parse_date(to)?),
        (None, None) if !require_bounds => (
            today
                .checked_sub_days(Days::new(29))
                .ok_or_else(|| ApiError::invalid("Export date is out of range."))?,
            today,
        ),
        _ => {
            return Err(ApiError::invalid(
                "Supply both from and to dates for this export.",
            ))
        }
    };
    let span = dates.1.signed_duration_since(dates.0).num_days();
    if !(0..MAX_WINDOW_DAYS).contains(&span) {
        return Err(ApiError::invalid(
            "Export dates must be ordered and span no more than 366 inclusive days.",
        ));
    }
    Ok(dates)
}

/// Always quote RFC 4180 fields and neutralize spreadsheet formula prefixes.
/// Checking after Unicode whitespace/BOM prevents leading-space formula bypasses.
fn csv_field(value: &str) -> String {
    let value = value.replace('\0', "");
    let leading = value.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    let dangerous =
        leading.starts_with(['=', '+', '-', '@']) || value.starts_with(['\t', '\r', '\n']);
    format!(
        "\"{}{}\"",
        if dangerous { "'" } else { "" },
        value.replace('"', "\"\"")
    )
}

fn csv_row<I, S>(output: &mut String, fields: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let cells: Vec<String> = fields.into_iter().map(|x| csv_field(x.as_ref())).collect();
    output.push_str(&cells.join(","));
    output.push_str("\r\n");
}

fn response(csv: String, name: &str) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, "text/csv; charset=utf-8"))
        .insert_header((
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{name}\""),
        ))
        .insert_header((header::CACHE_CONTROL, "no-store, private"))
        .insert_header((header::PRAGMA, "no-cache"))
        .insert_header((
            header::HeaderName::from_static("x-content-type-options"),
            "nosniff",
        ))
        .body(csv)
}

#[derive(Debug, FromRow)]
struct AccountingRow {
    received_on: NaiveDate,
    source: String,
    entry_count: i64,
    cash_received_cents: i64,
    adjustment_cents: i64,
}

fn accounting_csv(rows: &[AccountingRow]) -> String {
    let mut csv = String::new();
    csv_row(&mut csv, ACCOUNTING_HEADER.iter());
    for row in rows {
        // These three fields are typed i64 values, not untrusted strings. Preserve
        // negative reversal totals as numeric cells rather than apostrophe text.
        csv.push_str(&format!(
            "{},{},\"{}\",\"{}\",\"{}\"\r\n",
            csv_field(&row.received_on.to_string()),
            csv_field(&row.source),
            row.entry_count,
            row.cash_received_cents,
            row.adjustment_cents,
        ));
    }
    csv
}

/// Daily recorded payment totals, with adjustments separated from cash.
pub async fn accounting(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<ExportWindow>,
) -> Result<HttpResponse> {
    let actor = access::actor(&req, &state.pool).await?;
    if !matches!(actor.role.as_str(), "admin" | "billing" | "executive") {
        return Err(ApiError::denied());
    }
    access::require(&state.pool, &actor, "payments", false).await?;
    let (from, to) = window(&query, true, Utc::now().date_naive())?;
    let mut tx = state.pool.begin().await?;
    let rows: Vec<AccountingRow> = sqlx::query_as(ACCOUNTING_SQL)
        .bind(actor.org)
        .bind(from)
        .bind(to)
        .fetch_all(&mut *tx)
        .await?;
    let csv = accounting_csv(&rows);
    access::audit(&mut tx, &actor, "export_accounting_csv", "payments", None).await?;
    tx.commit().await?;
    Ok(response(
        csv,
        &format!("nexus-accounting-{from}-to-{to}.csv"),
    ))
}

#[derive(FromRow)]
struct ClaimRow {
    id: Uuid,
    client_id: Uuid,
    policy_id: Uuid,
    session_id: Uuid,
    authorization_id: Option<Uuid>,
    service_code: String,
    diagnosis_code: String,
    units: i32,
    amount_cents: i64,
    status: String,
    external_reference: String,
    denial_reason: String,
    void_reason: String,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

/// An internal review extract, deliberately not a payer-ready X12 transaction.
pub async fn claims(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<ExportWindow>,
) -> Result<HttpResponse> {
    let actor = access::actor(&req, &state.pool).await?;
    // A temporary resource exception cannot broaden this bulk-export policy.
    if !matches!(actor.role.as_str(), "admin" | "billing") {
        return Err(ApiError::denied());
    }
    access::require(&state.pool, &actor, "claims", false).await?;
    let (from, to) = window(&query, false, Utc::now().date_naive())?;
    let start = from
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| ApiError::invalid("Export start is out of range."))?
        .and_utc();
    let end = to
        .checked_add_days(Days::new(1))
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .ok_or_else(|| ApiError::invalid("Export end is out of range."))?
        .and_utc();
    let mut tx = state.pool.begin().await?;
    let rows: Vec<ClaimRow> = sqlx::query_as("SELECT id,client_id,policy_id,session_id,authorization_id,service_code,diagnosis_code,units,amount_cents,status,external_reference,denial_reason,void_reason,created_at,updated_at FROM claims WHERE organization_id=$1 AND created_at >= $2 AND created_at < $3 ORDER BY created_at,id LIMIT $4")
        .bind(actor.org).bind(start).bind(end).bind((MAX_CLAIMS + 1) as i64).fetch_all(&mut *tx).await?;
    if rows.len() > MAX_CLAIMS {
        return Err(ApiError::invalid("More than 10000 claims match. Narrow the export dates; no partial export was generated."));
    }
    let mut csv = String::new();
    csv_row(&mut csv, CLAIMS_HEADER.iter());
    for row in rows {
        csv_row(
            &mut csv,
            [
                "internal_review_not_payer_submission".into(),
                row.id.to_string(),
                row.client_id.to_string(),
                row.policy_id.to_string(),
                row.session_id.to_string(),
                row.authorization_id
                    .map(|x| x.to_string())
                    .unwrap_or_default(),
                row.service_code,
                row.diagnosis_code,
                row.units.to_string(),
                row.amount_cents.to_string(),
                row.status,
                row.external_reference,
                row.denial_reason,
                row.void_reason,
                row.created_at.to_rfc3339(),
                row.updated_at.to_rfc3339(),
            ],
        );
    }
    access::audit(
        &mut tx,
        &actor,
        "export_claims_csv_internal_review",
        "claims",
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(response(
        csv,
        &format!("nexus-claims-internal-review-{from}-to-{to}.csv"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 2).expect("valid test date")
    }
    fn query(from: Option<&str>, to: Option<&str>) -> ExportWindow {
        ExportWindow {
            from: from.map(str::to_owned),
            to: to.map(str::to_owned),
        }
    }

    #[test]
    fn csv_escapes_delimiters_quotes_and_line_breaks() {
        assert_eq!(
            csv_field("Alpha, \"Beta\"\r\nGamma"),
            "\"Alpha, \"\"Beta\"\"\r\nGamma\""
        );
        assert_eq!(csv_field(""), "\"\"");
        assert_eq!(csv_field("A\0B"), "\"AB\"");
    }

    #[test]
    fn csv_neutralizes_spreadsheet_formulas_and_whitespace_bypasses() {
        for text in [
            "=1+1",
            "+cmd",
            "-1+2",
            "@SUM(A1)",
            "  =1+1",
            "\t=1",
            "\r\n=1",
            "\u{feff}=1",
            "\u{2003}+1",
            "\0=1",
        ] {
            assert!(csv_field(text).starts_with("\"'"), "unsafe field {text:?}");
        }
        assert_eq!(csv_field("payer"), "\"payer\"");
        assert_eq!(csv_field("2026-11-02"), "\"2026-11-02\"");
    }

    #[test]
    fn calendar_bounds_are_strict_and_inclusive() {
        for date in [
            "2026-2-02",
            "0000-01-01",
            "2026-02-30",
            "2026/11/02",
            " 2026-11-02",
            "2026-11-02T00:00:00Z",
        ] {
            assert!(parse_date(date).is_err());
        }
        assert!(window(
            &query(Some("2024-01-01"), Some("2024-12-31")),
            true,
            today()
        )
        .is_ok());
        assert!(window(
            &query(Some("2024-01-01"), Some("2025-01-01")),
            true,
            today()
        )
        .is_err());
        assert!(window(
            &query(Some("2026-11-02"), Some("2026-11-01")),
            true,
            today()
        )
        .is_err());
        assert!(window(
            &query(Some("2026-11-02"), Some("2026-11-02")),
            true,
            today()
        )
        .is_ok());
    }

    #[test]
    fn missing_bounds_and_default_are_explicit() {
        assert!(window(&query(None, None), true, today()).is_err());
        assert!(window(&query(Some("2026-11-01"), None), false, today()).is_err());
        let (from, to) = window(&query(None, None), false, today()).expect("default dates");
        assert_eq!(to, today());
        assert_eq!((to - from).num_days(), 29);
    }

    #[test]
    fn accounting_only_projects_aggregate_fields_and_separates_cash() {
        let rows = vec![
            AccountingRow {
                received_on: today(),
                source: "payer".into(),
                entry_count: 2,
                cash_received_cents: 12345,
                adjustment_cents: 0,
            },
            AccountingRow {
                received_on: today(),
                source: "adjustment".into(),
                entry_count: 1,
                cash_received_cents: 0,
                adjustment_cents: 500,
            },
            AccountingRow {
                received_on: today(),
                source: "payer".into(),
                entry_count: 1,
                cash_received_cents: -12345,
                adjustment_cents: 0,
            },
        ];
        let csv = accounting_csv(&rows);
        assert!(csv.contains("\"payer\",\"2\",\"12345\",\"0\""));
        assert!(csv.contains("\"adjustment\",\"1\",\"0\",\"500\""));
        assert!(csv.contains("\"payer\",\"1\",\"-12345\",\"0\""));
        assert!(!csv.contains("'-12345"));
        assert_eq!(ACCOUNTING_HEADER.len(), 5);
        for forbidden in [
            "client_id",
            "claim_id",
            "reference",
            "member_id",
            "email",
            "note",
            "JOIN clients",
            "reason",
        ] {
            assert!(!ACCOUNTING_SQL.contains(forbidden));
            assert!(!ACCOUNTING_HEADER.contains(&forbidden));
        }
        assert_eq!(accounting_csv(&[]).lines().count(), 1);
    }
}
