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
