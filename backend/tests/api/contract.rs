//! The contract fixtures and OpenAPI document the apps and CI check against.

use axum::http::StatusCode;
use mangiamundo::{api, store};
use serde_json::Value;

use crate::common::{app, app_with, at, call, session, state_with_notes, visits_uri, HERE, START};
use crate::fake_osm::{app_with_osm, osm_place};
use crate::shared_links::post_shared;

/// Writes real responses to `contract/`, where the Android and iOS tests decode them.
/// CI fails when the checked-in files differ, so an API change shows up in the diff.
#[tokio::test]
async fn writes_contract_fixtures() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../contract");
    let write = |name: &str, value: &Value| {
        std::fs::write(
            dir.join(format!("{name}.json")),
            serde_json::to_string_pretty(value).unwrap() + "\n",
        )
        .unwrap();
    };
    let db = store::connect("sqlite::memory:").await.unwrap();
    let unknown = store::UnknownPlace {
        id: "osm-node-1".into(),
        name: "Lalibela".into(),
        lat: 50.879,
        lng: 4.701,
        address: Some("Naamsestraat 12, Leuven".into()),
        distance_m: 0,
    };
    store::replace_unknown(&db, &[unknown]).await.unwrap();
    let (app, _) = app_with(db, None, START);
    let token = session(&app).await;
    let get = |uri: String| {
        let (app, token) = (&app, &token);
        async move {
            let (status, body) = call(app, "GET", &uri, Some(token), None).await;
            assert_eq!(status, StatusCode::OK, "{uri}");
            body
        }
    };

    // One verified and one unverified check-in, so progress, badges and places show both.
    // At least 3 places, so the two visits taste the country without completing it.
    let discover = get(format!("/api/discover?{HERE}")).await;
    let busy = discover["countries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["places"].as_u64().unwrap() >= 3)
        .unwrap();
    let country = busy["code"].as_str().unwrap().to_string();
    let places = get(format!("/api/places?{HERE}&country={country}")).await;
    let (first, second) = (&places[0], &places[1]);
    let (status, visit) = call(
        &app,
        "POST",
        &format!("{}?{HERE}", visits_uri(first)),
        Some(&token),
        Some(&at(first)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    call(&app, "POST", &visits_uri(second), Some(&token), None).await;

    let discover = get(format!("/api/discover?{HERE}")).await;
    write("countries", &get("/api/countries".into()).await);
    write("discover", &discover);
    write(
        "places",
        &get(format!("/api/places?{HERE}&country={country}")).await,
    );
    write(
        "celebrations",
        &get(format!("/api/celebrations?{HERE}&days=90")).await,
    );
    write(
        "reminders",
        &get(format!("/api/reminders?{HERE}&days=90")).await,
    );
    write("visit", &visit);
    write(
        "place",
        &get(format!(
            "/api/places/{}?lat=50.8798&lng=4.7005",
            first["id"].as_str().unwrap()
        ))
        .await,
    );
    write("areas", &get("/api/areas?q=leuv".into()).await);
    write(
        "unknown-places",
        &get(format!("/api/unknown-places?{HERE}&lat=50.8798&lng=4.7005")).await,
    );
    write(
        "located",
        &get("/api/areas/locate?lat=50.8798&lng=4.71".into()).await,
    );
    let (status, error) = call(&app, "GET", "/api/discover?area=demo-nowhere", None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    write("error", &error);
    let new = r#"{"name":"Kiwi","country":"NZ","lat":50.8798,"lng":4.7005,"note":false}"#;
    let (status, added) = call(&app, "POST", "/api/places", Some(&token), Some(new)).await;
    assert_eq!(status, StatusCode::OK);
    // Its id is random.
    let mut added = added;
    added["id"] = "user-0123456789abcdef0123456789abcdef".into();
    write("added", &added);
    let mut additions = get("/api/additions".into()).await;
    additions[0]["place_id"] = added["id"].clone();
    additions[0]["id"] = added["id"].clone();
    write("additions", &additions);

    // Taking back the unverified check-in, after everything that shows it is written.
    let (_, places) = call(
        &app,
        "GET",
        &format!("/api/places/{}", second["id"].as_str().unwrap()),
        Some(&token),
        None,
    )
    .await;
    let uri = format!("/api/visits/{}?{HERE}", places["visits"][0]["id"]);
    let (status, removed) = call(&app, "DELETE", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    write("visit-removed", &removed);

    // Two devices, one with a name, and a recovery code. Ids and times are random or real.
    call(
        &app,
        "POST",
        "/api/account/recovery-code",
        Some(&token),
        None,
    )
    .await;
    let (_, link) = call(&app, "POST", "/api/account/link-code", Some(&token), None).await;
    let body = format!(
        r#"{{"code":"{}","device":"iPhone"}}"#,
        link["code"].as_str().unwrap()
    );
    assert_eq!(
        call(&app, "POST", "/api/session/restore", None, Some(&body))
            .await
            .0,
        StatusCode::OK
    );
    let mut account = get("/api/account".into()).await;
    account["created_at"] = START.into();
    account["recovery_code_created_at"] = (START + 60).into();
    for (i, device) in account["devices"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        device["id"] = format!("{i}0123456789abcdef0123456789abcde").into();
        device["created_at"] = START.into();
        device["last_used_at"] = (START + 120).into();
    }
    write("account", &account);
    write(
        "link-code",
        &serde_json::json!({ "code": "ABCD-EFGH", "expires_at": START + 600 }),
    );

    // The server's version is real; the commit is only set in a Docker build.
    let mut health = get("/api/health".into()).await;
    health["commit"] = "0123abc".into();
    write("health", &health);

    // Tokens, recovery codes and the slugs of shared pages are random.
    let (status, mut new) = call(&app, "POST", "/api/session", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(new["token"].is_string());
    new["token"] = "0123456789abcdef0123456789abcdef".into();
    write("session", &new);
    let uri = "/api/account/recovery-code";
    let (status, mut recovery) = call(&app, "POST", uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(recovery["code"].is_string());
    recovery["code"] = "ABCD-EFGH-JKLM-NPQR-STUV".into();
    write("recovery-code", &recovery);
    let (status, mut link) = post_shared(&app, &format!("/api/share?{HERE}"), &token).await;
    assert_eq!(status, StatusCode::OK);
    let page = "https://mangiamundo.test/p/";
    assert!(link["url"].as_str().unwrap().starts_with(page));
    link["url"] = format!("{page}abcd2345ef").into();
    link["image_url"] = format!("{page}abcd2345ef/card.png").into();
    write("share-link", &link);

    // A suggestion is drawn at random: with every other place excluded, one is left.
    let everywhere = get(format!("/api/places?{HERE}")).await;
    let everywhere = everywhere.as_array().unwrap();
    let id = |p: &Value| p["id"].as_str().unwrap().to_string();
    let only = everywhere
        .iter()
        .find(|p| {
            p["visits"].as_array().unwrap().is_empty()
                && p["countries"].as_array().unwrap().len() == 1
                && p["hours"]["status"]["open"] == true
                && !id(p).starts_with("user-")
        })
        .map(id)
        .unwrap();
    let others: Vec<String> = everywhere.iter().map(id).filter(|i| *i != only).collect();
    let suggestion = get(format!(
        "/api/suggestion?{HERE}&exclude={}",
        others.join(",")
    ))
    .await;
    assert_eq!(suggestion["place"]["id"], only.as_str());
    write("suggestion", &suggestion);

    // An area that is loading: the fetch is begun by hand, and so slow it stays where it began.
    let db = store::connect("sqlite::memory:").await.unwrap();
    let (state, _) = state_with_notes(db, None, None, START);
    let loads = state.loads.clone();
    let _loading = loads.begin("demo-leuven");
    loads.step("demo-leuven", 15.0, 95.0, "Looking for restaurants", 1e9);
    let loading = api::router(state);
    let (status, progress) = call(
        &loading,
        "GET",
        "/api/areas/demo-leuven/progress",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    write("area-progress", &progress);

    // Reading again from OpenStreetMap: a place that got hours there, and one mapped since
    // that the user had added.
    let (osm, _, fake, _) = app_with_osm().await;
    let token = session(&osm).await;
    let mut edited = osm_place(42, "Lalibela", "ethiopian", 50.8798, 4.7005);
    edited.opening_hours = Some("Mo-Su 12:00-22:00".into());
    let found = mangiamundo::providers::Lookup::Found(edited);
    fake.places
        .lock()
        .unwrap()
        .insert("osm-node-42".into(), found);
    let uri = "/api/places/osm-node-42/refresh?lat=50.88&lng=4.7";
    let (status, refreshed) = call(&osm, "POST", uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    write("refresh", &refreshed);
    let new = r#"{"name":"Inti","country":"PE","lat":50.8801,"lng":4.7009,"note":false}"#;
    let (status, _) = call(&osm, "POST", "/api/places", Some(&token), Some(new)).await;
    assert_eq!(status, StatusCode::OK);
    *fake.nearby.lock().unwrap() = vec![
        osm_place(42, "Lalibela", "ethiopian", 50.8798, 4.7005),
        osm_place(43, "Restaurant Inti", "peruvian", 50.8801, 4.7009),
    ];
    let uri = "/api/places/nearby/refresh?lat=50.88&lng=4.7";
    let (status, nearby) = call(&osm, "POST", uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    write("nearby", &nearby);

    // What a newer server might send: enum values this app version doesn't know.
    let mut newer = discover.clone();
    newer["badges"][0]["kind"] = "something-new".into();
    newer["countries"][0]["status"] = "ruby".into();
    newer["events"][0]["status"] = "paused".into();
    newer["unknown_field"] = true.into();
    write("discover-newer", &newer);
}

/// Writes the OpenAPI document to `contract/`, so CI shows API changes in the diff.
/// It must match what the server serves.
#[tokio::test]
async fn serves_and_writes_the_openapi_document() {
    let app = app().await;
    let (status, doc) = call(&app, "GET", "/api/openapi.json", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(doc["openapi"].as_str().unwrap().get(..3), Some("3.1"));
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../contract/openapi.json");
    std::fs::write(path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
}
