//! Calendar months and streaks of them.

use std::collections::BTreeSet;

use chrono::{Datelike, Duration, NaiveDate};

/// The first day of `day`'s month.
pub fn month_start(day: NaiveDate) -> NaiveDate {
    day.with_day(1).expect("every month has a first day")
}

/// The first day of the month before `day`'s.
fn previous_month(day: NaiveDate) -> NaiveDate {
    month_start(month_start(day) - Duration::days(1))
}

/// The current streak and the longest one, from the local days the user checked in. The
/// current one counts back from this month, or from last month while this one has none yet.
pub fn streaks(days: &[NaiveDate], today: NaiveDate) -> (u32, u32) {
    let months: BTreeSet<NaiveDate> = days.iter().map(|d| month_start(*d)).collect();
    let (mut run, mut best, mut last) = (0, 0, None);
    for &month in &months {
        run = if last == Some(previous_month(month)) {
            run + 1
        } else {
            1
        };
        best = best.max(run);
        last = Some(month);
    }
    let alive = last.is_some_and(|m| m >= previous_month(today));
    (if alive { run } else { 0 }, best)
}
