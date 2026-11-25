use super::*;

#[test]
fn finished_quests_add_xp() {
    let p = profile(Lifetime {
        quest_xp: 150,
        ..Default::default()
    });
    assert_eq!(p.xp, 150);
}

#[test]
fn verified_check_ins_earn_extra_xp() {
    let p = profile(Lifetime {
        visits: 2,
        verified: 1,
        ..Default::default()
    });
    assert_eq!(p.xp, 10 + 25);
}

#[test]
fn levels_follow_the_quadratic_curve() {
    assert_eq!(profile(Lifetime::default()).level, 1);
    // 1 visit in a new country: 60 XP, still level 1.
    let p = profile(Lifetime {
        visits: 1,
        countries: 1,
        ..Default::default()
    });
    assert_eq!(
        (p.xp, p.level, p.level_xp, p.next_level_xp),
        (60, 1, 0, 100)
    );
    // 5 visits over 5 countries: 300 XP, level 2 (100..400).
    let p = profile(Lifetime {
        visits: 5,
        countries: 5,
        ..Default::default()
    });
    assert_eq!(
        (p.xp, p.level, p.level_xp, p.next_level_xp),
        (300, 2, 100, 400)
    );
    assert_eq!(
        profile(Lifetime {
            visits: 10,
            countries: 6,
            ..Default::default()
        })
        .level,
        3
    );
}
