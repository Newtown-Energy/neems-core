//! US federal holiday computation for the peak-season wizard.
//!
//! The wizard's "weekdays Jun 24 – Sep 15 (excluding holidays)" flow runs
//! through [`us_federal_holidays_in_range`] to decide which dates to skip
//! when season-filling application rules.
//!
//! The set is defined declaratively below ([`FEDERAL_HOLIDAY_RULES`]) so
//! that adding, removing, or revising the list — for example to localize
//! to a different jurisdiction or to honor a site-specific shutdown — is a
//! single-array edit rather than a logic change.

use chrono::{Datelike, Duration, NaiveDate, Weekday};

/// A rule describing when a holiday falls in a given year.
#[derive(Debug, Clone)]
pub enum HolidayRule {
    /// Fixed month/day (e.g. Jul 4).
    FixedDate { month: u32, day: u32 },
    /// The nth occurrence of a weekday in a month (e.g. 3rd Monday of January =
    /// MLK Day). `nth` is 1-indexed; for the *last* weekday of a month use
    /// [`HolidayRule::LastWeekdayInMonth`].
    NthWeekdayInMonth { month: u32, weekday: Weekday, nth: u32 },
    /// The last occurrence of a weekday in a month (e.g. last Monday of May =
    /// Memorial Day).
    LastWeekdayInMonth { month: u32, weekday: Weekday },
}

/// Whether to roll a fixed-date holiday to the nearest weekday when it
/// falls on a weekend. Federal holidays follow this convention; sites that
/// want strict calendar dates instead can clone this list and omit the
/// `observed` flag for the affected entries.
#[derive(Debug, Clone)]
pub struct FederalHoliday {
    pub name: &'static str,
    pub rule: HolidayRule,
    pub observed: bool,
}

/// The default holiday set used by the peak-season wizard. Edit this list
/// to change which dates the wizard treats as holidays.
pub const FEDERAL_HOLIDAY_RULES: &[FederalHoliday] = &[
    FederalHoliday {
        name: "New Year's Day",
        rule: HolidayRule::FixedDate { month: 1, day: 1 },
        observed: true,
    },
    FederalHoliday {
        name: "Martin Luther King Jr. Day",
        rule: HolidayRule::NthWeekdayInMonth { month: 1, weekday: Weekday::Mon, nth: 3 },
        observed: false,
    },
    FederalHoliday {
        name: "Presidents Day",
        rule: HolidayRule::NthWeekdayInMonth { month: 2, weekday: Weekday::Mon, nth: 3 },
        observed: false,
    },
    FederalHoliday {
        name: "Memorial Day",
        rule: HolidayRule::LastWeekdayInMonth { month: 5, weekday: Weekday::Mon },
        observed: false,
    },
    FederalHoliday {
        name: "Juneteenth",
        rule: HolidayRule::FixedDate { month: 6, day: 19 },
        observed: true,
    },
    FederalHoliday {
        name: "Independence Day",
        rule: HolidayRule::FixedDate { month: 7, day: 4 },
        observed: true,
    },
    FederalHoliday {
        name: "Labor Day",
        rule: HolidayRule::NthWeekdayInMonth { month: 9, weekday: Weekday::Mon, nth: 1 },
        observed: false,
    },
    FederalHoliday {
        name: "Columbus Day",
        rule: HolidayRule::NthWeekdayInMonth { month: 10, weekday: Weekday::Mon, nth: 2 },
        observed: false,
    },
    FederalHoliday {
        name: "Veterans Day",
        rule: HolidayRule::FixedDate { month: 11, day: 11 },
        observed: true,
    },
    FederalHoliday {
        name: "Thanksgiving",
        rule: HolidayRule::NthWeekdayInMonth { month: 11, weekday: Weekday::Thu, nth: 4 },
        observed: false,
    },
    FederalHoliday {
        name: "Christmas Day",
        rule: HolidayRule::FixedDate { month: 12, day: 25 },
        observed: true,
    },
];

/// Resolve a [`HolidayRule`] to an actual date in `year`. Returns `None` if
/// the rule is malformed (e.g. asking for the 5th Monday of a month that
/// only has 4).
fn resolve(rule: &HolidayRule, year: i32) -> Option<NaiveDate> {
    match *rule {
        HolidayRule::FixedDate { month, day } => NaiveDate::from_ymd_opt(year, month, day),
        HolidayRule::NthWeekdayInMonth { month, weekday, nth } => {
            let first = NaiveDate::from_ymd_opt(year, month, 1)?;
            let offset_to_first_match = (7 + weekday.num_days_from_monday() as i64
                - first.weekday().num_days_from_monday() as i64)
                % 7;
            let nth_match = first + Duration::days(offset_to_first_match + 7 * (nth as i64 - 1));
            if nth_match.month() == month {
                Some(nth_match)
            } else {
                None
            }
        }
        HolidayRule::LastWeekdayInMonth { month, weekday } => {
            // Find the first day of the next month, walk backwards until we hit the target
            // weekday.
            let (next_y, next_m) = if month == 12 {
                (year + 1, 1)
            } else {
                (year, month + 1)
            };
            let first_of_next = NaiveDate::from_ymd_opt(next_y, next_m, 1)?;
            let mut d = first_of_next - Duration::days(1);
            while d.weekday() != weekday {
                d -= Duration::days(1);
            }
            Some(d)
        }
    }
}

/// If a fixed-date federal holiday falls on a weekend, federal observance
/// shifts it: Saturday → preceding Friday, Sunday → following Monday.
/// Weekday-anchored rules always fall on a weekday already and are
/// returned unchanged.
fn apply_federal_observance(actual: NaiveDate) -> NaiveDate {
    match actual.weekday() {
        Weekday::Sat => actual - Duration::days(1),
        Weekday::Sun => actual + Duration::days(1),
        _ => actual,
    }
}

/// All US federal holidays observed in `year`, sorted ascending.
pub fn us_federal_holidays(year: i32) -> Vec<NaiveDate> {
    let mut dates: Vec<NaiveDate> = FEDERAL_HOLIDAY_RULES
        .iter()
        .filter_map(|h| {
            resolve(&h.rule, year).map(|d| {
                if h.observed {
                    apply_federal_observance(d)
                } else {
                    d
                }
            })
        })
        .collect();
    dates.sort();
    dates.dedup();
    dates
}

/// All US federal holidays observed within `[start, end]` (both inclusive),
/// sorted ascending.
pub fn us_federal_holidays_in_range(start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
    let mut out = Vec::new();
    for year in start.year()..=end.year() {
        for h in us_federal_holidays(year) {
            if h >= start && h <= end {
                out.push(h);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_2026_holidays() {
        let h = us_federal_holidays(2026);
        // Independence Day 2026 is Saturday Jul 4 → observed Friday Jul 3.
        assert!(h.contains(&NaiveDate::from_ymd_opt(2026, 7, 3).unwrap()));
        assert!(!h.contains(&NaiveDate::from_ymd_opt(2026, 7, 4).unwrap()));
        // Christmas 2026 is Friday Dec 25 — no shift.
        assert!(h.contains(&NaiveDate::from_ymd_opt(2026, 12, 25).unwrap()));
        // MLK Day 2026: 3rd Monday of January = Jan 19.
        assert!(h.contains(&NaiveDate::from_ymd_opt(2026, 1, 19).unwrap()));
        // Memorial Day 2026: last Monday of May = May 25.
        assert!(h.contains(&NaiveDate::from_ymd_opt(2026, 5, 25).unwrap()));
        // Thanksgiving 2026: 4th Thursday of November = Nov 26.
        assert!(h.contains(&NaiveDate::from_ymd_opt(2026, 11, 26).unwrap()));
    }

    #[test]
    fn peak_season_window_2026() {
        // Jun 24 – Sep 15, 2026 should include Jul 3 (observed Jul 4)
        // and Sep 7 (Labor Day), nothing else.
        let h = us_federal_holidays_in_range(
            NaiveDate::from_ymd_opt(2026, 6, 24).unwrap(),
            NaiveDate::from_ymd_opt(2026, 9, 15).unwrap(),
        );
        assert_eq!(
            h,
            vec![
                NaiveDate::from_ymd_opt(2026, 7, 3).unwrap(),
                NaiveDate::from_ymd_opt(2026, 9, 7).unwrap(),
            ]
        );
    }

    #[test]
    fn weekend_observance_rules() {
        // Independence Day 2020 was Saturday Jul 4 → observed Friday Jul 3.
        let h = us_federal_holidays(2020);
        assert!(h.contains(&NaiveDate::from_ymd_opt(2020, 7, 3).unwrap()));
        // Christmas 2021 was Saturday Dec 25 → observed Friday Dec 24.
        let h = us_federal_holidays(2021);
        assert!(h.contains(&NaiveDate::from_ymd_opt(2021, 12, 24).unwrap()));
        // New Year's Day 2023 was Sunday Jan 1 → observed Monday Jan 2.
        let h = us_federal_holidays(2023);
        assert!(h.contains(&NaiveDate::from_ymd_opt(2023, 1, 2).unwrap()));
    }
}
