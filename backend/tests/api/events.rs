//! Events moving from upcoming to active, by the user's time zone.

use std::sync::atomic::Ordering;

use axum::http::StatusCode;
use mangiamundo::store;
use serde_json::Value;

use crate::common::{app_with, app_with_clock, call, session, visits_uri, DAY, HERE, START};

#[tokio::test]
async fn events_move_from_upcoming_to_active_and_unearned_ones_disappear() {
    let (app, clock) = app_with_clock().await;
    let token = session(&app).await;
    // Japan month runs through December 2026.
    let (_, discover) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&token),
        None,
    )
    .await;
    let japan = discover["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "japan-month-2026-12")
        .unwrap();
    assert_eq!(japan["status"], "upcoming");
    assert_eq!(japan["progress"], 0);

    clock.store(START + 20 * DAY, Ordering::Relaxed); // 6 December
    let (_, during) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&token),
        None,
    )
    .await;
    let events = during["events"].as_array().unwrap();
    let levant = events.iter().find(|e| e["id"] == "levant-month-2026-11");
    assert!(
        levant.is_none(),
        "an unearned event disappears once it is over"
    );
    let japan = events
        .iter()
        .find(|e| e["id"] == "japan-month-2026-12")
        .unwrap();
    assert_eq!(japan["status"], "active");
    assert_eq!(japan["target"], 3);
}

#[tokio::test]
async fn event_days_follow_the_users_time_zone() {
    // 1 December 2026, 08:00 in Tokyo: Japan month has begun there, but it is still
    // 30 November (23:00) in UTC.
    let morning_in_tokyo = 1_796_079_600;
    let db = store::connect("sqlite::memory:").await.unwrap();
    let (app, clock) = app_with(db, None, morning_in_tokyo);
    let token = session(&app).await;
    let (_, places) = call(
        &app,
        "GET",
        "/api/places?area=demo-tokyo&country=JP",
        Some(&token),
        None,
    )
    .await;
    let places = places.as_array().unwrap().clone();
    assert!(places.len() >= 2, "demo Tokyo has Japanese places");
    let visit = |i: usize, tz: &str| format!("{}{tz}", visits_uri(&places[i]));

    let japan = |discover: &Value| {
        discover["events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == "japan-month-2026-12")
            .unwrap()
            .clone()
    };
    let (_, before) = call(
        &app,
        "GET",
        "/api/discover?area=demo-tokyo&tz=Asia%2FTokyo",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(japan(&before)["status"], "active");
    let (_, before) = call(
        &app,
        "GET",
        "/api/discover?area=demo-tokyo",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(
        japan(&before)["status"],
        "upcoming",
        "without a zone, today is the UTC date"
    );

    // Checked in on 1 December, Tokyo time: it counts, whatever zone asks later.
    let (status, _) = call(
        &app,
        "POST",
        &visit(0, "?tz=Asia%2FTokyo"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    for tz in ["Asia%2FTokyo", "UTC", "America%2FLos_Angeles"] {
        let (_, now) = call(
            &app,
            "GET",
            &format!("/api/discover?area=demo-tokyo&tz={tz}"),
            Some(&token),
            None,
        )
        .await;
        assert_eq!(japan(&now)["progress"], 1, "progress seen from {tz}");
    }

    // 31 December 20:00 UTC, without a zone: dated 31 December, so it counts, even though
    // it was already 1 January in Tokyo (where the event, unearned, is now over).
    clock.store(1_798_747_200, Ordering::Relaxed);
    call(&app, "POST", &visit(1, ""), Some(&token), None).await;
    let (_, late) = call(
        &app,
        "GET",
        "/api/discover?area=demo-tokyo&tz=America%2FLos_Angeles",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(japan(&late)["progress"], 2);

    // An unknown zone is treated as UTC instead of failing.
    let (status, _) = call(
        &app,
        "GET",
        "/api/discover?area=demo-tokyo&tz=Etc%2FUnknown",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}
