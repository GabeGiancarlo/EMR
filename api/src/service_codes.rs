//! Service-code equality is defined once in PostgreSQL, including Unicode casing.
use crate::{ApiError, Result};
use sqlx::{PgConnection, Postgres, Transaction};

pub(crate) const IDENTITY_MIGRATION: i64 = 202611040021;
const BOUNDARY_WHITESPACE: &str = "\u{0009}\u{000A}\u{000B}\u{000C}\u{000D}\u{0020}\u{0085}\u{00A0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200A}\u{2028}\u{2029}\u{202F}\u{205F}\u{3000}";

pub(crate) async fn canonical(
    tx: &mut Transaction<'_, Postgres>,
    value: &str,
) -> Result<Option<String>> {
    Ok(sqlx::query_scalar("SELECT nexus_service_code_v1($1)")
        .bind(value)
        .fetch_one(&mut **tx)
        .await?)
}

pub(crate) async fn same(
    tx: &mut Transaction<'_, Postgres>,
    left: Option<&str>,
    right: Option<&str>,
) -> Result<bool> {
    let (Some(left), Some(right)) = (left, right) else {
        return Ok(false);
    };
    Ok(sqlx::query_scalar("SELECT COALESCE((nexus_service_code_v1($1) COLLATE \"C\") = (nexus_service_code_v1($2) COLLATE \"C\"), false)")
        .bind(left).bind(right).fetch_one(&mut **tx).await?)
}

/// Runs only before migration 021, inside the migration transaction and lock.
/// Table locks also prevent old application writers changing the inspected codes
/// between compatibility checks and installing the new identity indexes.
pub(crate) async fn preflight(conn: &mut PgConnection) -> Result<()> {
    let sources = [
        ("authorizations", "to_jsonb(t)->>'service_code'"),
        ("claims", "to_jsonb(t)->>'service_code'"),
        ("groups", "to_jsonb(t)->'service_snapshot'->>'service_code'"),
        ("program_services", "to_jsonb(t)->>'service_code'"),
        (
            "sessions",
            "to_jsonb(t)->'service_snapshot'->>'service_code'",
        ),
    ];
    let mut present = Vec::new();
    for (table, expression) in sources {
        let exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
            .bind(table)
            .fetch_one(&mut *conn)
            .await?;
        if exists {
            present.push((table, expression));
        }
    }
    if present.is_empty() {
        return Ok(());
    }
    // Identifiers come exclusively from the fixed allowlist above.
    let tables = present
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(", ");
    sqlx::query(&format!("LOCK TABLE {tables} IN SHARE MODE"))
        .execute(&mut *conn)
        .await?;
    for (table, expression) in present {
        let rows: Vec<(String, String)> = sqlx::query_as(&format!(
            "SELECT id::text, {expression} FROM {table} t WHERE {expression} IS NOT NULL ORDER BY id"))
            .fetch_all(&mut *conn).await?;
        for batch in rows.chunks(1000) {
            let raw: Vec<String> = batch.iter().map(|(_, value)| value.clone()).collect();
            // The proposed operation is evaluated directly: its named function
            // does not exist until migration 021. Preserve order explicitly.
            let database: Vec<String> = sqlx::query_scalar(
                "SELECT upper(btrim(value, $1) COLLATE pg_catalog.\"unicode\") FROM unnest($2::text[]) WITH ORDINALITY AS source(value, position) ORDER BY position")
                .bind(BOUNDARY_WHITESPACE).bind(&raw).fetch_all(&mut *conn).await?;
            for ((id, value), canonical) in batch.iter().zip(database) {
                if value.trim().to_uppercase() != canonical {
                    return Err(ApiError::conflict(format!(
                        "Migration 021 stopped: historical Rust and ICU service-code identity differ for {table}:{id}. No rows were rewritten; review Unicode compatibility before upgrading.")));
                }
            }
        }
    }
    Ok(())
}
