//! Restaurants users add because OpenStreetMap lacks them.

use sqlx::SqlitePool;

use super::imported::put_in_areas;
use crate::catalog;
use crate::game::Bonus;

/// A restaurant a user adds because OpenStreetMap lacks it.
pub struct NewPlace {
    pub name: String,
    pub lat: f64,
    pub lng: f64,
    pub address: Option<String>,
    pub url: Option<String>,
    pub country: &'static str,
    pub cuisine: &'static str,
    /// The unknown place this suggests the food of, from [`unknown_places`].
    pub suggested_for: Option<String>,
}

/// A place a user just added, and the reward it earns once OpenStreetMap has it.
pub struct AddedPlace {
    pub id: String,
    pub bonus: Bonus,
    /// The area the bonus was judged in, if one was cached.
    pub area_name: Option<String>,
}

/// Saves a place added by `user` and puts it in the cached areas around it. Returns it, or
/// `None` when the user already added `limit` places in the day before `at`. Checking and
/// inserting is one statement, so concurrent requests can't both take the last slot.
///
/// Its bonus is decided now, against the places everyone sees in its city: pending
/// additions don't count, so a mapper adding the same cuisine while it waits doesn't take
/// the bonus away. Only a city counts, never a postal code (which would make a "new
/// country" easier to find); with no cached city around it there is no bonus.
pub async fn add_place(
    db: &SqlitePool,
    user: &str,
    place: &NewPlace,
    at: i64,
    limit: i64,
) -> sqlx::Result<Option<AddedPlace>> {
    let id = format!("user-{}", uuid::Uuid::new_v4().simple());
    let mut tx = db.begin().await?;
    let added = sqlx::query(
        "INSERT INTO places (id, name, lat, lng, address, url, cuisine, updated_at, added_by, added_at, suggested_for)
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?8, ?11
         WHERE (SELECT COUNT(*) FROM places WHERE added_by = ?9 AND added_at > ?8 - 86400) < ?10",
    )
    .bind(&id)
    .bind(&place.name)
    .bind(place.lat)
    .bind(place.lng)
    .bind(&place.address)
    .bind(&place.url)
    .bind(place.cuisine)
    .bind(at)
    .bind(user)
    .bind(limit)
    .bind(&place.suggested_for)
    .execute(&mut *tx)
    .await?;
    if added.rows_affected() == 0 {
        return Ok(None);
    }
    sqlx::query("INSERT INTO place_countries (place_id, country) VALUES (?, ?)")
        .bind(&id)
        .bind(place.country)
        .execute(&mut *tx)
        .await?;
    let mut home: Option<(String, String)> = None;
    for (area_id, name, kind) in put_in_areas(&mut tx, &id, place.lat, place.lng).await? {
        if kind == "city" {
            home.get_or_insert((area_id, name));
        }
    }
    let bonus = match &home {
        Some((area_id, _)) => {
            let present: Vec<String> = sqlx::query_scalar(
                "SELECT DISTINCT pc.country FROM area_places ap
                 JOIN places p ON p.id = ap.place_id
                 JOIN place_countries pc ON pc.place_id = p.id
                 WHERE ap.area_id = ? AND p.added_by IS NULL
                   AND (SELECT COUNT(*) FROM closed_reports WHERE place_id = p.id) < 2",
            )
            .bind(area_id)
            .fetch_all(&mut *tx)
            .await?;
            bonus_for(place.country, &present)
        }
        None => Bonus::None,
    };
    let (area_id, area_name) = home.unzip();
    sqlx::query(
        "INSERT INTO additions (place_id, user_id, name, country, area_id, area_name, claimed_bonus, bonus, submitted_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8)",
    )
    .bind(&id)
    .bind(user)
    .bind(&place.name)
    .bind(place.country)
    .bind(&area_id)
    .bind(&area_name)
    .bind(bonus.as_str())
    .bind(at)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some(AddedPlace {
        id,
        bonus,
        area_name,
    }))
}

/// The bonus for adding a place of `country` to an area serving `present`.
fn bonus_for(country: &str, present: &[String]) -> Bonus {
    let continent = |code: &str| catalog::country(code).map(|c| c.continent.as_str());
    if present.iter().any(|c| c == country) {
        Bonus::None
    } else if present.iter().any(|c| continent(c) == continent(country)) {
        Bonus::Country
    } else {
        Bonus::Continent
    }
}

pub async fn set_osm_note(db: &SqlitePool, place_id: &str, note_id: i64) -> sqlx::Result<()> {
    sqlx::query("UPDATE places SET osm_note_id = ? WHERE id = ?")
        .bind(note_id)
        .bind(place_id)
        .execute(db)
        .await?;
    Ok(())
}
