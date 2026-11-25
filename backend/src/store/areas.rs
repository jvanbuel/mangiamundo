//! The area cache: cities and postal codes with the places in them.

use sqlx::{Row, SqlitePool};

use crate::geo::Shape;
use crate::providers::{AreaInfo, AreaKind, Boundary, FoundPlace};

/// A cached provider lookup for a city or postal code.
#[derive(Debug, Clone)]
pub struct Area {
    pub boundary: Boundary,
    pub provider: String,
    pub fetched_at: i64,
}

impl Area {
    pub fn id(&self) -> &str {
        &self.boundary.info.id
    }
}

pub(super) fn kind_name(kind: AreaKind) -> &'static str {
    match kind {
        AreaKind::City => "city",
        AreaKind::PostalCode => "postal_code",
    }
}

pub async fn find_area(db: &SqlitePool, id: &str) -> sqlx::Result<Option<Area>> {
    let row = sqlx::query("SELECT * FROM areas WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await?;
    Ok(row.and_then(|r| {
        let kind = match r.get::<String, _>("kind").as_str() {
            "city" => AreaKind::City,
            "postal_code" => AreaKind::PostalCode,
            _ => return None,
        };
        let shape: Shape = serde_json::from_str(r.get("shape")).ok()?;
        Some(Area {
            boundary: Boundary {
                info: AreaInfo {
                    id: r.get("id"),
                    name: r.get("name"),
                    kind,
                    detail: r.get("detail"),
                    lat: r.get("lat"),
                    lng: r.get("lng"),
                },
                shape,
            },
            provider: r.get("provider"),
            fetched_at: r.get("fetched_at"),
        })
    }))
}

/// Replaces the contents of an area with a fresh provider result. Places that dropped
/// out of the result stay in `places` so existing visits are never lost, and places users
/// added stay in the area.
pub async fn save_area(db: &SqlitePool, area: &Area, places: &[FoundPlace]) -> sqlx::Result<()> {
    let info = &area.boundary.info;
    let shape = serde_json::to_string(&area.boundary.shape).expect("a shape serializes");
    let mut tx = db.begin().await?;
    sqlx::query(
        "INSERT INTO areas (id, name, kind, detail, lat, lng, shape, provider, fetched_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, kind = excluded.kind,
             detail = excluded.detail, lat = excluded.lat, lng = excluded.lng,
             shape = excluded.shape, fetched_at = excluded.fetched_at",
    )
    .bind(&info.id)
    .bind(&info.name)
    .bind(kind_name(info.kind))
    .bind(&info.detail)
    .bind(info.lat)
    .bind(info.lng)
    .bind(shape)
    // Never read back for anything, but the column is NOT NULL without a default.
    .bind(&area.provider)
    .bind(area.fetched_at)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM area_places WHERE area_id = ?
         AND place_id NOT IN (SELECT id FROM places WHERE added_by IS NOT NULL)",
    )
    .bind(&info.id)
    .execute(&mut *tx)
    .await?;
    // An area cached after a place was added still needs it.
    let b = area.boundary.shape.bbox();
    let added = sqlx::query(
        "SELECT id, lat, lng FROM places WHERE added_by IS NOT NULL AND lat BETWEEN ? AND ? AND lng BETWEEN ? AND ?",
    )
    .bind(b.south)
    .bind(b.north)
    .bind(b.west)
    .bind(b.east)
    .fetch_all(&mut *tx)
    .await?;
    for r in added
        .iter()
        .filter(|r| area.boundary.shape.contains(r.get("lat"), r.get("lng")))
    {
        sqlx::query("INSERT OR IGNORE INTO area_places (area_id, place_id) VALUES (?, ?)")
            .bind(&info.id)
            .bind(r.get::<String, _>("id"))
            .execute(&mut *tx)
            .await?;
    }

    for place in places {
        upsert_place(&mut tx, place, area.fetched_at).await?;
        sqlx::query("INSERT OR IGNORE INTO area_places (area_id, place_id) VALUES (?, ?)")
            .bind(&info.id)
            .bind(&place.id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await
}

pub(super) async fn upsert_place(
    tx: &mut sqlx::SqliteConnection,
    place: &FoundPlace,
    at: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO places (id, name, lat, lng, address, url, cuisine, opening_hours, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, lat = excluded.lat,
             lng = excluded.lng, address = excluded.address, url = excluded.url,
             cuisine = excluded.cuisine, opening_hours = excluded.opening_hours,
             updated_at = excluded.updated_at",
    )
    .bind(&place.id)
    .bind(&place.name)
    .bind(place.lat)
    .bind(place.lng)
    .bind(&place.address)
    .bind(&place.url)
    .bind(&place.cuisine)
    .bind(&place.opening_hours)
    .bind(at)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM place_countries WHERE place_id = ?")
        .bind(&place.id)
        .execute(&mut *tx)
        .await?;
    for country in &place.countries {
        sqlx::query("INSERT OR IGNORE INTO place_countries (place_id, country) VALUES (?, ?)")
            .bind(&place.id)
            .bind(country)
            .execute(&mut *tx)
            .await?;
    }
    Ok(())
}

/// The cached areas a place is in.
pub async fn areas_of_place(db: &SqlitePool, place_id: &str) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar("SELECT area_id FROM area_places WHERE place_id = ? ORDER BY area_id")
        .bind(place_id)
        .fetch_all(db)
        .await
}

/// The cached areas whose outline holds a point, by id.
pub async fn areas_containing(db: &SqlitePool, lat: f64, lng: f64) -> sqlx::Result<Vec<Boundary>> {
    // Areas are at most 1.5° across, so any area holding the point is centred within that.
    let near = sqlx::query(
        "SELECT id, name, kind, detail, lat, lng, shape FROM areas
         WHERE lat BETWEEN ? AND ? AND lng BETWEEN ? AND ? ORDER BY id",
    )
    .bind(lat - 1.5)
    .bind(lat + 1.5)
    .bind(lng - 1.5)
    .bind(lng + 1.5)
    .fetch_all(db)
    .await?;
    Ok(near
        .into_iter()
        .filter_map(|r| {
            let kind = match r.get::<String, _>("kind").as_str() {
                "city" => AreaKind::City,
                "postal_code" => AreaKind::PostalCode,
                _ => return None,
            };
            let shape: Shape = serde_json::from_str(r.get("shape")).ok()?;
            shape.contains(lat, lng).then(|| Boundary {
                info: AreaInfo {
                    id: r.get("id"),
                    name: r.get("name"),
                    kind,
                    detail: r.get("detail"),
                    lat: r.get("lat"),
                    lng: r.get("lng"),
                },
                shape,
            })
        })
        .collect())
}
