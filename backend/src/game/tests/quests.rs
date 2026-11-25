use std::collections::HashSet;

use super::*;

#[test]
fn every_area_always_has_a_quest() {
    let w = monthly_quests("empty", day("2026-11-25"), &[], &[], &[]);
    assert_eq!(ids(&w), ["check-ins"]);
    assert_eq!(
        (w.month.as_str(), w.starts, w.ends),
        ("2026-11", day("2026-11-01"), day("2026-11-30"))
    );
    assert_eq!(
        monthly_quests("x", day("2028-02-10"), &[], &[], &[]).ends,
        day("2028-02-29")
    );
    // Someone who tried everything in a one-country area still gets one.
    let rows = [row("a", "IT", true)];
    let history = [tasted("a", "IT", "2026-01-01", "2026-01-01")];
    let w = monthly_quests("one", day("2026-11-25"), &rows, &history, &[]);
    assert!(ids(&w).contains(&"check-ins"));
    assert!(!ids(&w).contains(&"new-country"));
}

#[test]
fn quests_are_fixed_for_the_month_and_vary_by_area_and_month() {
    let rows = [
        row("a", "IT", true),
        row("b", "IT", false),
        row("c", "JP", false),
        row("d", "MX", false),
    ];
    let before = [tasted("a", "IT", "2026-10-01", "2026-10-01")];
    let start = monthly_quests("leuven", day("2026-11-01"), &rows, &before, &["MX".into()]);
    assert_eq!(start.quests.len(), 3);
    // Checking in during the month changes progress, not which quests run.
    let mut later = before.to_vec();
    later.push(tasted("c", "JP", "2026-11-26", "2026-11-26"));
    let end = monthly_quests("leuven", day("2026-11-30"), &rows, &later, &["MX".into()]);
    assert_eq!(ids(&start), ids(&end));

    let picks: HashSet<String> = ["leuven", "gent", "brussel", "antwerpen"]
        .iter()
        .flat_map(|area| {
            ["2026-11-10", "2026-12-10"].map(|d| {
                ids(&monthly_quests(
                    area,
                    day(d),
                    &rows,
                    &before,
                    &["MX".into()],
                ))
                .join(",")
            })
        })
        .collect();
    assert!(picks.len() > 1, "always the same quests: {picks:?}");
}

#[test]
fn quest_progress() {
    let rows = [
        row("a", "IT", true),
        row("b", "IT", true),
        row("c", "JP", true),
        row("d", "MX", true),
        row("e", "MX", false),
    ];
    let history = [
        tasted("a", "IT", "2026-10-01", "2026-11-24"),
        tasted("b", "IT", "2026-11-24", "2026-11-24"),
        tasted("c", "JP", "2026-11-25", "2026-11-25"),
        tasted("d", "MX", "2026-11-25", "2026-11-25"),
    ];
    // Try every area id until each kind showed up once, so the test doesn't depend on the pick.
    let mut seen = std::collections::HashMap::new();
    for area in (0..40).map(|i| format!("area-{i}")) {
        let w = monthly_quests(
            &area,
            day("2026-11-26"),
            &rows,
            &history,
            &["MX".into(), "FR".into()],
        );
        for q in w.quests {
            seen.entry(q.id.clone()).or_insert(q);
        }
    }
    let q = |id: &str| seen.get(id).unwrap_or_else(|| panic!("no quest {id}"));
    // JP and MX are new this month; IT was known.
    assert_eq!(
        (q("new-country").progress, q("new-country").done),
        (1, true)
    );
    // Europe, Asia and the Americas.
    assert!(q("continents").done);
    // MX celebrates (FR has no places here); one of its two places visited.
    assert_eq!(q("celebration").countries, ["MX"]);
    assert_eq!((q("celebration").progress, q("celebration").target), (1, 2));
    // IT went from 1 of 2 places (gold) to 2 of 2 (diamond).
    assert_eq!(q("level-up").countries, ["IT"]);
    assert!(q("level-up").done);
    assert_eq!((q("check-ins").progress, q("check-ins").done), (2, true));
}

#[test]
fn check_ins_elsewhere_do_not_count_for_an_area() {
    let rows = [row("a", "IT", false), row("c", "JP", false)];
    // Two places in another area this month, on two continents, one of them new.
    let history = [
        tasted("x", "IT", "2026-11-24", "2026-11-24"),
        tasted("y", "MX", "2026-11-25", "2026-11-25"),
    ];
    for area in (0..40).map(|i| format!("area-{i}")) {
        let w = monthly_quests(&area, day("2026-11-26"), &rows, &history, &[]);
        for q in w.quests {
            assert_eq!((q.progress, q.done), (0, false), "{}", q.id);
        }
    }
}
