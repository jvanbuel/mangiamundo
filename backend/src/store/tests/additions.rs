use super::*;

#[tokio::test]
async fn two_additions_of_one_place_are_marked_seen_one_at_a_time() {
    let db = connect("sqlite::memory:").await.unwrap();
    let user = new_user(&db).await;
    let new = NewPlace {
        name: "Lalibela".into(),
        lat: 50.88,
        lng: 4.7,
        address: None,
        url: None,
        country: "ET",
        cuisine: "ethiopian",
        suggested_for: None,
    };
    // The same user adds the same restaurant twice; the import confirms both.
    let first = add_place(&db, &user, &new, 1000, 5)
        .await
        .unwrap()
        .unwrap()
        .id;
    let second = add_place(&db, &user, &new, 1001, 5)
        .await
        .unwrap()
        .unwrap()
        .id;
    let mut twin = place("osm-node-1", 50.88, 4.7, "ethiopian");
    twin.name = "Lalibela".into();
    let all = Bbox {
        south: -90.0,
        west: -180.0,
        north: 90.0,
        east: 180.0,
    };
    replace_imported(&db, all, &[twin]).await.unwrap();
    let listed = additions(&db, &user).await.unwrap();
    assert!(
        listed.iter().all(|a| a.place_id == "osm-node-1"),
        "both are the OSM place now"
    );

    // Told about the first only: the second is still news.
    mark_additions_seen(&db, &user, std::slice::from_ref(&first))
        .await
        .unwrap();
    let seen: Vec<(String, bool)> = additions(&db, &user)
        .await
        .unwrap()
        .into_iter()
        .map(|a| (a.id, a.seen))
        .collect();
    assert_eq!(seen, vec![(second, false), (first, true)]);
}

#[tokio::test]
async fn the_bonus_is_judged_in_the_city_never_the_postal_code() {
    let db = connect("sqlite::memory:").await.unwrap();
    let user = new_user(&db).await;
    let shape = serde_json::to_string(&Shape::rect(Bbox {
        south: 50.8,
        west: 4.6,
        north: 50.95,
        east: 4.8,
    }))
    .unwrap();
    let area = |id: &str, kind: &str| {
        sqlx::query("INSERT INTO areas (id, name, kind, lat, lng, shape, provider, fetched_at) VALUES (?1, ?1, ?2, 50.88, 4.7, ?3, 'osm', 1000)")
                .bind(id.to_string())
                .bind(kind.to_string())
                .bind(shape.clone())
    };
    let new = NewPlace {
        name: "Lalibela".into(),
        lat: 50.88,
        lng: 4.7,
        address: None,
        url: None,
        country: "ET",
        cuisine: "ethiopian",
        suggested_for: None,
    };

    // Only a postal code is cached: no bonus, though Ethiopia is new there.
    area("3000", "postal_code").execute(&db).await.unwrap();
    let added = add_place(&db, &user, &new, 1000, 5).await.unwrap().unwrap();
    assert_eq!((added.bonus, added.area_name), (Bonus::None, None));

    // With the city cached too, the bonus is judged there.
    area("leuven", "city").execute(&db).await.unwrap();
    let added = add_place(&db, &user, &new, 1001, 5).await.unwrap().unwrap();
    assert_eq!(
        (added.bonus, added.area_name.as_deref()),
        (Bonus::Continent, Some("leuven"))
    );
}

#[tokio::test]
async fn a_bonus_held_by_an_addition_that_turns_out_a_duplicate_goes_to_the_next_in_line() {
    let db = connect("sqlite::memory:").await.unwrap();
    let user = new_user(&db).await;
    // Three Ethiopian additions in one city, each claiming the country bonus. Habesha is
    // 220 m from the two Lalibelas, which are the same restaurant.
    for (id, name, lat, at) in [
        ("user-a", "Lalibela Original", 50.88, 1000),
        ("user-b", "Habesha", 50.882, 1001),
        ("user-c", "Lalibela New", 50.88, 1002),
    ] {
        for sql in [
                "INSERT INTO places (id, name, lat, lng, cuisine, updated_at, added_by, added_at)
                 VALUES (?1, ?2, ?3, 4.7, 'ethiopian', ?4, ?5, ?4)",
                "INSERT INTO place_countries (place_id, country) VALUES (?1, 'ET')",
                "INSERT INTO additions (place_id, user_id, name, country, area_id, claimed_bonus, bonus, submitted_at)
                 VALUES (?1, ?5, ?2, 'ET', 'leuven', 'country', 'country', ?4)",
            ] {
                sqlx::query(sql).bind(id).bind(name).bind(lat).bind(at).bind(&user).execute(&db).await.unwrap();
            }
    }
    let osm = |id: &str, name: &str, lat| {
        let mut p = place(id, lat, 4.7, "ethiopian");
        p.name = name.into();
        p
    };
    let all = Bbox {
        south: -90.0,
        west: -180.0,
        north: 90.0,
        east: 180.0,
    };
    let settled = || async {
        sqlx::query_as::<_, (String, String, bool)>(
            "SELECT place_id, bonus, rewarded FROM additions ORDER BY place_id",
        )
        .fetch_all(&db)
        .await
        .unwrap()
    };
    let row = |id: &str, bonus: &str, rewarded| (id.to_string(), bonus.to_string(), rewarded);

    // Habesha and "Lalibela New" are mapped; "Lalibela Original" matches neither, and
    // still holds the bonus as the earliest claim.
    replace_imported(
        &db,
        all,
        &[
            osm("osm-node-1", "Lalibela New", 50.88),
            osm("osm-node-2", "Habesha", 50.882),
        ],
    )
    .await
    .unwrap();
    assert_eq!(
        settled().await,
        vec![
            row("user-a", "country", false),
            row("user-b", "none", true),
            row("user-c", "none", true)
        ]
    );

    // The mapper renames it "Lalibela": the first addition now matches, but the place
    // went to "Lalibela New". The bonus it held goes to Habesha, next in line.
    replace_imported(
        &db,
        all,
        &[
            osm("osm-node-1", "Lalibela", 50.88),
            osm("osm-node-2", "Habesha", 50.882),
        ],
    )
    .await
    .unwrap();
    assert_eq!(
        settled().await,
        vec![
            row("user-a", "none", false),
            row("user-b", "country", true),
            row("user-c", "none", true)
        ]
    );
}
