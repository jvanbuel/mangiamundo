//! Signing devices in with link codes, signing them out, and deleting the account.

use axum::http::StatusCode;

use crate::common::{app, app_with_clock, call, session, START};

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
