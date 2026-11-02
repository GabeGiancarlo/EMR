//! Adult outpatient/IOP/PHP product policy. February 29 maps to March 1
//! in non-leap years; this is not a statement of jurisdiction-specific law.
use chrono::{Datelike, NaiveDate};

pub fn adult_anniversary(birth: NaiveDate) -> Option<NaiveDate> {
    let year = birth.year().checked_add(18)?;
    birth.with_year(year).or_else(|| {
        (birth.month() == 2 && birth.day() == 29)
            .then(|| NaiveDate::from_ymd_opt(year, 3, 1))
            .flatten()
    })
}
pub fn has_completed_adult_years(birth: NaiveDate, today: NaiveDate) -> bool {
    adult_anniversary(birth).is_some_and(|anniversary| today >= anniversary)
}
pub fn ineligible_message(field: &str) -> String {
    format!("{field}: this program accepts adults aged 18 or older as of the facility's local date. Check the date of birth. February 29 birthdays become eligible on March 1 in a non-leap year.")
}
#[cfg(test)]
mod tests {
    use super::*;
    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }
    #[test]
    fn birthday_boundary_uses_calendar_years() {
        assert!(!has_completed_adult_years(d(2008, 6, 15), d(2026, 6, 14)));
        assert!(has_completed_adult_years(d(2008, 6, 15), d(2026, 6, 15)));
        assert!(has_completed_adult_years(d(2008, 6, 15), d(2026, 6, 16)));
        assert!(!has_completed_adult_years(d(2004, 1, 1), d(2021, 12, 31)));
    }
    #[test]
    fn leap_birthday_becomes_eligible_on_march_first() {
        assert_eq!(adult_anniversary(d(2004, 2, 29)), Some(d(2022, 3, 1)));
        assert!(!has_completed_adult_years(d(2004, 2, 29), d(2022, 2, 28)));
        assert!(has_completed_adult_years(d(2004, 2, 29), d(2022, 3, 1)));
    }
}
