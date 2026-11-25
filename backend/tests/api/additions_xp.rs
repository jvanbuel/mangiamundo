//! The XP an added place earns once the import confirms it.

use std::sync::atomic::Ordering;

use axum::http::StatusCode;
use mangiamundo::store;
use serde_json::Value;

use crate::common::{app_with, badge, call, session, HERE, START};

#[tokio::test]
async fn an_added_place_earns_xp_once_the_import_confirms_it() {
    let db = store::connect("sqlite::memory:").await.unwrap();
    let (app, clock) = app_with(db.clone(), None, START);
    let (alice, bob, carol) = (
        session(&app).await,
        session(&app).await,
        session(&app).await,
    );

    // Countries by what they would bring to Leuven: one already there, one new from a
    // continent that is there, and two new from a continent that isn't.
    let (_, discover) = call(&app, "GET", &format!("/api/discover?{HERE}"), None, None).await;
    let here: Vec<&str> = discover["countries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["code"].as_str().unwrap())
        .collect();
    let continents_here: Vec<&str> = discover["countries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["continent"].as_str().unwrap())
        .collect();
    let (_, catalog) = call(&app, "GET", "/api/countries", None, None).await;
    let addable: Vec<&Value> = catalog["countries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| !c["cuisine"].is_null())
        .collect();
    let code = |c: &&Value| c["code"].as_str().unwrap().to_string();
    let present = addable
        .iter()
        .find(|c| here.contains(&c["code"].as_str().unwrap()))
        .map(code)
        .unwrap();
    let new_country = addable
        .iter()
        .find(|c| {
            !here.contains(&c["code"].as_str().unwrap())
                && continents_here.contains(&c["continent"].as_str().unwrap())
        })
        .map(code)
        .unwrap();
    let elsewhere: Vec<&&Value> = addable
        .iter()
        .filter(|c| !continents_here.contains(&c["continent"].as_str().unwrap()))
        .collect();
    let far = elsewhere
        .iter()
        .find(|c| {
            elsewhere
                .iter()
                .any(|o| o["continent"] == c["continent"] && o["code"] != c["code"])
        })
        .unwrap();
    let (far_a, far_b) = (
        code(far),
        elsewhere
            .iter()
            .find(|o| o["continent"] == far["continent"] && o["code"] != far["code"])
            .map(|c| code(c))
            .unwrap(),
    );

    let (_, places) = call(&app, "GET", &format!("/api/places?{HERE}"), None, None).await;
    let (lat, lng) = (
        places[0]["lat"].as_f64().unwrap(),
        places[0]["lng"].as_f64().unwrap(),
    );
    let add = |token: String, name: &'static str, country: String| {
        let (app, clock) = (app.clone(), clock.clone());
        async move {
            clock.fetch_add(60, Ordering::Relaxed);
            let body = format!(
                r#"{{"name":"{name}","country":"{country}","lat":{lat},"lng":{lng},"note":false}}"#
            );
            let (status, place) =
                call(&app, "POST", "/api/places", Some(&token), Some(&body)).await;
            assert_eq!(status, StatusCode::OK, "{place}");
            place
        }
    };
    let reward = |place: &Value| {
        (
            place["reward"]["bonus"].as_str().unwrap().to_string(),
            place["reward"]["xp"].as_u64().unwrap(),
        )
    };
    let s = |v: &str| v.to_string();

    // Decided when added, ignoring other pending additions.
    let alpha = add(alice.clone(), "Alpha", far_a.clone()).await;
    assert_eq!(reward(&alpha), (s("continent"), 380));
    assert_eq!(alpha["reward"]["area_name"], "Leuven");
    assert_eq!(
        alpha["countries"],
        serde_json::json!([far_a]),
        "still a place"
    );
    assert_eq!(
        reward(&add(bob.clone(), "Beta", far_b.clone()).await),
        (s("continent"), 380)
    );
    assert_eq!(
        reward(&add(bob.clone(), "alpha", far_a.clone()).await),
        (s("continent"), 380)
    );
    assert_eq!(
        reward(&add(carol.clone(), "Gamma", present.clone()).await),
        (s("none"), 30)
    );
    assert_eq!(
        reward(&add(carol.clone(), "Delta", new_country.clone()).await),
        (s("country"), 130)
    );

    let get = |uri: &'static str, token: String| {
        let app = app.clone();
        async move { call(&app, "GET", uri, Some(&token), None).await.1 }
    };
    let xp = |token: String| {
        let app = app.clone();
        async move {
            call(
                &app,
                "GET",
                &format!("/api/discover?{HERE}"),
                Some(&token),
                None,
            )
            .await
            .1["profile"]["xp"]
                .as_u64()
                .unwrap()
        }
    };
    let pending = get("/api/additions", alice.clone()).await;
    assert_eq!(pending[0]["status"], "pending");
    assert_eq!(pending[0]["xp"], 380);
    assert_eq!(xp(alice.clone()).await, 0, "nothing before confirmation");
    assert_eq!(
        call(&app, "GET", "/api/additions", None, None).await.0,
        StatusCode::UNAUTHORIZED
    );

    // The import brings all four restaurants (Alpha once).
    let twin = |n: u32, name: &str, country: &str| {
        let cuisine = mangiamundo::catalog::cuisine_tag(country).unwrap();
        mangiamundo::providers::FoundPlace {
            id: format!("osm-node-{n}"),
            name: name.into(),
            lat,
            lng,
            address: None,
            url: None,
            cuisine: Some(cuisine.into()),
            opening_hours: None,
            countries: vec![],
        }
    };
    let all = mangiamundo::geo::Bbox {
        south: -90.0,
        west: -180.0,
        north: 90.0,
        east: 180.0,
    };
    let import = [
        twin(1, "Alpha", &far_a),
        twin(2, "Beta", &far_b),
        twin(3, "Gamma", &present),
        twin(4, "Delta", &new_country),
    ];
    assert_eq!(store::replace_imported(&db, all, &import).await.unwrap(), 5);

    let mine = get("/api/additions", alice.clone()).await;
    assert_eq!(mine[0]["status"], "confirmed");
    assert_eq!(mine[0]["place_id"], "osm-node-1");
    assert_eq!(mine[0]["seen"], false);
    assert_eq!(xp(alice.clone()).await, 380);
    let (_, discover) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&alice),
        None,
    )
    .await;
    assert_eq!(badge(&discover, "pioneer")["earned"], true);
    assert_eq!(discover["profile"]["added"], 1);

    // Bob's continent went to Alice, who was first, but Beta still brings a new country.
    // His Alpha was hers already.
    let his = get("/api/additions", bob.clone()).await;
    let status = |a: &Value| {
        (
            a["name"].as_str().unwrap().to_string(),
            a["status"].as_str().unwrap().to_string(),
            a["bonus"].as_str().unwrap().to_string(),
            a["xp"].as_u64().unwrap(),
        )
    };
    assert_eq!(status(&his[0]), (s("alpha"), s("duplicate"), s("none"), 0));
    assert_eq!(
        status(&his[1]),
        (s("Beta"), s("confirmed"), s("country"), 130)
    );
    assert_eq!(xp(bob.clone()).await, 130);
    assert_eq!(xp(carol.clone()).await, 30 + 130);

    // Only what the caller lists, and only their own.
    let seen = |ids: &[&Value]| format!(r#"{{"ids":{}}}"#, serde_json::to_string(ids).unwrap());
    let put = |token: String, body: String| {
        let app = app.clone();
        async move {
            call(
                &app,
                "PUT",
                "/api/additions/seen",
                Some(&token),
                Some(&body),
            )
            .await
            .0
        }
    };
    let beta = &his[1]["id"];
    assert_eq!(
        put(alice.clone(), seen(&[beta])).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        get("/api/additions", bob.clone()).await[1]["seen"],
        false,
        "Beta is Bob's"
    );
    assert_eq!(
        get("/api/additions", alice.clone()).await[0]["seen"],
        false,
        "not listed"
    );
    assert_eq!(
        put(alice.clone(), seen(&[&mine[0]["id"]])).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(get("/api/additions", alice.clone()).await[0]["seen"], true);

    let too_many: Vec<Value> = (0..101).map(|i| Value::from(format!("user-{i}"))).collect();
    assert_eq!(
        put(bob.clone(), seen(&too_many.iter().collect::<Vec<_>>())).await,
        StatusCode::BAD_REQUEST
    );
}
