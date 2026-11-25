use super::*;

/// Every table that refers to a user, with a row of the merged user's in it.
#[tokio::test]
async fn a_merge_moves_every_table_of_the_user() {
    let db = crate::store::connect("sqlite::memory:").await.unwrap();
    let from = create_user(&db, None, 0).await.unwrap();
    let from = session_for_token(&db, &from, 0)
        .await
        .unwrap()
        .unwrap()
        .user;
    let into = create_user(&db, None, 0).await.unwrap();
    let into = session_for_token(&db, &into, 0)
        .await
        .unwrap()
        .unwrap()
        .user;
    sqlx::query("INSERT INTO no_cuisine_reports (place_id, user_id, reported_at) VALUES ('osm-node-1', ?, 0)")
        .bind(&from)
        .execute(&db)
        .await
        .unwrap();
    new_link_code(&db, &from, 0).await.unwrap();

    let mut tx = db.begin().await.unwrap();
    merge(&mut tx, &from, &into).await.unwrap();
    tx.commit().await.unwrap();

    let reports: Vec<String> = sqlx::query_scalar("SELECT user_id FROM no_cuisine_reports")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(reports, std::slice::from_ref(&into));
    let sessions: Vec<String> = sqlx::query_scalar("SELECT DISTINCT user_id FROM sessions")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(sessions, [into]);
    let codes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM link_codes")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(codes, 0);
}

/// A row in every table with a column that names a user, found here by the column's
/// name and not the way `merge` finds them: after a merge each row is the other user's,
/// or its table is one that says it stays behind.
#[tokio::test]
async fn a_merge_leaves_no_row_of_the_user_behind() {
    let db = crate::store::connect("sqlite::memory:").await.unwrap();
    let mut conn = db.acquire().await.unwrap();
    // The rows are made up: what they refer to isn't there, and a value may be one a
    // column doesn't allow.
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
    for id in ["from", "into"] {
        sqlx::query("INSERT INTO users (id, token, created_at) VALUES (?, ?, 0)")
            .bind(id)
            .bind(id)
            .execute(&mut *conn)
            .await
            .unwrap();
    }
    for (table, user_column) in &named {
        let columns: Vec<(String, String)> =
            sqlx::query_as("SELECT name, type FROM pragma_table_info(?)")
                .bind(table)
                .fetch_all(&mut *conn)
                .await
                .unwrap();
        let quoted: Vec<String> = columns.iter().map(|(n, _)| format!(r#""{n}""#)).collect();
        let values: Vec<&str> = columns
            .iter()
            .map(
                |(column, kind)| match (column == user_column, kind.as_str()) {
                    (true, _) => "'from'",
                    (false, "TEXT") => "'x'",
                    (false, _) => "1",
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

    merge(&mut conn, "from", "into").await.unwrap();

    for (table, column) in &named {
        let owners: Vec<String> =
            sqlx::query_scalar(&format!(r#"SELECT "{column}" FROM "{table}""#))
                .fetch_all(&mut *conn)
                .await
                .unwrap();
        // Named here again on purpose: leaving a table out of a merge is a decision.
        let expected = if table == "link_codes" {
            "from"
        } else {
            "into"
        };
        assert_eq!(owners, [expected], "{table}.{column}");
    }
}
