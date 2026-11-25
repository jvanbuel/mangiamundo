//! Restoring an account with a recovery code.

use axum::http::StatusCode;
use mangiamundo::store;

use crate::common::{app, assert_error, call, session, visits_uri, HERE, START};

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
