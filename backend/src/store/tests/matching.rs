use super::*;
use crate::store::areas::upsert_place;
use crate::store::matching::similar_names;

#[test]
fn names_match_ignoring_case_punctuation_and_extra_words() {
    assert!(similar_names("Lalibela", "Restaurant LALIBELA"));
    assert!(similar_names("Chez Marie-Thérèse", "chez marie thérèse"));
    assert!(!similar_names("Lalibela", "Habesha"));
    assert!(!similar_names("", "Habesha"));
}

#[tokio::test]
async fn an_import_swaps_pending_places_for_their_osm_twins() {
    let db = connect("sqlite::memory:").await.unwrap();
    let mut users = vec![];
    for _ in 0..2 {
        users.push(new_user(&db).await);
    }
    let (a, b) = (&users[0], &users[1]);
    let new = |name: &str, lat, lng| NewPlace {
        name: name.into(),
        lat,
        lng,
        address: None,
        url: None,
        country: "ET",
        cuisine: "ethiopian",
        suggested_for: None,
    };
    let day = |d| NaiveDate::from_ymd_opt(2026, 11, d).unwrap();
    let mine = add_place(&db, a, &new("Lalibela", 50.88, 4.7), 1000, 5)
        .await
        .unwrap()
        .unwrap()
        .id;
    let theirs = add_place(&db, b, &new("lalibela", 50.88, 4.7), 1001, 5)
        .await
        .unwrap()
        .unwrap()
        .id;
    let far = add_place(&db, a, &new("Lalibela", 50.882, 4.7), 1002, 5)
        .await
        .unwrap()
        .unwrap()
        .id;
    // 0.0012° of longitude is 46 m up here, past a fixed 0.001° box.
    add_place(&db, a, &new("Polar", 70.0, 20.0), 1003, 5)
        .await
        .unwrap()
        .unwrap();

    // `a` already checked in at the OSM place on the 19th, as well as at their own copy.
    let mut twin = place("osm-node-1", 50.8801, 4.7, "ethiopian");
    twin.name = "Restaurant Lalibela".into();
    let mut conn = db.acquire().await.unwrap();
    upsert_place(&mut conn, &twin, 1000).await.unwrap();
    drop(conn);
    let at_twin = add_visit(&db, a, "osm-node-1", true, 1000, day(19))
        .await
        .unwrap();
    add_visit(&db, a, &mine, true, 1000, day(19)).await.unwrap();
    let mine_20th = add_visit(&db, a, &mine, true, 2000, day(20)).await.unwrap();
    let theirs_19th = add_visit(&db, b, &theirs, true, 1000, day(19))
        .await
        .unwrap();

    let mut decoy = place("osm-node-2", 50.88, 4.7, "ethiopian");
    decoy.name = "Habesha".into();
    let mut polar = place("osm-node-3", 70.0, 20.0012, "ethiopian");
    polar.name = "Polar".into();
    let all = Bbox {
        south: -90.0,
        west: -180.0,
        north: 90.0,
        east: 180.0,
    };
    assert_eq!(
        replace_imported(&db, all, &[decoy, twin, polar])
            .await
            .unwrap(),
        3
    );

    let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM places ORDER BY id")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(
        ids,
        vec!["osm-node-1".to_string(), "osm-node-3".into(), far]
    );
    // Only the clashing visit on the 19th goes; the others keep their ids.
    let visits: Vec<(i64, String)> = sqlx::query_as("SELECT id, place_id FROM visits ORDER BY id")
        .fetch_all(&db)
        .await
        .unwrap();
    let on_twin = |v: &Visit| (v.id, "osm-node-1".to_string());
    assert_eq!(
        visits,
        vec![
            on_twin(&at_twin),
            on_twin(&mine_20th),
            on_twin(&theirs_19th)
        ]
    );
    let countries: Vec<String> =
        sqlx::query_scalar("SELECT country FROM place_countries WHERE place_id = 'osm-node-1'")
            .fetch_all(&db)
            .await
            .unwrap();
    assert_eq!(countries, vec!["ET"]);
}

#[tokio::test]
async fn additions_made_at_the_same_second_are_settled_in_one_order() {
    let db = connect("sqlite::memory:").await.unwrap();
    let user = new_user(&db).await;
    // Two additions of the same new restaurant in the same second. "user-b" is inserted
    // first, so an order by time alone would likely settle it first.
    for id in ["user-b", "user-a"] {
        for sql in [
                "INSERT INTO places (id, name, lat, lng, cuisine, updated_at, added_by, added_at)
                 VALUES (?1, 'Lalibela', 50.88, 4.7, 'ethiopian', 1000, ?2, 1000)",
                "INSERT INTO place_countries (place_id, country) VALUES (?1, 'ET')",
                "INSERT INTO additions (place_id, user_id, name, country, area_id, claimed_bonus, bonus, submitted_at)
                 VALUES (?1, ?2, 'Lalibela', 'ET', 'leuven', 'country', 'country', 1000)",
            ] {
                sqlx::query(sql).bind(id).bind(&user).execute(&db).await.unwrap();
            }
    }
    let mut twin = place("osm-node-1", 50.88, 4.7, "ethiopian");
    twin.name = "Lalibela".into();
    let all = Bbox {
        south: -90.0,
        west: -180.0,
        north: 90.0,
        east: 180.0,
    };
    replace_imported(&db, all, &[twin]).await.unwrap();
    let settled: Vec<(String, String, bool)> =
        sqlx::query_as("SELECT place_id, bonus, rewarded FROM additions ORDER BY place_id")
            .fetch_all(&db)
            .await
            .unwrap();
    assert_eq!(
        settled,
        vec![
            ("user-a".into(), "country".into(), true),
            ("user-b".into(), "none".into(), false)
        ]
    );
}
