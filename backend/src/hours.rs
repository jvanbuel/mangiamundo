//! Opening hours in OSM syntax, evaluated in the restaurant's own time zone.

use chrono::{DateTime, TimeDelta};
use opening_hours::localization::{Coordinates, TzLocation};
use opening_hours::{Context, OpeningHours, RuleKind};

/// Whether a place is open at `now` (Unix time), and a short line such as
/// "Open until 22:00" or "Closed · opens Tue 11:30". `None` when the hours are
/// missing, unparseable or say "unknown".
pub fn status(raw: &str, lat: f64, lng: f64, now: i64) -> Option<(bool, String)> {
    let locale = TzLocation::from_coords(Coordinates::new(lat, lng)?);
    let tz = *locale.get_timezone();
    // Mappers often write "Mo-Tu, Th-Sa 12:00-14:00", where ", " would start a new rule
    // that opens Mo-Tu all day. Read it as a list first, then as written.
    let parsed = raw.replace(", ", ",").parse::<OpeningHours>().or_else(|_| raw.parse());
    let hours = parsed.ok()?.with_context(Context::default().with_locale(locale));
    let now = DateTime::from_timestamp(now, 0)?.with_timezone(&tz);
    let open = match hours.state(now).0 {
        RuleKind::Open => true,
        RuleKind::Closed => false,
        RuleKind::Unknown => return None,
    };
    // A change more than a week away is not worth mentioning ("24/7", "closed for good").
    let next = hours.next_change(now).filter(|t| *t - now < TimeDelta::days(7));
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
        assert_eq!(at("Mo-Fr 11:00-14:00", MONDAY_NOON), Some((true, "Open until 14:00".into())));
    }

    #[test]
    fn closed_until_later_today_or_another_day() {
        assert_eq!(at("Mo 18:00-22:00", MONDAY_NOON), Some((false, "Closed · opens 18:00".into())));
        assert_eq!(at("Tu-Su 11:30-22:00", MONDAY_NOON), Some((false, "Closed · opens Tue 11:30".into())));
    }

    #[test]
    fn reads_comma_space_as_a_list() {
        // 13:00 on a Monday: lunch, not "open all day Monday".
        assert_eq!(at("Mo-Tu, Th-Sa 12:00-14:30, 18:00-22:00", MONDAY_NOON), Some((true, "Open until 14:30".into())));
        // A genuine second rule still works.
        assert_eq!(at("Mo-Su 12:00-22:00, Mo off", MONDAY_NOON).map(|s| s.0), Some(false));
    }

    #[test]
    fn opening_tomorrow_names_the_day() {
        // 21:00 on Monday in Leuven.
        let monday_evening = MONDAY_NOON + 8 * 3600;
        assert_eq!(at("Tu-Sa 08:00-21:30", monday_evening), Some((false, "Closed · opens Tue 08:00".into())));
        assert_eq!(at("Mo 18:00-02:00", monday_evening), Some((true, "Open until 02:00".into())));
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
        assert_eq!(status("Mo 11:00-14:00", 35.68, 139.65, MONDAY_NOON).map(|s| s.0), Some(false));
        assert_eq!(status("Mo 20:00-23:00", 35.68, 139.65, MONDAY_NOON).map(|s| s.0), Some(true));
    }
}
