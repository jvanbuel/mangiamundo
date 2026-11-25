//! End-to-end tests of the HTTP API against the offline demo provider.

mod added_places;
mod additions_xp;
mod anonymous;
mod area_lookups;
mod areas;
mod celebrations;
mod closed_reports;
mod common;
mod completion;
mod contract;
mod errors;
mod events;
mod fake_osm;
mod shared_links;
mod suggestions;
mod unknown_food;
mod visit_counts;
mod visits;

use std::sync::atomic::Ordering;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use mangiamundo::store;
use serde_json::Value;
use tower::ServiceExt;

use crate::common::{
    app, app_with, app_with_clock, assert_error, call, session, visits_uri, DAY, HERE, START,
};
use crate::fake_osm::{app_with_osm, countries_here, osm_place};
use crate::shared_links::{fetch, path_of, post_shared};

#[tokio::test]
async fn a_place_can_be_refreshed_from_osm_before_the_next_import() {
    use mangiamundo::providers::Lookup;
    let (app, clock, fake, db) = app_with_osm().await;
    let token = session(&app).await;
    let refresh = |id: &'static str| {
        let (app, token) = (app.clone(), token.clone());
        async move {
            call(
                &app,
                "POST",
                &format!("/api/places/{id}/refresh"),
                Some(&token),
                None,
            )
            .await
        }
    };
    let set = |lookup: Lookup| {
        fake.places
            .lock()
            .unwrap()
            .insert("osm-node-42".into(), lookup);
    };

    // The same as we have.
    set(Lookup::Found(osm_place(
        42,
        "Lalibela",
        "ethiopian",
        50.8798,
        4.7005,
    )));
    let (status, body) = refresh("osm-node-42").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "unchanged");
    assert_eq!(body["place"]["name"], "Lalibela");

    // Hours added on OSM: in the place, and in the local copy the next area rebuild reads.
    let mut edited = osm_place(42, "Lalibela", "ethiopian", 50.8798, 4.7005);
    edited.opening_hours = Some("Mo-Su 12:00-22:00".into());
    set(Lookup::Found(edited));
    let (_, body) = refresh("osm-node-42").await;
    assert_eq!(body["status"], "updated");
    assert_eq!(body["place"]["hours"]["raw"], "Mo-Su 12:00-22:00");
    assert_eq!(
        body["place"]["hours"]["week"].as_array().map(Vec::len),
        Some(7),
        "with the week, like the details"
    );
    let copy: Option<String> =
        sqlx::query_scalar("SELECT opening_hours FROM osm_places WHERE id = 'osm-node-42'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(copy.as_deref(), Some("Mo-Su 12:00-22:00"));

    // Checked in, then the place closes on OSM: hidden from every list, visits kept.
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/places/osm-node-42/visits",
            Some(&token),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(countries_here(&app, &token)
        .await
        .contains(&"ET".to_string()));
    set(Lookup::Gone);
    let (_, body) = refresh("osm-node-42").await;
    assert_eq!(
        (body["status"].as_str(), &body["place"]),
        (Some("gone"), &Value::Null)
    );
    assert!(!countries_here(&app, &token)
        .await
        .contains(&"ET".to_string()));
    let (status, place) = call(&app, "GET", "/api/places/osm-node-42", Some(&token), None).await;
    assert_eq!(
        (status, place["visits"].as_array().unwrap().len()),
        (StatusCode::OK, 1)
    );
    let copies: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM osm_places")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(copies, 0, "the next area rebuild must not bring it back");

    // Reopened: listed again.
    set(Lookup::Found(osm_place(
        42,
        "Lalibela",
        "ethiopian",
        50.8798,
        4.7005,
    )));
    assert_eq!(refresh("osm-node-42").await.1["status"], "updated");
    assert!(countries_here(&app, &token)
        .await
        .contains(&"ET".to_string()));

    // Five a minute.
    assert_eq!(refresh("osm-node-42").await.0, StatusCode::OK);
    let (status, error) = refresh("osm-node-42").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_error(&error, "too_many_requests");
    clock.fetch_add(61, Ordering::Relaxed);

    // Moved out of Leuven on OSM: no longer listed there.
    set(Lookup::Found(osm_place(
        42,
        "Lalibela",
        "ethiopian",
        51.2194,
        4.4025,
    )));
    assert_eq!(refresh("osm-node-42").await.1["status"], "updated");
    assert!(!countries_here(&app, &token)
        .await
        .contains(&"ET".to_string()));

    // Only OSM places this user can see, and OSM must answer.
    assert_eq!(
        call(&app, "POST", "/api/places/osm-node-42/refresh", None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(refresh("user-abc").await.0, StatusCode::BAD_REQUEST);
    assert_eq!(refresh("osm-node-999").await.0, StatusCode::NOT_FOUND);
    fake.down.store(true, Ordering::Relaxed);
    let (status, error) = refresh("osm-node-42").await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_error(&error, "upstream");
}

#[tokio::test]
async fn places_just_mapped_nearby_are_added_and_confirm_pending_additions() {
    let (app, clock, fake, _db) = app_with_osm().await;
    let (alice, bob) = (session(&app).await, session(&app).await);
    // Alice added Inti, a Peruvian place, before a mapper put it on OSM.
    let body = r#"{"name":"Inti","country":"PE","lat":50.8801,"lng":4.7009,"note":false}"#;
    assert_eq!(
        call(&app, "POST", "/api/places", Some(&alice), Some(body))
            .await
            .0,
        StatusCode::OK
    );
    assert!(!countries_here(&app, &bob).await.contains(&"PE".to_string()));

    *fake.nearby.lock().unwrap() = vec![
        osm_place(42, "Lalibela", "ethiopian", 50.8798, 4.7005),
        osm_place(43, "Restaurant Inti", "peruvian", 50.8801, 4.7009),
        osm_place(44, "Far Away", "greek", 10.0, 10.0),
    ];
    let look = |token: String| {
        let app = app.clone();
        async move {
            call(
                &app,
                "POST",
                "/api/places/nearby/refresh?lat=50.88&lng=4.7",
                Some(&token),
                None,
            )
            .await
        }
    };
    let (status, body) = look(bob.clone()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let added: Vec<&str> = body["added"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        added,
        vec!["Restaurant Inti"],
        "already listed, and outside every cached area, don't count"
    );
    assert_eq!(body["matched"], 1);
    assert!(
        countries_here(&app, &bob).await.contains(&"PE".to_string()),
        "for everyone now"
    );
    let (_, additions) = call(&app, "GET", "/api/additions", Some(&alice), None).await;
    assert_eq!(
        (
            additions[0]["status"].as_str(),
            additions[0]["place_id"].as_str()
        ),
        (Some("confirmed"), Some("osm-node-43"))
    );

    // Looking again finds nothing new.
    let (_, body) = look(bob.clone()).await;
    assert_eq!(
        (
            body["added"].as_array().unwrap().len(),
            body["matched"].as_u64()
        ),
        (0, Some(0))
    );

    // Five an hour, per user.
    for _ in 0..3 {
        assert_eq!(look(bob.clone()).await.0, StatusCode::OK);
    }
    assert_eq!(look(bob.clone()).await.0, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(look(alice.clone()).await.0, StatusCode::OK);
    clock.fetch_add(3601, Ordering::Relaxed);
    assert_eq!(look(bob.clone()).await.0, StatusCode::OK);
    let bad = call(
        &app,
        "POST",
        "/api/places/nearby/refresh?lat=91&lng=4.7",
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(bad.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_recovery_code_restores_the_account_on_a_new_device() {
    let app = app().await;
    let old = session(&app).await;
    let places = call(
        &app,
        "GET",
        &format!("/api/places?{HERE}"),
        Some(&old),
        None,
    )
    .await
    .1;
    let (first, second) = (&places[0], &places[1]);
    assert_eq!(
        call(&app, "POST", &visits_uri(first), Some(&old), None)
            .await
            .0,
        StatusCode::OK
    );

    let (status, account) = call(&app, "GET", "/api/account", Some(&old), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(account["recovery_code_created_at"].is_null());
    assert_eq!(account["devices"].as_array().unwrap().len(), 1);
    assert_eq!(account["devices"][0]["current"], true);

    let (status, body) = call(&app, "POST", "/api/account/recovery-code", Some(&old), None).await;
    assert_eq!(status, StatusCode::OK);
    let code = body["code"].as_str().unwrap().to_string();
    assert_eq!(code.len(), 24);

    // A new phone already checked in once before restoring: that visit is kept.
    let new = session(&app).await;
    assert_eq!(
        call(&app, "POST", &visits_uri(second), Some(&new), None)
            .await
            .0,
        StatusCode::OK
    );
    let typed = code.to_lowercase().replace('-', " ");
    let body = format!(r#"{{"code":"{typed}","device":"iPhone"}}"#);
    let (status, restored) = call(
        &app,
        "POST",
        "/api/session/restore",
        Some(&new),
        Some(&body),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let restored = restored["token"].as_str().unwrap().to_string();

    // The new phone's anonymous session is gone, its visit moved over.
    assert_eq!(
        call(&app, "GET", "/api/account", Some(&new), None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (_, profile) = call(
        &app,
        "GET",
        &format!("/api/discover?{HERE}"),
        Some(&restored),
        None,
    )
    .await;
    assert_eq!(profile["profile"]["visits"], 2, "{profile}");

    let (_, account) = call(&app, "GET", "/api/account", Some(&restored), None).await;
    let devices = account["devices"].as_array().unwrap();
    assert_eq!(devices.len(), 2);
    assert!(account["recovery_code_created_at"].is_i64());
    let current = devices.iter().find(|d| d["current"] == true).unwrap();
    assert_eq!(current["name"], "iPhone");

    // The code keeps working until a new one replaces it.
    let again = format!(r#"{{"code":"{code}"}}"#);
    assert_eq!(
        call(&app, "POST", "/api/session/restore", None, Some(&again))
            .await
            .0,
        StatusCode::OK
    );
    call(&app, "POST", "/api/account/recovery-code", Some(&old), None).await;
    let (status, error) = call(&app, "POST", "/api/session/restore", None, Some(&again)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_error(&error, "not_found");
}

#[tokio::test]
async fn a_link_code_signs_in_one_device_once_within_ten_minutes() {
    let (app, clock) = app_with_clock().await;
    let phone = session(&app).await;
    let (status, link) = call(&app, "POST", "/api/account/link-code", Some(&phone), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(link["expires_at"], START + 600);
    let body = format!(
        r#"{{"code":"{}","device":"Web"}}"#,
        link["code"].as_str().unwrap()
    );

    let (status, web) = call(&app, "POST", "/api/session/restore", None, Some(&body)).await;
    assert_eq!(status, StatusCode::OK);
    let web = web["token"].as_str().unwrap().to_string();
    assert_eq!(
        call(&app, "POST", "/api/session/restore", None, Some(&body))
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    let (_, account) = call(&app, "GET", "/api/account", Some(&web), None).await;
    assert_eq!(account["devices"].as_array().unwrap().len(), 2);

    let (_, link) = call(&app, "POST", "/api/account/link-code", Some(&phone), None).await;
    let body = format!(r#"{{"code":"{}"}}"#, link["code"].as_str().unwrap());
    clock.store(START + 601, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(
        call(&app, "POST", "/api/session/restore", None, Some(&body))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn devices_can_be_signed_out_and_the_account_deleted() {
    let app = app().await;
    let phone = session(&app).await;
    let (_, link) = call(&app, "POST", "/api/account/link-code", Some(&phone), None).await;
    let body = format!(r#"{{"code":"{}"}}"#, link["code"].as_str().unwrap());
    let laptop = call(&app, "POST", "/api/session/restore", None, Some(&body))
        .await
        .1["token"]
        .as_str()
        .unwrap()
        .to_string();

    let (_, account) = call(&app, "GET", "/api/account", Some(&phone), None).await;
    let other = account["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["current"] == false)
        .unwrap();
    let uri = format!("/api/account/devices/{}", other["id"].as_str().unwrap());
    // Someone else can't sign a device of this account out.
    let stranger = session(&app).await;
    assert_eq!(
        call(&app, "DELETE", &uri, Some(&stranger), None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&app, "DELETE", &uri, Some(&phone), None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&app, "GET", "/api/account", Some(&laptop), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        call(&app, "DELETE", "/api/account", Some(&phone), None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&app, "GET", "/api/account", Some(&phone), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&app, "GET", "/api/account", Some(&stranger), None)
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn link_codes_are_limited_per_hour() {
    let app = app().await;
    let token = session(&app).await;
    for _ in 0..10 {
        assert_eq!(
            call(&app, "POST", "/api/account/link-code", Some(&token), None)
                .await
                .0,
            StatusCode::OK
        );
    }
    let (status, error) = call(&app, "POST", "/api/account/link-code", Some(&token), None).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_error(&error, "too_many_requests");
}

/// `POST /api/session` as Fly's proxy passes it on from `address`.
async fn session_from(app: &Router, address: &str) -> StatusCode {
    let request = Request::builder()
        .method("POST")
        .uri("/api/session")
        .header("fly-client-ip", address)
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(request).await.unwrap().status()
}

#[tokio::test]
async fn new_accounts_are_limited_per_address_and_hour() {
    let (app, now) = app_with_clock().await;
    for _ in 0..20 {
        assert_eq!(session_from(&app, "203.0.113.7").await, StatusCode::OK);
    }
    assert_eq!(
        session_from(&app, "203.0.113.7").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    let (status, error) = call(&app, "GET", "/api/health", None, None).await;
    assert_eq!((status, &error["ok"]), (StatusCode::OK, &Value::Bool(true)));
    // Someone else is not held up, and one connection's IPv6 addresses count as one.
    assert_eq!(session_from(&app, "203.0.113.8").await, StatusCode::OK);
    for _ in 0..20 {
        assert_eq!(session_from(&app, "2001:db8:1:2::1").await, StatusCode::OK);
    }
    assert_eq!(
        session_from(&app, "2001:db8:1:2:a:b:c:d").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(session_from(&app, "2001:db8:1:3::1").await, StatusCode::OK);
    // The accounts that were made keep working, and an hour later there is room again.
    now.fetch_add(3600, Ordering::Relaxed);
    assert_eq!(session_from(&app, "203.0.113.7").await, StatusCode::OK);
}

/// Without the proxy's header (tests, development) only the limit for everyone applies.
#[tokio::test]
async fn new_accounts_are_limited_for_everyone_together() {
    let app = app().await;
    for _ in 0..1000 {
        let (status, _) = call(&app, "POST", "/api/session", None, None).await;
        assert_eq!(status, StatusCode::OK);
    }
    let (status, error) = call(&app, "POST", "/api/session", None, None).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_error(&error, "too_many_requests");
}

/// A restore that read its session before another restore moved that session to a new
/// account merges that new account, not the stale one.
#[tokio::test]
async fn a_restore_merges_the_account_its_session_belongs_to_now() {
    use mangiamundo::account;
    let db = store::connect("sqlite::memory:").await.unwrap();
    let user = |token: String| {
        let db = db.clone();
        async move {
            account::session_for_token(&db, &token, START)
                .await
                .unwrap()
                .unwrap()
        }
    };
    let stale = user(account::create_user(&db, None, START).await.unwrap()).await;
    let moved_to = user(account::create_user(&db, None, START).await.unwrap())
        .await
        .user;
    let target = user(account::create_user(&db, None, START).await.unwrap())
        .await
        .user;
    // The other restore, done meanwhile: this session now belongs to `moved_to`.
    sqlx::query("UPDATE sessions SET user_id = ? WHERE id = ?")
        .bind(&moved_to)
        .bind(&stale.id)
        .execute(&db)
        .await
        .unwrap();
    let code = account::new_recovery_code(&db, &target, START)
        .await
        .unwrap();
    account::redeem(&db, &code, Some(&stale), None, START)
        .await
        .unwrap()
        .unwrap();
    let left: Option<String> = sqlx::query_scalar("SELECT id FROM users WHERE id = ?")
        .bind(&moved_to)
        .fetch_optional(&db)
        .await
        .unwrap();
    assert_eq!(
        left, None,
        "`moved_to` was merged into the restored account"
    );
}

#[tokio::test]
async fn sharing_progress_makes_a_public_page_that_can_be_turned_off() {
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
    let (_, places) = call(
        &app,
        "GET",
        &format!("/api/places?{HERE}&country={country}"),
        Some(&token),
        None,
    )
    .await;
    let (status, _) = call(&app, "POST", &visits_uri(&places[0]), Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);

    let (status, link) = post_shared(&app, &format!("/api/share?{HERE}"), &token).await;
    assert_eq!(status, StatusCode::OK);
    assert!(link["text"]
        .as_str()
        .unwrap()
        .contains("I've tasted food from 1 of the"));
    let page = path_of(&link["url"]);
    assert_eq!(path_of(&link["image_url"]), format!("{page}/card.png"));

    // Sharing again keeps the link.
    let (_, again) = post_shared(&app, &format!("/api/share?{HERE}"), &token).await;
    assert_eq!(again["url"], link["url"]);
    let (_, account) = call(&app, "GET", "/api/account", Some(&token), None).await;
    assert_eq!(account["public_profile"].as_str(), Some(page.as_str()));

    // Anyone can open the page and its card, which shows no places.
    let (status, kind, html) = fetch(&app, &page).await;
    assert_eq!(status, StatusCode::OK);
    assert!(kind.starts_with("text/html"));
    let html = String::from_utf8(html).unwrap();
    assert!(html.contains(r#"<meta property="og:image" content="https://mangiamundo.test/p/"#));
    assert!(html.contains("1 of "));
    assert!(
        !html.contains(places[0]["name"].as_str().unwrap()),
        "no place names on the public page"
    );
    let (status, kind, png) = fetch(&app, &format!("{page}/card.png")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(kind, "image/png");
    assert!(png.starts_with(b"\x89PNG"));

    // Turned off, the link is gone.
    let (status, _) = call(&app, "DELETE", "/api/share", Some(&token), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(fetch(&app, &page).await.0, StatusCode::NOT_FOUND);
    let (_, account) = call(&app, "GET", "/api/account", Some(&token), None).await;
    assert!(account["public_profile"].is_null());
}

#[tokio::test]
async fn a_shared_check_in_has_its_own_page_until_it_is_removed() {
    let app = app().await;
    let token = session(&app).await;
    let other = session(&app).await;
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
    let (_, places) = call(
        &app,
        "GET",
        &format!("/api/places?{HERE}&country={country}"),
        Some(&token),
        None,
    )
    .await;
    let (_, result) = call(&app, "POST", &visits_uri(&places[0]), Some(&token), None).await;
    let visit = result["visit"]["id"].as_i64().unwrap();

    // Only its owner can share it.
    let (status, _) = post_shared(&app, &format!("/api/visits/{visit}/share"), &other).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, link) = post_shared(&app, &format!("/api/visits/{visit}/share"), &token).await;
    assert_eq!(status, StatusCode::OK);
    let name = places[0]["name"].as_str().unwrap();
    assert!(link["text"].as_str().unwrap().contains(name));
    let (_, again) = post_shared(&app, &format!("/api/visits/{visit}/share"), &token).await;
    assert_eq!(again["url"], link["url"], "the same link each time");

    let page = path_of(&link["url"]);
    let (status, _, html) = fetch(&app, &page).await;
    assert_eq!(status, StatusCode::OK);
    assert!(String::from_utf8(html).unwrap().contains(name));
    let (status, kind, _) = fetch(&app, &format!("{page}/card.png")).await;
    assert_eq!((status, kind.as_str()), (StatusCode::OK, "image/png"));

    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/api/visits/{visit}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetch(&app, &page).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn monthly_quests_pay_once_and_the_streak_shows() {
    let app = app().await;
    let token = session(&app).await;
    let discover = |app: Router, token: String| async move {
        call(
            &app,
            "GET",
            &format!("/api/discover?{HERE}"),
            Some(&token),
            None,
        )
        .await
        .1
    };
    let before = discover(app.clone(), token.clone()).await;
    let check_ins = |d: &Value| {
        d["quests"]["quests"]
            .as_array()
            .unwrap()
            .iter()
            .find(|q| q["id"] == "check-ins")
            .cloned()
            .expect("check-ins quest")
    };
    assert_eq!(before["quests"]["month"], "2026-11");
    assert_eq!(check_ins(&before)["done"], false);
    assert_eq!(before["profile"]["streak"], 0);

    let (_, places) = call(
        &app,
        "GET",
        &format!("/api/places?{HERE}"),
        Some(&token),
        None,
    )
    .await;
    let places = places.as_array().unwrap();
    let uri = format!("{}?{HERE}", visits_uri(&places[0]));
    let (_, first) = call(&app, "POST", &uri, Some(&token), None).await;
    // Looking doesn't pay anything, however often.
    let between = discover(app.clone(), token.clone()).await;
    assert_eq!(check_ins(&between)["done"], false);
    assert_eq!(between["profile"]["xp"], first["profile"]["xp"]);
    // The check-in that finishes a quest answers with its XP, with or without an area.
    let (_, second) = call(&app, "POST", &visits_uri(&places[1]), Some(&token), None).await;
    let xp = second["profile"]["xp"].as_u64().unwrap();
    assert!(xp >= first["profile"]["xp"].as_u64().unwrap() + 10 + 50);
    let after = discover(app.clone(), token.clone()).await;
    assert_eq!(check_ins(&after)["done"], true);
    assert_eq!(after["profile"]["streak"], 1);
    assert_eq!(after["profile"]["xp"].as_u64().unwrap(), xp);
    // Asking again doesn't pay the quest twice, and neither does checking in again.
    let again = discover(app.clone(), token.clone()).await;
    assert_eq!(again["profile"]["xp"].as_u64().unwrap(), xp);
    let (_, repeat) = call(&app, "POST", &uri, Some(&token), None).await;
    assert_eq!(repeat["profile"]["xp"].as_u64().unwrap(), xp);
    assert_eq!(repeat["progress"]["profile"]["xp"].as_u64().unwrap(), xp);
    let quest_xp: u64 = after["quests"]["quests"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|q| q["done"] == true)
        .map(|q| q["xp"].as_u64().unwrap())
        .sum();
    assert!(quest_xp >= 50 && xp >= quest_xp + 2 * 10);
}

#[tokio::test]
async fn quests_are_paid_per_area_at_check_in_and_taken_back_with_it() {
    let db = store::connect("sqlite::memory:").await.unwrap();
    let (app, _) = app_with(db.clone(), None, START);
    let token = session(&app).await;
    // Who was paid for "two check-ins", per area.
    let paid = || async {
        sqlx::query_scalar::<_, String>(
            "SELECT area_id FROM quest_rewards WHERE quest = 'check-ins' ORDER BY area_id",
        )
        .fetch_all(&db)
        .await
        .unwrap()
    };
    let done = |app: Router, token: String, area: &'static str| async move {
        let uri = format!("/api/discover?area={area}");
        let (_, d) = call(&app, "GET", &uri, Some(&token), None).await;
        let quests = d["quests"]["quests"].as_array().unwrap().clone();
        let q = quests.into_iter().find(|q| q["id"] == "check-ins").unwrap();
        (q["progress"].as_u64().unwrap(), q["done"] == true)
    };
    // A postal code of Leuven, the city around it, and another city: all three cached.
    let (_, west) = call(&app, "GET", "/api/places?area=demo-leuven-3000", None, None).await;
    let (_, tokyo) = call(&app, "GET", "/api/places?area=demo-tokyo", None, None).await;
    call(&app, "GET", &format!("/api/discover?{HERE}"), None, None).await;
    let (west, tokyo) = (west.as_array().unwrap(), tokyo.as_array().unwrap());

    // Two check-ins in the postal code, made while looking at the city, finish the quest
    // in both: the places are in both.
    let mut visits = Vec::new();
    for place in &west[..2] {
        let uri = format!("{}?{HERE}", visits_uri(place));
        let (status, body) = call(&app, "POST", &uri, Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        visits.push(body);
    }
    assert_eq!(paid().await, ["demo-leuven", "demo-leuven-3000"]);
    let xp = visits[1]["profile"]["xp"].as_u64().unwrap();
    assert!(xp >= 2 * 10 + 2 * 50);
    assert_eq!(visits[1]["progress"]["profile"]["xp"].as_u64().unwrap(), xp);
    for area in ["demo-leuven", "demo-leuven-3000"] {
        assert_eq!(done(app.clone(), token.clone(), area).await, (2, true));
    }

    // They don't count in Tokyo, which has the quest too: one check-in there is one of two.
    assert_eq!(
        done(app.clone(), token.clone(), "demo-tokyo").await,
        (0, false)
    );
    let uri = format!("{}?area=demo-tokyo", visits_uri(&tokyo[0]));
    call(&app, "POST", &uri, Some(&token), None).await;
    assert_eq!(
        done(app.clone(), token.clone(), "demo-tokyo").await,
        (1, false)
    );
    assert_eq!(paid().await, ["demo-leuven", "demo-leuven-3000"]);
    let uri = format!("{}?area=demo-tokyo", visits_uri(&tokyo[1]));
    call(&app, "POST", &uri, Some(&token), None).await;
    assert_eq!(
        paid().await,
        ["demo-leuven", "demo-leuven-3000", "demo-tokyo"]
    );

    // Taking a check-in back takes back the quests it finished, where it finished them.
    let uri = format!("/api/visits/{}", visits[0]["visit"]["id"]);
    let (status, _) = call(&app, "DELETE", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(paid().await, ["demo-tokyo"]);
    assert_eq!(
        done(app.clone(), token.clone(), "demo-leuven").await,
        (1, false)
    );
    // And checking in again pays them again, once.
    let uri = visits_uri(&west[0]);
    let (_, back) = call(&app, "POST", &uri, Some(&token), None).await;
    assert_eq!(
        paid().await,
        ["demo-leuven", "demo-leuven-3000", "demo-tokyo"]
    );
    let (_, d) = call(
        &app,
        "GET",
        "/api/discover?area=demo-tokyo",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(d["profile"]["xp"], back["profile"]["xp"]);
}

#[tokio::test]
async fn planning_reminders_books_no_quest_rewards() {
    // 1 December 2026, 12:00 UTC: the day the quests reminder is due.
    let first = START + 15 * DAY;
    let db = store::connect("sqlite::memory:").await.unwrap();
    let (app, _) = app_with(db.clone(), None, first);
    let token = session(&app).await;
    let (_, places) = call(&app, "GET", &format!("/api/places?{HERE}"), None, None).await;
    let booked = || async {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM quest_rewards")
            .fetch_one(&db)
            .await
            .unwrap()
    };
    assert_eq!(booked().await, 0);
    // Two check-ins finish quests, which are booked right then, also without an area.
    for place in &places.as_array().unwrap()[..2] {
        let (status, _) = call(&app, "POST", &visits_uri(place), Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
    }
    let finished = booked().await;
    assert!(finished > 0);

    let uri = format!("/api/reminders?{HERE}&days=366");
    let (status, plan) = call(&app, "GET", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    let plan = plan.as_array().unwrap();
    let quests = &plan[0];
    assert_eq!(quests["key"], "quests:2026-12");
    assert_eq!(quests["local_date"], "2026-12-01");
    assert_eq!(quests["hour"], 10);
    assert_eq!(quests["title"], "New quests this month");
    assert_eq!(quests["url"], "/?tab=badges");
    assert_eq!(quests["tag"], "quests");
    // A year ahead: every month's quests, soonest first.
    let months = plan.iter().filter(|r| r["tag"] == "quests").count();
    assert_eq!(months, 13);
    let dates: Vec<&str> = plan
        .iter()
        .map(|r| r["local_date"].as_str().unwrap())
        .collect();
    assert!(dates.is_sorted());
    assert_eq!(booked().await, finished);

    // The plan is the same for nobody in particular, and `days` is how far it looks.
    let (status, anonymous) = call(&app, "GET", &uri, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(anonymous.as_array().unwrap().len(), plan.len());
    let uri = format!("/api/reminders?{HERE}&days=0");
    let (_, today) = call(&app, "GET", &uri, Some(&token), None).await;
    assert!(today
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["local_date"] == "2026-12-01"));
    assert_eq!(booked().await, finished);

    // Neither does looking at the quests themselves.
    let uri = format!("/api/discover?{HERE}");
    call(&app, "GET", &uri, Some(&token), None).await;
    assert_eq!(booked().await, finished);
}
