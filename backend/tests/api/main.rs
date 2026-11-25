//! End-to-end tests of the HTTP API against the offline demo provider.

mod account_limits;
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
mod devices;
mod errors;
mod events;
mod fake_osm;
mod osm_nearby;
mod osm_refresh;
mod recovery;
mod shared_links;
mod suggestions;
mod unknown_food;
mod visit_counts;
mod visits;

use axum::http::StatusCode;
use axum::Router;
use mangiamundo::store;
use serde_json::Value;

use crate::common::{app, app_with, call, session, visits_uri, DAY, HERE, START};
use crate::shared_links::{fetch, path_of, post_shared};

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
