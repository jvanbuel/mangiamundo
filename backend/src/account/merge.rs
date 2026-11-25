//! Merging one account into another, table by table as the schema says.

/// Every table and column that refers to `users`, read from the schema, so that a new
/// table of a user's is found without being listed anywhere.
pub(super) async fn owned(
    db: impl sqlx::SqliteExecutor<'_>,
) -> sqlx::Result<Vec<(String, String)>> {
    sqlx::query_as(
        r#"SELECT m.name, f."from" FROM sqlite_master m, pragma_foreign_key_list(m.name) f
           WHERE m.type = 'table' AND f."table" = 'users' ORDER BY m.name"#,
    )
    .fetch_all(db)
    .await
}

/// Tables whose rows stay behind in a merge, and so go with the merged user: a link code
/// made for that account no longer signs anyone in.
const NOT_MERGED: &[&str] = &["link_codes"];

/// Moves everything of user `from` to user `into`, then deletes `from`. Where both have the
/// same row (a visit on the same day, a closed report of the same place, a public page)
/// `into`'s stays.
///
/// What a user has is read from the schema: every column that refers to `users`. A new
/// table of theirs is merged without being listed here.
pub(super) async fn merge(
    tx: &mut sqlx::SqliteConnection,
    from: &str,
    into: &str,
) -> sqlx::Result<()> {
    for (table, column) in owned(&mut *tx).await? {
        if NOT_MERGED.contains(&table.as_str()) {
            continue;
        }
        let statement =
            format!(r#"UPDATE OR IGNORE "{table}" SET "{column}" = ?1 WHERE "{column}" = ?2"#);
        sqlx::query(&statement)
            .bind(into)
            .bind(from)
            .execute(&mut *tx)
            .await?;
    }
    // Rows left behind were duplicates; they go with the user.
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(from)
        .execute(&mut *tx)
        .await?;
    Ok(())
}
