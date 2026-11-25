//! Opening hours in OSM syntax, evaluated in the restaurant's own time zone.

use chrono::{DateTime, Datelike, TimeDelta};
use chrono_tz::Tz;
use opening_hours::localization::{Coordinates, TzLocation};
use opening_hours::schedule::TimeRange;
use opening_hours::{Context, OpeningHours, RuleKind};
use serde::Serialize;

/// One day of a place's week.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct Day {
    /// English weekday name, e.g. "Monday".
    pub name: String,
    /// The current day where the place is.
    pub today: bool,
    /// When it is open, e.g. `["11:30-14:00", "18:00-22:00"]`; empty when closed all day.
    /// A range past midnight belongs to the day it starts: "18:00-02:00".
    pub open: Vec<String>,
}

/// The hours parsed and set in the time zone at `lat`, `lng`.
fn parse(raw: &str, lat: f64, lng: f64) -> Option<(OpeningHours<TzLocation<Tz>>, Tz)> {
    let locale = TzLocation::from_coords(Coordinates::new(lat, lng)?);
    let tz = *locale.get_timezone();
    // Mappers often write "Mo-Tu, Th-Sa 12:00-14:00", where ", " would start a new rule
    // that opens Mo-Tu all day. Read it as a list first, then as written.
    let parsed = raw
        .replace(", ", ",")
        .parse::<OpeningHours>()
        .or_else(|_| raw.parse());
    Some((
        parsed
            .ok()?
            .with_context(Context::default().with_locale(locale)),
        tz,
    ))
}

/// Whether a place is open at `now` (Unix time), and a short line such as
/// "Open until 22:00" or "Closed · opens Tue 11:30". `None` when the hours are
/// missing, unparseable or say "unknown".
pub fn status(raw: &str, lat: f64, lng: f64, now: i64) -> Option<(bool, String)> {
    let (hours, tz) = parse(raw, lat, lng)?;
    let now = DateTime::from_timestamp(now, 0)?.with_timezone(&tz);
    let open = match hours.state(now).0 {
        RuleKind::Open => true,
        RuleKind::Closed => false,
        RuleKind::Unknown => return None,
    };
    // A change more than a week away is not worth mentioning ("24/7", "closed for good").
    let next = hours
        .next_change(now)
        .filter(|t| *t - now < TimeDelta::days(7));
    // Closing just after midnight still reads as tonight; opening tomorrow needs the day.
    let when = |t: DateTime<chrono_tz::Tz>| {
        let tonight = open && t - now < TimeDelta::hours(12);
        if tonight || t.date_naive() == now.date_naive() {
            t.format("%H:%M").to_string()
        } else {
            t.format("%a %H:%M").to_string()
        }
    };
    let text = match (open, next) {
        (true, Some(t)) => format!("Open until {}", when(t)),
        (true, None) => "Open".to_string(),
        (false, Some(t)) => format!("Closed · opens {}", when(t)),
        (false, None) => "Closed".to_string(),
    };
    Some((open, text))
}

/// The week containing `now`, Monday to Sunday, in the place's time zone. `None` when the
/// hours can't be parsed or say "unknown" for part of the week.
pub fn week(raw: &str, lat: f64, lng: f64, now: i64) -> Option<Vec<Day>> {
    let (hours, tz) = parse(raw, lat, lng)?;
    let today = DateTime::from_timestamp(now, 0)?
        .with_timezone(&tz)
        .date_naive();
    let monday = today - TimeDelta::days(today.weekday().num_days_from_monday().into());
    // The Sunday before and the Monday after too, for ranges that cross midnight.
    let days: Vec<Vec<TimeRange>> = (-1..=7)
        .map(|i| {
            hours
                .schedule_at(monday + TimeDelta::days(i))
                .into_iter()
                .collect()
        })
        .collect();
    // "unknown" hours would read as closed; clients show the raw text instead.
    if days[1..8]
        .iter()
        .flatten()
        .any(|r| r.kind == RuleKind::Unknown)
    {
        return None;
    }
    let days: Vec<Vec<TimeRange>> = days
        .into_iter()
        .map(|d| d.into_iter().filter(|r| r.kind == RuleKind::Open).collect())
        .collect();
    Some(
        (0..7)
            .zip(0..)
            .map(|(i, offset)| {
                let date = monday + TimeDelta::days(offset);
                Day {
                    name: date.format("%A").to_string(),
                    today: date == today,
                    open: spans(&days[i], &days[i + 1], &days[i + 2]),
                }
            })
            .collect(),
    )
}

/// Minutes in a day: 24:00 is where a range that runs past midnight is cut.
const DAY_END: u16 = 24 * 60;

/// The open ranges of a day as text, joining a late range with its continuation after
/// midnight and leaving out the continuation of the day before.
fn spans(before: &[TimeRange], day: &[TimeRange], after: &[TimeRange]) -> Vec<String> {
    let mins = |r: &TimeRange| {
        (
            r.range.start.mins_from_midnight(),
            r.range.end.mins_from_midnight(),
        )
    };
    // A range up to 24:00 that started that day, followed by one from 00:00 the next day.
    let crosses = |late: Option<&TimeRange>, early: Option<&TimeRange>| match (
        late.map(mins),
        early.map(mins),
    ) {
        (Some((start, DAY_END)), Some((0, end))) => start > 0 && end < DAY_END,
        _ => false,
    };
    let skip = usize::from(crosses(before.last(), day.first()));
    let carry = crosses(day.last(), after.first()).then(|| after[0].range.end);
    let last = day.len().saturating_sub(1);
    day.iter()
        .enumerate()
        .skip(skip)
        .map(|(i, r)| match (mins(r), carry) {
            ((0, DAY_END), _) => "24 hours".to_string(),
            (_, Some(end)) if i == last => format!("{}-{end}", r.range.start),
            _ => format!("{}-{}", r.range.start, r.range.end),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEUVEN: (f64, f64) = (50.8798, 4.7005);
    /// Monday 16 November 2026, 12:00 UTC = 13:00 in Leuven.
    const MONDAY_NOON: i64 = 1_794_830_400;

    fn at(raw: &str, now: i64) -> Option<(bool, String)> {
        status(raw, LEUVEN.0, LEUVEN.1, now)
    }

    #[test]
    fn open_with_closing_time_in_local_time() {
        assert_eq!(
            at("Mo-Fr 11:00-14:00", MONDAY_NOON),
            Some((true, "Open until 14:00".into()))
        );
    }

    #[test]
    fn closed_until_later_today_or_another_day() {
        assert_eq!(
            at("Mo 18:00-22:00", MONDAY_NOON),
            Some((false, "Closed · opens 18:00".into()))
        );
        assert_eq!(
            at("Tu-Su 11:30-22:00", MONDAY_NOON),
            Some((false, "Closed · opens Tue 11:30".into()))
        );
    }

    #[test]
    fn reads_comma_space_as_a_list() {
        // 13:00 on a Monday: lunch, not "open all day Monday".
        assert_eq!(
            at("Mo-Tu, Th-Sa 12:00-14:30, 18:00-22:00", MONDAY_NOON),
            Some((true, "Open until 14:30".into()))
        );
        // A genuine second rule still works.
        assert_eq!(
            at("Mo-Su 12:00-22:00, Mo off", MONDAY_NOON).map(|s| s.0),
            Some(false)
        );
    }

    #[test]
    fn opening_tomorrow_names_the_day() {
        // 21:00 on Monday in Leuven.
        let monday_evening = MONDAY_NOON + 8 * 3600;
        assert_eq!(
            at("Tu-Sa 08:00-21:30", monday_evening),
            Some((false, "Closed · opens Tue 08:00".into()))
        );
        assert_eq!(
            at("Mo 18:00-02:00", monday_evening),
            Some((true, "Open until 02:00".into()))
        );
    }

    #[test]
    fn always_open_and_unknown() {
        assert_eq!(at("24/7", MONDAY_NOON), Some((true, "Open".into())));
        assert_eq!(at("Mo-Su unknown", MONDAY_NOON), None);
        assert_eq!(at("not hours at all", MONDAY_NOON), None);
    }

    #[test]
    fn uses_the_restaurants_time_zone() {
        // 12:00 UTC is 21:00 in Tokyo.
        assert_eq!(
            status("Mo 11:00-14:00", 35.68, 139.65, MONDAY_NOON).map(|s| s.0),
            Some(false)
        );
        assert_eq!(
            status("Mo 20:00-23:00", 35.68, 139.65, MONDAY_NOON).map(|s| s.0),
            Some(true)
        );
    }

    fn open_times(raw: &str) -> Vec<(String, bool, Vec<String>)> {
        week(raw, LEUVEN.0, LEUVEN.1, MONDAY_NOON)
            .unwrap()
            .into_iter()
            .map(|d| (d.name, d.today, d.open))
            .collect()
    }

    #[test]
    fn week_runs_monday_to_sunday_and_marks_today() {
        let days = open_times("Mo-Fr 11:00-14:00, 18:00-22:00");
        assert_eq!(days.len(), 7);
        assert_eq!(
            days[0],
            (
                "Monday".into(),
                true,
                vec!["11:00-14:00".into(), "18:00-22:00".into()]
            )
        );
        assert_eq!(days[6], ("Sunday".into(), false, vec![]));
        // Wednesday 21:00 UTC is Wednesday in Leuven, and already Thursday in Tokyo.
        let wednesday = MONDAY_NOON + 2 * 86_400 + 9 * 3600;
        assert!(week("24/7", LEUVEN.0, LEUVEN.1, wednesday).unwrap()[2].today);
        assert!(week("24/7", 35.68, 139.65, wednesday).unwrap()[3].today);
    }

    #[test]
    fn week_keeps_late_ranges_on_the_day_they_start() {
        let days = open_times("Mo 18:00-02:00");
        assert_eq!(days[0].2, vec!["18:00-02:00".to_string()]);
        assert_eq!(days[1].2, Vec::<String>::new());
        // Sunday's late range continues into next week's Monday, and this week's Monday
        // doesn't start with last Sunday's.
        let days = open_times("Su 20:00-01:00");
        assert_eq!(days[0].2, Vec::<String>::new());
        assert_eq!(days[6].2, vec!["20:00-01:00".to_string()]);
    }

    #[test]
    fn week_of_always_open_and_unreadable_hours() {
        assert!(open_times("24/7")
            .iter()
            .all(|d| d.2 == vec!["24 hours".to_string()]));
        assert!(week("not hours at all", LEUVEN.0, LEUVEN.1, MONDAY_NOON).is_none());
        assert!(week("Mo-Su 10:00-18:00 unknown", LEUVEN.0, LEUVEN.1, MONDAY_NOON).is_none());
        assert!(week(
            "Mo-Fr 11:00-14:00; Sa unknown",
            LEUVEN.0,
            LEUVEN.1,
            MONDAY_NOON
        )
        .is_none());
    }
}
