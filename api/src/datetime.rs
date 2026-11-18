//! Date and timestamp contracts shared by API inputs, claim exports and citation IDs.
use serde::{de::Error, Deserialize, Deserializer};
use time::{
    format_description::well_known::Rfc3339, macros::format_description, Date, OffsetDateTime,
};

pub(crate) fn parse_date(value: &str) -> Result<Date, time::error::Parse> {
    Date::parse(
        value,
        format_description!("[year]-[month padding:none]-[day padding:none]"),
    )
}

pub(crate) fn date_string<'de, D: Deserializer<'de>>(de: D) -> Result<Date, D::Error> {
    parse_date(&String::deserialize(de)?).map_err(D::Error::custom)
}

pub(crate) fn parse_timestamp(value: &str) -> Result<OffsetDateTime, &'static str> {
    if !matches!(value.as_bytes().get(10), Some(b'T' | b't' | b' ')) {
        return Err("expected a date and time separated by T or a space");
    }
    let parse =
        |text: &str| OffsetDateTime::parse(text, &Rfc3339).map_err(|_| "invalid RFC3339 timestamp");
    if value.get(17..19) == Some("60") {
        // PostgreSQL stores :60 as the start of the next minute. Compare the
        // same instant for scheduling and optimistic edits before persistence.
        let mut preceding = value.to_owned();
        preceding.replace_range(17..19, "59");
        let stamp = parse(&preceding)?;
        if stamp.nanosecond() != 0 {
            return Err("fractional leap second is outside PostgreSQL timestamp bounds");
        }
        return stamp
            .checked_add(time::SignedDuration::SECOND)
            .ok_or("timestamp out of range");
    }
    parse(value)
}

pub(crate) fn optional_timestamp<'de, D: Deserializer<'de>>(
    de: D,
) -> Result<Option<OffsetDateTime>, D::Error> {
    Option::<String>::deserialize(de)?
        .as_deref()
        .map(parse_timestamp)
        .transpose()
        .map_err(D::Error::custom)
}

// Fractions in existing API timestamps, knowledge hashes and claim CSVs use
// groups of three digits. Shortening a fraction would change the document ID.
fn timestamp_body(value: OffsetDateTime) -> String {
    let nanos = value.nanosecond();
    let fraction = if nanos == 0 {
        String::new()
    } else if nanos.is_multiple_of(1_000_000) {
        format!(".{:03}", nanos / 1_000_000)
    } else if nanos.is_multiple_of(1_000) {
        format!(".{:06}", nanos / 1_000)
    } else {
        format!(".{nanos:09}")
    };
    format!(
        "{}T{:02}:{:02}:{:02}{fraction}",
        value.date(),
        value.hour(),
        value.minute(),
        value.second()
    )
}

pub(crate) fn canonical_timestamp(value: OffsetDateTime) -> String {
    let minutes = value.offset().whole_minutes();
    let sign = if minutes < 0 { '-' } else { '+' };
    let offset = minutes.unsigned_abs();
    format!(
        "{}{sign}{:02}:{:02}",
        timestamp_body(value),
        offset / 60,
        offset % 60
    )
}

pub(crate) fn now_timestamp() -> String {
    format!("{}Z", timestamp_body(OffsetDateTime::now_utc()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_timestamp_text_for_existing_hashes_and_exports() {
        for (input, expected) in [
            ("2026-04-01T08:15:00Z", "2026-04-01T08:15:00+00:00"),
            ("2026-04-01T08:15:00.1Z", "2026-04-01T08:15:00.100+00:00"),
            (
                "2026-04-01T08:15:00.1234+05:30",
                "2026-04-01T08:15:00.123400+05:30",
            ),
            (
                "2026-04-01T08:15:00.000000001-04:00",
                "2026-04-01T08:15:00.000000001-04:00",
            ),
        ] {
            assert_eq!(
                canonical_timestamp(parse_timestamp(input).unwrap()),
                expected
            );
        }
    }

    #[test]
    fn leap_second_matches_the_postgresql_minute_boundary() {
        assert_eq!(
            parse_timestamp("2016-12-31T18:59:60.000-05:00").unwrap(),
            parse_timestamp("2017-01-01T00:00:00Z").unwrap()
        );
        assert!(parse_timestamp("2016-12-31T23:59:60.5Z").is_err());
    }

    #[test]
    fn validates_calendar_dates_and_timestamp_separators() {
        assert_eq!(
            parse_date("2024-2-29").unwrap(),
            time::macros::date!(2024 - 02 - 29)
        );
        assert!(parse_date("2026-02-29").is_err());
        assert!(parse_timestamp("2026-04-01X08:15:00Z").is_err());
        assert!(parse_timestamp("2026-04-01 08:15:00Z").is_ok());
        assert!(parse_timestamp("2026-04-01T08:15:00").is_err());
    }
}
