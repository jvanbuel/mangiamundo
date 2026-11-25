//! The local copy of OpenStreetMap: imports, refreshes and nearby lookups.

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use super::areas::upsert_place;
use super::matching::match_pending;
use super::now;
use crate::catalog;
use crate::geo::{Bbox, Shape};
use crate::providers::{FoundPlace, Lookup};

/// Replaces the imported OpenStreetMap places in `bbox`, and marks the cached areas centred
/// in it as stale so their next request rebuilds them from the new copy. Pending places
/// that the import now has are swapped for it; returns how many.
pub async fn replace_imported(
    db: &SqlitePool,
    bbox: Bbox,
    places: &[FoundPlace],
) -> sqlx::Result<usize> {
    let mut tx = db.begin().await?;
    // ponytail: by centre, so an area straddling the box edge is refetched live once per import.
    sqlx::query(
        "UPDATE areas SET fetched_at = 0 WHERE lat BETWEEN ? AND ? AND lng BETWEEN ? AND ?",
    )
    .bind(bbox.south)
    .bind(bbox.north)
    .bind(bbox.west)
    .bind(bbox.east)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM osm_places")
        .execute(&mut *tx)
        .await?;
    for place in places {
        keep_imported(&mut tx, place).await?;
    }
    let matched = match_pending(&mut tx, now()).await?;
    tx.commit().await?;
    Ok(matched)
}

/// Saves a place in the local copy of OpenStreetMap, so the next area rebuild uses it.
pub(super) async fn keep_imported(
    tx: &mut sqlx::SqliteConnection,
    place: &FoundPlace,
) -> sqlx::Result<()> {
    let Some(cuisine) = &place.cuisine else {
        return Ok(());
    };
    sqlx::query(
        "INSERT OR REPLACE INTO osm_places (id, name, lat, lng, address, url, cuisine, opening_hours)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&place.id)
    .bind(&place.name)
    .bind(place.lat)
    .bind(place.lng)
    .bind(&place.address)
    .bind(&place.url)
    .bind(cuisine)
    .bind(&place.opening_hours)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// Puts a place in every cached area whose outline holds it, and returns those areas as
/// (id, name, kind), by id.
pub(super) async fn put_in_areas(
    tx: &mut sqlx::SqliteConnection,
    place_id: &str,
    lat: f64,
    lng: f64,
) -> sqlx::Result<Vec<(String, String, String)>> {
    // Areas are at most 1.5° across, so any area holding the place is centred within that.
    let near = sqlx::query("SELECT id, name, kind, shape FROM areas WHERE lat BETWEEN ? AND ? AND lng BETWEEN ? AND ? ORDER BY id")
        .bind(lat - 1.5)
        .bind(lat + 1.5)
        .bind(lng - 1.5)
        .bind(lng + 1.5)
        .fetch_all(&mut *tx)
        .await?;
    let mut inside = Vec::new();
    for r in near {
        if serde_json::from_str::<Shape>(r.get("shape")).is_ok_and(|s| s.contains(lat, lng)) {
            let area_id: String = r.get("id");
            sqlx::query("INSERT OR IGNORE INTO area_places (area_id, place_id) VALUES (?, ?)")
                .bind(&area_id)
                .bind(place_id)
                .execute(&mut *tx)
                .await?;
            inside.push((area_id, r.get("name"), r.get("kind")));
        }
    }
    Ok(inside)
}

/// What reading a place again from OpenStreetMap changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Refreshed {
    /// Its name, position, address, website, cuisine or hours changed, or it is listed again.
    Updated,
    /// OpenStreetMap has the same as we do. A very recent edit may not have reached it yet.
    Unchanged,
    /// No longer on OpenStreetMap as a restaurant serving a national cuisine: hidden now.
    Gone,
}

/// Applies a place read again from OpenStreetMap, to the place and to the local copy, so
/// the next import or area rebuild doesn't bring back the old version. A gone place leaves
/// every area but keeps its row, so visits to it are never lost.
pub async fn refresh_place(
    db: &SqlitePool,
    id: &str,
    lookup: &Lookup,
    at: i64,
) -> sqlx::Result<Refreshed> {
    let mut tx = db.begin().await?;
    let listed: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM area_places WHERE place_id = ?)")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    let outcome = match lookup {
        Lookup::Gone => {
            sqlx::query("DELETE FROM area_places WHERE place_id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM osm_places WHERE id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            Refreshed::Gone
        }
        Lookup::Found(place) => {
            let before = snapshot(&mut tx, id).await?;
            upsert_place(&mut tx, place, at).await?;
            keep_imported(&mut tx, place).await?;
            // It may have moved: only the areas holding its new position keep it.
            sqlx::query("DELETE FROM area_places WHERE place_id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            put_in_areas(&mut tx, id, place.lat, place.lng).await?;
            let after = snapshot(&mut tx, id).await?;
            if listed && before == after {
                Refreshed::Unchanged
            } else {
                Refreshed::Updated
            }
        }
    };
    tx.commit().await?;
    Ok(outcome)
}

/// What users see of a place, to tell whether a refresh changed it.
async fn snapshot(tx: &mut sqlx::SqliteConnection, id: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar(
        "SELECT json_array(name, round(lat, 6), round(lng, 6), address, url, cuisine, opening_hours,
                (SELECT group_concat(country) FROM (SELECT country FROM place_countries WHERE place_id = ?1 ORDER BY country)))
         FROM places WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
}

/// Saves places found around a point, puts them in the cached areas they lie in and
/// matches pending additions against them straight away. Returns the ids of those that
/// weren't listed in any area before, and how many pending places were matched.
pub async fn add_nearby(
    db: &SqlitePool,
    places: &[FoundPlace],
    at: i64,
) -> sqlx::Result<(Vec<String>, usize)> {
    let mut tx = db.begin().await?;
    let mut new = Vec::new();
    for place in places {
        let listed: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM area_places WHERE place_id = ?)")
                .bind(&place.id)
                .fetch_one(&mut *tx)
                .await?;
        upsert_place(&mut tx, place, at).await?;
        keep_imported(&mut tx, place).await?;
        let areas = put_in_areas(&mut tx, &place.id, place.lat, place.lng).await?;
        if !listed && !areas.is_empty() {
            new.push(place.id.clone());
        }
    }
    let matched = match_pending(&mut tx, at).await?;
    tx.commit().await?;
    Ok((new, matched))
}

/// An `osm_places` row, or `None` when its cuisine maps to no country.
pub(super) fn found(r: &sqlx::sqlite::SqliteRow) -> Option<FoundPlace> {
    let cuisine: String = r.get("cuisine");
    let countries = catalog::countries_for_cuisine_tag(&cuisine);
    (!countries.is_empty()).then(|| FoundPlace {
        id: r.get("id"),
        name: r.get("name"),
        lat: r.get("lat"),
        lng: r.get("lng"),
        address: r.get("address"),
        url: r.get("url"),
        cuisine: Some(cuisine),
        opening_hours: r.get("opening_hours"),
        countries,
    })
}

/// Imported places inside a box.
pub async fn imported_in(db: &SqlitePool, b: Bbox) -> sqlx::Result<Vec<FoundPlace>> {
    let rows =
        sqlx::query("SELECT * FROM osm_places WHERE lat BETWEEN ? AND ? AND lng BETWEEN ? AND ?")
            .bind(b.south)
            .bind(b.north)
            .bind(b.west)
            .bind(b.east)
            .fetch_all(db)
            .await?;
    Ok(rows.iter().filter_map(found).collect())
}
