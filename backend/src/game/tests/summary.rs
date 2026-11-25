use super::*;

#[test]
fn streak_badges_use_the_best_streak() {
    let s = summarize(
        &[],
        Lifetime {
            streak: 1,
            best_streak: 4,
            ..Default::default()
        },
    );
    assert!(earned(&s, "streak-3"));
    assert!(!earned(&s, "streak-6"));
    assert_eq!((s.profile.streak, s.profile.best_streak), (1, 4));
}

#[test]
fn empty_area_earns_nothing() {
    let s = summarize(&[], Lifetime::default());
    assert_eq!(s.world.available, 0);
    assert!(s.badges.iter().all(|b| !b.earned));
    assert_eq!(s.profile.level, 1);
}

#[test]
fn counts_countries_continents_and_world() {
    let rows = [
        row("a", "IT", true),
        row("b", "IT", true),
        row("c", "FR", true),
        row("d", "FR", false),
        row("e", "JP", false),
    ];
    let s = summarize(
        &rows,
        Lifetime {
            visits: 3,
            countries: 2,
            ..Default::default()
        },
    );

    let status = |code: &str| s.countries.iter().find(|c| c.code == code).unwrap().status;
    assert_eq!(status("IT"), Status::Completed);
    assert_eq!(status("FR"), Status::Discovered);
    assert_eq!(status("JP"), Status::Available);

    let europe = s.continents.iter().find(|c| c.code == "EU").unwrap();
    assert_eq!(
        (europe.available, europe.discovered, europe.completed),
        (2, 2, 1)
    );
    assert_eq!(
        (s.world.available, s.world.discovered, s.world.completed),
        (3, 2, 1)
    );
    assert_eq!((s.places, s.places_visited), (5, 3));

    assert!(earned(&s, "first-bite"));
    assert!(earned(&s, "continent-EU"));
    assert!(!earned(&s, "continent-AS"));
    assert!(!earned(&s, "world"));
    assert!(earned(&s, "complete-1"));
    // Continents with nothing available in the city get no badge at all.
    assert!(s.badges.iter().all(|b| b.id != "continent-AF"));
}

#[test]
fn a_place_serving_two_countries_counts_once_as_a_place() {
    let rows = [row("a", "TH", true), row("a", "VN", true)];
    let s = summarize(
        &rows,
        Lifetime {
            visits: 1,
            countries: 2,
            ..Default::default()
        },
    );
    assert_eq!((s.places, s.places_visited), (1, 1));
    assert_eq!(s.world.discovered, 2);
    assert!(earned(&s, "world") && earned(&s, "world-complete"));
}

#[test]
fn countries_go_gold_platinum_diamond() {
    use Status::*;
    assert_eq!(Status::of(30, 0), Available);
    assert_eq!(Status::of(30, 1), Discovered);
    assert_eq!(Status::of(30, 4), Discovered);
    assert_eq!(Status::of(30, 5), Platinum);
    assert_eq!(Status::of(30, 9), Platinum);
    assert_eq!(Status::of(30, 10), Completed);
    // Small countries reach diamond by visiting every place, skipping platinum.
    assert_eq!(Status::of(3, 2), Discovered);
    assert_eq!(Status::of(3, 3), Completed);
    assert_eq!(Status::of(7, 6), Platinum);
    assert_eq!(Status::of(7, 7), Completed);
}

#[test]
fn added_restaurants_earn_xp_with_bonuses_for_new_countries_and_continents() {
    assert_eq!(
        (Bonus::None.xp(), Bonus::Country.xp(), Bonus::Continent.xp()),
        (30, 130, 380)
    );
    // One plain addition, one new country, one new continent (which is a new country too).
    let lifetime = Lifetime {
        added: 3,
        country_bonuses: 2,
        continent_bonuses: 1,
        ..Default::default()
    };
    let p = profile(lifetime);
    assert_eq!((p.xp, p.added), (30 + 130 + 380, 3));
    let s = summarize(&[], lifetime);
    assert!(earned(&s, "pioneer"));
    assert!(!earned(
        &summarize(
            &[],
            Lifetime {
                added: 1,
                ..Default::default()
            }
        ),
        "pioneer"
    ));
}
