use super::*;

async fn users(db: &SqlitePool) -> Vec<String> {
    sqlx::query_scalar("SELECT id FROM users ORDER BY id")
        .fetch_all(db)
        .await
        .unwrap()
}

#[tokio::test]
async fn an_empty_account_goes_once_it_is_old_and_unused() {
    const DAY: i64 = 86_400;
    let db = crate::store::connect("sqlite::memory:").await.unwrap();
    let token = create_user(&db, None, 0).await.unwrap();
    let user = session_for_token(&db, &token, 0)
        .await
        .unwrap()
        .unwrap()
        .user;

    // Too young, whatever it holds.
    let at = ABANDONED_AFTER_SECS;
    assert_eq!(clean_up(&db, at).await.unwrap().users, 0);
    // Old, but a device used it lately.
    session_for_token(&db, &token, at + 30 * DAY).await.unwrap();
    assert_eq!(clean_up(&db, at + 31 * DAY).await.unwrap().users, 0);
    assert_eq!(
        clean_up(&db, at + 30 * DAY + ABANDONED_AFTER_SECS)
            .await
            .unwrap()
            .users,
        0
    );
    assert_eq!(users(&db).await, std::slice::from_ref(&user));
    // A recovery code is a way back in.
    new_recovery_code(&db, &user, 0).await.unwrap();
    let later = at + 31 * DAY + ABANDONED_AFTER_SECS;
    assert_eq!(clean_up(&db, later).await.unwrap().users, 0);
    sqlx::query("UPDATE users SET recovery_hash = NULL")
        .execute(&db)
        .await
        .unwrap();
    // An unused link code is not.
    new_link_code(&db, &user, later).await.unwrap();

    assert_eq!(clean_up(&db, later).await.unwrap().users, 1);
    assert_eq!(users(&db).await, [] as [String; 0]);
    assert!(session_for_token(&db, &token, later)
        .await
        .unwrap()
        .is_none());
    // Without any session left it is just as gone.
    let token = create_user(&db, None, 0).await.unwrap();
    sqlx::query("DELETE FROM sessions")
        .execute(&db)
        .await
        .unwrap();
    assert!(session_for_token(&db, &token, 0).await.unwrap().is_none());
    assert_eq!(clean_up(&db, later).await.unwrap().users, 1);
}

/// One old, unused user per table with a column that names a user, each with a row in
/// just that table. Found by the column's name and not the way `clean_up` finds them.
#[tokio::test]
async fn an_account_with_a_row_in_any_table_is_kept() {
    let db = crate::store::connect("sqlite::memory:").await.unwrap();
    let mut conn = db.acquire().await.unwrap();
    // The rows are made up, as in `a_merge_leaves_no_row_of_the_user_behind`.
    for pragma in ["foreign_keys = OFF", "ignore_check_constraints = ON"] {
        sqlx::query(&format!("PRAGMA {pragma}"))
            .execute(&mut *conn)
            .await
            .unwrap();
    }
    let named: Vec<(String, String)> = sqlx::query_as(
        "SELECT m.name, c.name FROM sqlite_master m, pragma_table_info(m.name) c
         WHERE m.type = 'table' AND c.name IN ('user_id', 'added_by') ORDER BY m.name",
    )
    .fetch_all(&mut *conn)
    .await
    .unwrap();
    assert!(named.len() >= 10, "{named:?}");
    for (table, user_column) in &named {
        sqlx::query("INSERT INTO users (id, token, created_at) VALUES (?, ?, 0)")
            .bind(table)
            .bind(table)
            .execute(&mut *conn)
            .await
            .unwrap();
        let columns: Vec<(String, String)> =
            sqlx::query_as("SELECT name, type FROM pragma_table_info(?)")
                .bind(table)
                .fetch_all(&mut *conn)
                .await
                .unwrap();
        let quoted: Vec<String> = columns.iter().map(|(n, _)| format!(r#""{n}""#)).collect();
        let values: Vec<String> = columns
            .iter()
            .map(
                |(column, kind)| match (column == user_column, kind.as_str()) {
                    (true, _) => format!("'{table}'"),
                    // Unique per row: some of these columns are keys.
                    (false, "TEXT") => format!("'x-{table}'"),
                    // Also the time a session was last used: long ago.
                    (false, _) => "1".into(),
                },
            )
            .collect();
        let insert = format!(
            r#"INSERT INTO "{table}" ({}) VALUES ({})"#,
            quoted.join(", "),
            values.join(", ")
        );
        sqlx::query(&insert).execute(&mut *conn).await.unwrap();
    }
    drop(conn);

    let cleaned = clean_up(&db, 10 * ABANDONED_AFTER_SECS).await.unwrap();

    // Named here again on purpose: what doesn't keep an account is a decision.
    let kept: Vec<String> = named
        .iter()
        .map(|(table, _)| table.clone())
        .filter(|table| table != "sessions" && table != "link_codes")
        .collect();
    assert_eq!(users(&db).await, kept);
    assert_eq!(cleaned.users, 2);
}

#[tokio::test]
async fn expired_codes_and_old_reminder_records_are_deleted() {
    let db = crate::store::connect("sqlite::memory:").await.unwrap();
    let token = create_user(&db, None, 0).await.unwrap();
    let user = session_for_token(&db, &token, 0)
        .await
        .unwrap()
        .unwrap()
        .user;
    let now = 100 * 86_400;
    for (email, expires_at) in [("old@example.com", now), ("new@example.com", now + 1)] {
        sqlx::query("INSERT INTO email_codes (email, hash, expires_at) VALUES (?, 'h', ?)")
            .bind(email)
            .bind(expires_at)
            .execute(&db)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO push_subscriptions (endpoint, user_id, p256dh, auth, timezone, created_at) VALUES ('e', ?, 'p', 'a', 'UTC', 0)")
        .bind(&user)
        .execute(&db)
        .await
        .unwrap();
    for (key, sent_at) in [
        ("old", now - PUSH_SENT_KEPT_SECS - 1),
        ("new", now - PUSH_SENT_KEPT_SECS),
    ] {
        sqlx::query("INSERT INTO push_sent (endpoint, key, sent_at) VALUES ('e', ?, ?)")
            .bind(key)
            .bind(sent_at)
            .execute(&db)
            .await
            .unwrap();
    }
    // In this order: making a link code deletes the ones that expired before it.
    new_link_code(&db, &user, now).await.unwrap();
    new_link_code(&db, &user, now - LINK_TTL_SECS)
        .await
        .unwrap();

    let cleaned = clean_up(&db, now).await.unwrap();

    assert_eq!(
        cleaned,
        Cleaned {
            users: 0,
            email_codes: 1,
            link_codes: 1,
            push_sent: 1
        }
    );
    let emails: Vec<String> = sqlx::query_scalar("SELECT email FROM email_codes")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(emails, ["new@example.com"]);
    let keys: Vec<String> = sqlx::query_scalar("SELECT key FROM push_sent")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(keys, ["new"]);
    assert_eq!(clean_up(&db, now).await.unwrap(), Cleaned::default());
}
