use super::*;

/// A database as it was before tokens were hashed, with one device signed in.
#[tokio::test]
async fn a_token_from_before_hashing_still_signs_in() {
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let mut before = sqlx::migrate!();
    before.migrations = before
        .migrations
        .iter()
        .filter(|m| m.version < 18)
        .cloned()
        .collect();
    before.run(&db).await.unwrap();
    sqlx::query("INSERT INTO users (id, token, created_at) VALUES ('u', 'u', 0)")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO sessions (id, token, user_id, created_at, last_used_at) VALUES ('s', 'old-token', 'u', 0, 0)")
        .execute(&db)
        .await
        .unwrap();

    // Twice: the server does this at every start.
    crate::store::migrate(&db).await.unwrap();
    crate::store::migrate(&db).await.unwrap();

    let stored: Vec<String> = sqlx::query_scalar("SELECT token_hash FROM sessions")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(stored, [hash("old-token")]);
    let session = session_for_token(&db, "old-token", 0).await.unwrap();
    assert_eq!(session.unwrap().user, "u");
    // What is stored signs nobody in.
    assert!(session_for_token(&db, &stored[0], 0)
        .await
        .unwrap()
        .is_none());
    assert!(session_for_token(&db, "plain:old-token", 0)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn a_new_token_is_stored_as_its_hash() {
    let db = crate::store::connect("sqlite::memory:").await.unwrap();
    let token = create_user(&db, None, 0).await.unwrap();
    let stored: String = sqlx::query_scalar("SELECT token_hash FROM sessions")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(stored, hash(&token));
    assert_ne!(stored, token);
    assert_eq!(hash_plain_tokens(&db).await.unwrap(), 0);
    assert!(session_for_token(&db, &token, 0).await.unwrap().is_some());
}
