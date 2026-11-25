use super::*;

const LEUVEN: (f64, f64) = (50.88, 4.70);

#[tokio::test]
async fn the_nearest_places_of_a_country_come_from_the_areas_and_the_import() {
    let db = connect("sqlite::memory:").await.unwrap();
    let all = Bbox {
        south: -90.0,
        west: -180.0,
        north: 90.0,
        east: 180.0,
    };
    replace_imported(
        &db,
        all,
        &[
            place("leuven", 50.88, 4.70, "greek"),
            place("mechelen", 51.03, 4.48, "Greek;pizza"),
            place("antwerp", 51.22, 4.40, "greek"),
            place("italian", 50.88, 4.70, "italian"),
        ],
    )
    .await
    .unwrap();
    // One already a place, as an area rebuild or a nearby refresh leaves it.
    let listed = FoundPlace {
        countries: vec!["GR"],
        ..place("brussels", 50.85, 4.35, "greek")
    };
    let gone = FoundPlace {
        countries: vec!["GR"],
        ..place("gone", 50.88, 4.70, "greek")
    };
    add_nearby(&db, &[listed, gone], now()).await.unwrap();
    // Gone from OpenStreetMap: kept for its visits, but listed nowhere.
    refresh_place(&db, "gone", &crate::providers::Lookup::Gone, now())
        .await
        .unwrap();

    let found = nearest(&db, "GR", None, LEUVEN, 3).await.unwrap();
    let ids: Vec<&str> = found.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["leuven", "mechelen", "brussels"]);
    assert_eq!(found[0].distance_m, 0);
    assert!(found[1].distance_m < found[2].distance_m);
    assert_eq!(found[1].countries, ["GR"]);
    // The imported ones it returned are places now, so they open and take check-ins.
    for id in ["leuven", "mechelen"] {
        assert!(
            crate::store::place(&db, id, None).await.unwrap().is_some(),
            "{id}"
        );
    }
    assert!(crate::store::place(&db, "antwerp", None)
        .await
        .unwrap()
        .is_none());

    // Asked again, the saved ones come from the places and nothing is listed twice.
    let again = nearest(&db, "GR", None, LEUVEN, 10).await.unwrap();
    let ids: Vec<&str> = again.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["leuven", "mechelen", "brussels", "antwerp"]);
    assert!(nearest(&db, "PE", None, LEUVEN, 10)
        .await
        .unwrap()
        .is_empty());
}
