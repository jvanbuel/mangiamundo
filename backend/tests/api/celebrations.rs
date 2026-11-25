//! Celebrations of countries found nearby, on the user's local day.

use axum::http::StatusCode;
use mangiamundo::store;
use serde_json::Value;

use crate::common::{app, app_with, call, HERE};

#[tokio::test]
async fn celebrations_only_list_countries_found_nearby() {
    let app = app().await;
    let (_, discover) = call(&app, "GET", &format!("/api/discover?{HERE}"), None, None).await;
    let here: Vec<&str> = discover["countries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["code"].as_str().unwrap())
        .collect();

    let (status, list) = call(
        &app,
        "GET",
        &format!("/api/celebrations?{HERE}&days=366"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let list = list.as_array().unwrap();
    assert_ne!(list.as_slice(), [] as [Value; 0]);
    let mut previous = String::new();
    for celebration in list {
        let starts = celebration["starts"].as_str().unwrap().to_string();
        assert!(starts >= previous, "sorted by date");
        assert!(celebration["ends"].as_str().unwrap() >= "2026-11-16");
        for nearby in celebration["nearby"].as_array().unwrap() {
            assert!(here.contains(&nearby["code"].as_str().unwrap()));
            assert!(nearby["places"].as_u64().unwrap() > 0);
        }
        assert_ne!(
            celebration["nearby"].as_array().unwrap().as_slice(),
            [] as [Value; 0]
        );
        assert!(celebration["reminder"]["title"]
            .as_str()
            .unwrap()
            .starts_with(celebration["title"].as_str().unwrap()));
        assert!(celebration["reminder"]["body"]
            .as_str()
            .unwrap()
            .contains("near you"));
        previous = starts;
    }
    // Thanksgiving is listed exactly when there is American food in the area.
    let thanksgiving = list.iter().any(|c| c["id"] == "thanksgiving");
    assert_eq!(thanksgiving, here.contains(&"US"));
}

#[tokio::test]
async fn celebrations_start_on_the_users_local_day() {
    use chrono::{NaiveDate, TimeZone};
    let tokyo: chrono_tz::Tz = "Asia/Tokyo".parse().unwrap();
    let app = app().await;
    // A one-day celebration with food in demo Tokyo, from 2027 so it is ahead of START.
    let (_, all) = call(
        &app,
        "GET",
        "/api/celebrations?area=demo-tokyo&days=366",
        None,
        None,
    )
    .await;
    let one_day = all
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["starts"] == c["ends"] && c["starts"].as_str().unwrap() >= "2027")
        .expect("a one-day celebration in demo Tokyo")
        .clone();
    let date: NaiveDate = one_day["starts"].as_str().unwrap().parse().unwrap();

    // 01:00 on that day in Tokyo, still the day before in UTC.
    let at = tokyo
        .from_local_datetime(&date.and_hms_opt(1, 0, 0).unwrap())
        .unwrap()
        .timestamp();
    let db = store::connect("sqlite::memory:").await.unwrap();
    let (app, _) = app_with(db, None, at);
    let ids = |list: &Value| {
        list.as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].clone())
            .collect::<Vec<_>>()
    };
    let (_, local) = call(
        &app,
        "GET",
        "/api/celebrations?area=demo-tokyo&days=0&tz=Asia%2FTokyo",
        None,
        None,
    )
    .await;
    assert!(ids(&local).contains(&one_day["id"]), "it is today in Tokyo");
    let (_, utc) = call(
        &app,
        "GET",
        "/api/celebrations?area=demo-tokyo&days=0",
        None,
        None,
    )
    .await;
    assert!(!ids(&utc).contains(&one_day["id"]), "it is tomorrow in UTC");
}
