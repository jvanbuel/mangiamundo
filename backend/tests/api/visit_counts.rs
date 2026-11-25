//! Counting the places visited in a window of time.

use mangiamundo::{account, store};

use crate::common::START;

#[tokio::test]
async fn visits_between_counts_distinct_places_inside_the_window() {
    use mangiamundo::geo::{Bbox, Shape};
    use mangiamundo::providers::{AreaInfo, AreaKind, Boundary, FoundPlace};
    use mangiamundo::store::{save_area, Area};

    let db = store::connect("sqlite::memory:").await.unwrap();
    let place = |id: &str, countries: Vec<&'static str>| FoundPlace {
        id: id.into(),
        name: id.into(),
        lat: 50.0,
        lng: 4.0,
        address: None,
        url: None,
        cuisine: None,
        opening_hours: None,
        countries,
    };
    let info = AreaInfo {
        id: "test-area".into(),
        name: "Test".into(),
        kind: AreaKind::City,
        detail: None,
        lat: 50.0,
        lng: 4.0,
    };
    let shape = Shape::rect(Bbox {
        south: 49.9,
        west: 3.9,
        north: 50.1,
        east: 4.1,
    });
    let area = Area {
        boundary: Boundary { info, shape },
        provider: "test".into(),
        fetched_at: START,
    };
    save_area(
        &db,
        &area,
        &[
            place("sushi", vec!["JP"]),
            place("ramen", vec!["JP"]),
            place("fusion", vec!["JP", "KR"]),
            place("pasta", vec!["IT"]),
        ],
    )
    .await
    .unwrap();
    let token = account::create_user(&db, None, START).await.unwrap();
    let session = account::session_for_token(&db, &token, START).await;
    let user = session.unwrap().unwrap().user;

    let day = |s: &str| s.parse::<chrono::NaiveDate>().unwrap();
    let (first, last) = (day("2026-12-01"), day("2026-12-31"));
    // The stored local date decides, not the instant.
    store::add_visit(&db, &user, "sushi", false, START, day("2026-11-30"))
        .await
        .unwrap(); // the day before
    store::add_visit(&db, &user, "ramen", false, START, first)
        .await
        .unwrap(); // first day
    store::add_visit(&db, &user, "fusion", false, START, last)
        .await
        .unwrap(); // last day, two countries
    store::add_visit(&db, &user, "pasta", false, START, day("2026-12-02"))
        .await
        .unwrap(); // wrong country

    let japan = vec!["JP".to_string()];
    assert_eq!(
        store::visits_between(&db, &user, &japan, first, last)
            .await
            .unwrap(),
        2
    );
    let east_asia = vec!["JP".to_string(), "KR".to_string()];
    assert_eq!(
        store::visits_between(&db, &user, &east_asia, first, last)
            .await
            .unwrap(),
        2,
        "a place counts once"
    );
    assert_eq!(
        store::visits_between(&db, &user, &[], first, last)
            .await
            .unwrap(),
        0
    );
}
