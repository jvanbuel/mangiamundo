//! Shared progress and check-ins, as public pages that can be taken down.

use axum::http::StatusCode;

use crate::common::{app, call, session, visits_uri, HERE};
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
