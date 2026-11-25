//! Check-ins completing countries and saying what they unlocked.

use axum::http::StatusCode;
use serde_json::Value;

use crate::common::{app, at, badge, call, session, visits_uri, HERE};

#[tokio::test]
async fn visiting_every_place_completes_a_country() {
    let app = app().await;
    let token = session(&app).await;
    let (_, discover) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&token),
        None,
    )
    .await;
    let country = discover["countries"][0]["code"]
        .as_str()
        .unwrap()
        .to_string();

    let (status, places) = call(
        &app,
        "GET",
        &format!("/api/places?{HERE}&country={country}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let places = places.as_array().unwrap().clone();
    assert_eq!(
        places.len() as u64,
        discover["countries"][0]["places"].as_u64().unwrap()
    );
    assert!(places.iter().all(|p| p["countries"]
        .as_array()
        .unwrap()
        .contains(&Value::from(country.clone()))));
    // Open places first, then closed, then unknown hours; alphabetical within each group.
    let key = |p: &Value| {
        let group = match p["hours"]["status"]["open"].as_bool() {
            Some(true) => 0,
            Some(false) => 1,
            None => 2,
        };
        (group, p["name"].as_str().unwrap().to_lowercase())
    };
    assert!(places.windows(2).all(|w| key(&w[0]) <= key(&w[1])));
    assert!(places.iter().all(|p| p["visits"] == Value::Array(vec![])));

    // First check-in, made on the spot: 10 XP for the visit, 15 for being there, 50 for the
    // country, and 100 for the "taste a new country" quest it finishes.
    let first = &places[0];
    let here = at(first);
    let (status, visit) = call(&app, "POST", &visits_uri(first), Some(&token), Some(&here)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(visit["visit"]["verified"], true);
    assert_eq!(visit["profile"]["xp"], 175);

    for place in &places[1..] {
        let uri = visits_uri(place);
        let (status, visit) = call(&app, "POST", &uri, Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(visit["visit"]["verified"], false);
        // Checking in twice changes nothing.
        call(&app, "POST", &uri, Some(&token), None).await;
    }

    let (_, after) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&token),
        None,
    )
    .await;
    let entry = &after["countries"][0];
    assert_eq!(entry["visited"], entry["places"]);
    assert_eq!(entry["status"], "completed");
    assert_eq!(after["world"]["discovered"], 1);
    assert_eq!(after["world"]["completed"], 1);
    assert_eq!(
        after["places_visited"].as_u64().unwrap(),
        places.len() as u64
    );
    assert_eq!(
        after["profile"]["visits"].as_u64().unwrap(),
        places.len() as u64
    );
    assert_eq!(badge(&after, "first-bite")["earned"], true);
    assert_eq!(badge(&after, "complete-1")["earned"], true);
    assert_eq!(badge(&after, "world")["earned"], false);

    // Another user sees none of it.
    let other = session(&app).await;
    let (_, theirs) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&other),
        None,
    )
    .await;
    assert_eq!(theirs["world"]["discovered"], 0);

    // Undoing a visit takes the completion away again, and says so with the area.
    let uri = format!("/api/visits/{}?{HERE}", visit["visit"]["id"]);
    let (status, removed) = call(&app, "DELETE", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(removed["progress"]["world"]["completed"], 0);
    let (_, undone) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(undone["world"]["completed"], 0);
    let mut undone = undone;
    undone.as_object_mut().unwrap().remove("area");
    assert_eq!(removed["progress"], undone);
    assert_eq!(removed["profile"], undone["profile"]);
}

#[tokio::test]
async fn a_check_in_says_what_it_unlocked_in_the_area() {
    let app = app().await;
    let token = session(&app).await;
    let discover = |token: String| {
        let app = &app;
        async move {
            let uri = format!("/api/discover?{HERE}");
            call(app, "GET", &uri, Some(&token), None).await.1
        }
    };
    let before = discover(token.clone()).await;
    let busy = before["countries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["places"].as_u64().unwrap() >= 3)
        .unwrap();
    let country = busy["code"].as_str().unwrap();
    let uri = format!("/api/places?{HERE}&country={country}");
    let (_, places) = call(&app, "GET", &uri, Some(&token), None).await;
    let (first, second) = (&places[0], &places[1]);
    let in_area = |place: &Value| format!("{}?{HERE}", visits_uri(place));

    // An area that doesn't exist: an error, and no visit is kept.
    let uri = format!("{}?area=demo-nowhere", visits_uri(first));
    let (status, _) = call(&app, "POST", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(discover(token.clone()).await["places_visited"], 0);

    // The first check-in: the country turns gold and the first badge is earned.
    let (status, result) = call(&app, "POST", &in_area(first), Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    let unlocked = result["unlocked"].as_array().unwrap();
    assert_eq!(unlocked[0]["kind"], "country");
    assert_eq!(unlocked[0]["country"], country);
    assert!(unlocked[0]["title"].as_str().unwrap().ends_with(": gold"));
    let more = busy["goal"].as_u64().unwrap() - 1;
    assert_eq!(unlocked[0]["text"], format!("{more} more for diamond."));
    let first_bite = badge(&before, "first-bite");
    let earned = unlocked.iter().find(|u| u["kind"] == "badge").unwrap();
    assert_eq!(earned["badge"], first_bite["kind"]);
    assert_eq!(
        earned["title"],
        format!("Badge earned: {}", first_bite["title"].as_str().unwrap())
    );
    assert_eq!(earned["text"], first_bite["description"]);
    // The progress is what discover says next, so clients need not ask.
    let after = discover(token.clone()).await;
    for part in [
        "countries",
        "badges",
        "profile",
        "events",
        "quests",
        "world",
    ] {
        assert_eq!(result["progress"][part], after[part], "{part}");
    }
    assert_eq!(result["profile"], after["profile"]);

    // Again the same day, or at another place of a country that stays gold: nothing new
    // for the country.
    let (_, again) = call(&app, "POST", &in_area(first), Some(&token), None).await;
    assert_eq!(again["unlocked"], serde_json::json!([]));
    let (_, next) = call(&app, "POST", &in_area(second), Some(&token), None).await;
    assert!(next["unlocked"]
        .as_array()
        .unwrap()
        .iter()
        .all(|u| u["kind"] != "country"));

    // Without an area there is nothing to compare.
    let other = session(&app).await;
    let (_, plain) = call(&app, "POST", &visits_uri(first), Some(&other), None).await;
    assert_eq!(plain["progress"], Value::Null);
    assert_eq!(plain["unlocked"], serde_json::json!([]));
}
