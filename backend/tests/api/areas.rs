//! Finding areas, measuring distances in them, and loading their progress.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

use crate::common::{app, call, HERE};

#[tokio::test]
async fn a_postal_code_holds_part_of_its_city() {
    let app = app().await;
    let total = |d: &Value| {
        d["countries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["places"].as_u64().unwrap())
            .sum::<u64>()
    };
    let (_, city) = call(&app, "GET", "/api/discover?area=demo-leuven", None, None).await;
    let (status, west) = call(
        &app,
        "GET",
        "/api/discover?area=demo-leuven-3000",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(west["area"]["kind"], "postal_code");
    let (_, east) = call(
        &app,
        "GET",
        "/api/discover?area=demo-leuven-3010",
        None,
        None,
    )
    .await;
    assert!(total(&west) > 0 && total(&east) > 0);
    assert_eq!(total(&west) + total(&east), total(&city));
}

#[tokio::test]
async fn areas_can_be_searched_and_located() {
    let app = app().await;
    let (status, found) = call(&app, "GET", "/api/areas?q=leuv", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(found[0]["id"], "demo-leuven");
    assert_eq!(found[0]["kind"], "city");
    assert_eq!(found[0]["detail"], "Belgium");
    let (_, codes) = call(&app, "GET", "/api/areas?q=3010", None, None).await;
    assert_eq!(codes[0]["name"], "3010 Leuven");
    assert_eq!(codes[0]["kind"], "postal_code");
    let (status, suggested) = call(
        &app,
        "GET",
        "/api/areas/suggest?q=leu&lat=50.88&lng=4.7",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(suggested[0]["id"], "demo-leuven");
    let (_, here) = call(
        &app,
        "GET",
        "/api/areas/locate?lat=50.8798&lng=4.71",
        None,
        None,
    )
    .await;
    assert_eq!(here["city"]["id"], "demo-leuven");
    assert_eq!(here["postal_code"]["id"], "demo-leuven-3010");
    let (_, nowhere) = call(&app, "GET", "/api/areas/locate?lat=0&lng=0", None, None).await;
    assert!(nowhere["city"].is_null());
}

#[tokio::test]
async fn distances_are_measured_from_the_user_when_given() {
    let app = app().await;
    let (_, discover) = call(&app, "GET", &format!("/api/discover?{HERE}"), None, None).await;
    let country = discover["countries"][0]["code"]
        .as_str()
        .unwrap()
        .to_string();
    let (_, places) = call(
        &app,
        "GET",
        &format!("/api/places?{HERE}&country={country}"),
        None,
        None,
    )
    .await;
    let first = &places[0];
    let uri = format!(
        "/api/places?{HERE}&country={country}&lat={}&lng={}",
        first["lat"], first["lng"]
    );
    let (_, from_there) = call(&app, "GET", &uri, None, None).await;
    let same = from_there
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == first["id"])
        .unwrap();
    assert_eq!(same["distance_m"], 0);
}

#[tokio::test]
async fn an_area_that_is_not_loading_has_no_progress() {
    let response = app()
        .await
        .oneshot(
            Request::get("/api/areas/demo-leuven/progress")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"null");
}
