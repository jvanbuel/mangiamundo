use super::*;

#[test]
fn streak_counts_months_in_a_row() {
    let today = day("2026-11-25");
    assert_eq!(streaks(&[], today), (0, 0));
    // This month, last month (twice) and the month before: 3, across a year end too.
    let days = [
        day("2026-11-03"),
        day("2026-10-30"),
        day("2026-10-01"),
        day("2026-09-12"),
    ];
    assert_eq!(streaks(&days, today), (3, 3));
    assert_eq!(
        streaks(&[day("2026-12-31"), day("2027-01-01")], day("2027-01-20")),
        (2, 2)
    );
    // Nothing yet this month doesn't break it.
    assert_eq!(streaks(&days[1..], today), (2, 2));
    // A whole month without one does; the best run stays.
    let old = [day("2026-08-20"), day("2026-07-01"), day("2026-06-30")];
    assert_eq!(streaks(&old, today), (0, 3));
    let mixed = [&old[..], &days[..1]].concat();
    assert_eq!(streaks(&mixed, today), (1, 3));
}
