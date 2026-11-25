use super::*;

#[tokio::test]
async fn imported_places_are_found_inside_a_box() {
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
            place("in", 50.88, 4.70, "greek"),
            place("out", 51.2, 4.4, "greek"),
            place("odd", 50.88, 4.70, "burger"),
        ],
    )
    .await
    .unwrap();
    let found = imported_in(
        &db,
        Bbox {
            south: 50.8,
            west: 4.6,
            north: 50.9,
            east: 4.8,
        },
    )
    .await
    .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, "in");
    assert_eq!(found[0].countries, vec!["GR"]);
}

#[tokio::test]
async fn an_import_marks_the_areas_in_its_box_stale() {
    let db = connect("sqlite::memory:").await.unwrap();
    for (id, lat, lng) in [("leuven", 50.88, 4.70), ("paris", 48.86, 2.35)] {
        sqlx::query("INSERT INTO areas (id, name, kind, lat, lng, shape, provider, fetched_at) VALUES (?, ?, 'city', ?, ?, '[]', 'osm', 1000)")
                .bind(id)
                .bind(id)
                .bind(lat)
                .bind(lng)
                .execute(&db)
                .await
                .unwrap();
    }
    replace_imported(
        &db,
        Bbox {
            south: 49.4,
            west: 2.4,
            north: 51.6,
            east: 6.5,
        },
        &[],
    )
    .await
    .unwrap();
    let fetched: Vec<(String, i64)> =
        sqlx::query_as("SELECT id, fetched_at FROM areas ORDER BY id")
            .fetch_all(&db)
            .await
            .unwrap();
    assert_eq!(fetched, vec![("leuven".into(), 0), ("paris".into(), 1000)]);
}

#[tokio::test]
async fn a_suggestion_gives_way_to_the_cuisine_osm_tags() {
    let db = connect("sqlite::memory:").await.unwrap();
    let user = new_user(&db).await;
    let suggest = |country, cuisine| NewPlace {
        name: "Lalibela".into(),
        lat: 50.88,
        lng: 4.7,
        address: None,
        url: None,
        country,
        cuisine,
        suggested_for: Some("osm-node-1".into()),
    };
    let all = Bbox {
        south: 50.0,
        west: 4.0,
        north: 51.0,
        east: 5.0,
    };
    let mut tagged = place("osm-node-1", 50.88, 4.7, "eritrean");
    tagged.name = "Lalibela".into();

    // Wrong guess: OSM says Eritrean. The visit moves over, the addition goes.
    let wrong = add_place(&db, &user, &suggest("ET", "ethiopian"), 1000, 5)
        .await
        .unwrap()
        .unwrap()
        .id;
    add_visit(
        &db,
        &user,
        &wrong,
        false,
        1000,
        NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        replace_imported(&db, all, &[tagged.clone()]).await.unwrap(),
        1
    );
    assert!(crate::store::place(&db, &wrong, Some(&user))
        .await
        .unwrap()
        .is_none());
    assert_eq!(visits(&db, &user, "osm-node-1").await.unwrap().len(), 1);
    assert!(additions(&db, &user).await.unwrap().is_empty());

    // Right guess: confirmed as any addition.
    add_place(&db, &user, &suggest("ER", "eritrean"), 1001, 5)
        .await
        .unwrap()
        .unwrap();
    replace_imported(&db, all, &[tagged]).await.unwrap();
    let done = additions(&db, &user).await.unwrap();
    assert_eq!(done[0].status, AdditionStatus::Confirmed);
}
