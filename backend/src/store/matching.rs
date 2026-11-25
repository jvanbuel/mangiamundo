//! Pending additions meeting their imported twins, and the bonuses they settle.

use sqlx::Row;

use super::areas::upsert_place;
use super::imported::found;
use crate::catalog;
use crate::game::Bonus;
use crate::geo;

/// How close an imported place must be to a pending one to be the same restaurant.
const MATCH_RADIUS_M: f64 = 50.0;

/// Swaps each pending place that now has an imported twin (same country, similar name,
/// within [`MATCH_RADIUS_M`]) for that twin: its visits and areas move over and the
/// pending row goes. `at` is when they are confirmed. Returns how many were swapped.
pub(super) async fn match_pending(tx: &mut sqlx::SqliteConnection, at: i64) -> sqlx::Result<usize> {
    let pending = sqlx::query(
        "SELECT p.id, p.name, p.lat, p.lng, p.suggested_for, pc.country FROM places p
         JOIN place_countries pc ON pc.place_id = p.id
         WHERE p.added_by IS NOT NULL ORDER BY p.added_at, p.id",
    )
    .fetch_all(&mut *tx)
    .await?;
    let mut matched = 0;
    for p in pending {
        let (id, name, lat, lng, country): (String, String, f64, f64, String) = (
            p.get("id"),
            p.get("name"),
            p.get("lat"),
            p.get("lng"),
            p.get("country"),
        );
        let suggested_for: Option<String> = p.get("suggested_for");
        // 0.001° of latitude is 111 m; a degree of longitude shrinks with cos(latitude).
        let dlng = 0.001 / lat.to_radians().cos().max(0.01);
        let near = sqlx::query(
            "SELECT * FROM osm_places WHERE lat BETWEEN ? AND ? AND lng BETWEEN ? AND ?",
        )
        .bind(lat - 0.001)
        .bind(lat + 0.001)
        .bind(lng - dlng)
        .bind(lng + dlng)
        .fetch_all(&mut *tx)
        .await?;
        let twin = near
            .iter()
            .filter_map(found)
            .filter(|o| o.countries.contains(&country.as_str()) && similar_names(&o.name, &name))
            .map(|o| (geo::distance_m(lat, lng, o.lat, o.lng), o))
            .filter(|(d, _)| *d <= MATCH_RADIUS_M)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        // A suggestion is about one OSM place: once that has a cuisine of its own, OSM wins.
        let overruled = match (&twin, suggested_for) {
            (Some(_), _) | (None, None) => None,
            (None, Some(osm_id)) => sqlx::query("SELECT * FROM osm_places WHERE id = ?")
                .bind(osm_id)
                .fetch_optional(&mut *tx)
                .await?
                .as_ref()
                .and_then(found),
        };
        let twin = match (twin, overruled) {
            (Some((_, twin)), _) => {
                confirm_addition(&mut *tx, &id, &twin.id, at).await?;
                twin
            }
            (None, Some(osm)) => {
                sqlx::query("DELETE FROM additions WHERE place_id = ?")
                    .bind(&id)
                    .execute(&mut *tx)
                    .await?;
                osm
            }
            (None, None) => continue,
        };
        upsert_place(&mut *tx, &twin, at).await?;
        // A clash means the user already has that row for the twin; the pending copy goes with the place.
        for sql in [
            "UPDATE OR IGNORE visits SET place_id = ?1 WHERE place_id = ?2",
            "UPDATE OR IGNORE closed_reports SET place_id = ?1 WHERE place_id = ?2",
            "INSERT OR IGNORE INTO area_places (area_id, place_id) SELECT area_id, ?1 FROM area_places WHERE place_id = ?2",
            "DELETE FROM places WHERE id = ?2",
        ] {
            sqlx::query(sql).bind(&twin.id).bind(&id).execute(&mut *tx).await?;
        }
        matched += 1;
    }
    Ok(matched)
}

/// Marks an addition as confirmed. Only the first addition of a place to be confirmed earns
/// anything; later ones are duplicates. Then the bonuses of its city are settled again.
async fn confirm_addition(
    tx: &mut sqlx::SqliteConnection,
    id: &str,
    osm_id: &str,
    at: i64,
) -> sqlx::Result<()> {
    let Some(area_id) =
        sqlx::query_scalar::<_, Option<String>>("SELECT area_id FROM additions WHERE place_id = ?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
    else {
        return Ok(());
    };
    let duplicate: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM additions WHERE osm_place_id = ? AND rewarded)",
    )
    .bind(osm_id)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE additions SET confirmed_at = ?, osm_place_id = ?, rewarded = ?,
             bonus = CASE WHEN ? THEN 'none' ELSE claimed_bonus END
         WHERE place_id = ?",
    )
    .bind(at)
    .bind(osm_id)
    .bind(!duplicate)
    .bind(duplicate)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if let Some(area_id) = area_id {
        settle_bonuses(&mut *tx, &area_id, id).await?;
    }
    Ok(())
}

/// Hands out a city's country and continent bonuses in submission order: an addition keeps
/// the bonus it claimed unless an earlier one, pending or rewarded, holds it. Duplicates hold
/// nothing, so a bonus whose claimant turned out to be a duplicate goes to the next in line.
/// `just_confirmed` takes the bonus it is due; others already confirmed only ever gain, so XP
/// that was granted and announced never goes down.
async fn settle_bonuses(
    tx: &mut sqlx::SqliteConnection,
    area_id: &str,
    just_confirmed: &str,
) -> sqlx::Result<()> {
    let rows: Vec<(String, String, String, String, bool)> = sqlx::query_as(
        "SELECT place_id, country, claimed_bonus, bonus, confirmed_at IS NOT NULL FROM additions
         WHERE area_id = ? AND (confirmed_at IS NULL OR rewarded)
         ORDER BY submitted_at, place_id",
    )
    .bind(area_id)
    .fetch_all(&mut *tx)
    .await?;
    let continent = |code: &str| catalog::country(code).map(|c| c.continent.as_str());
    let (mut countries, mut continents) = (Vec::new(), Vec::new());
    for (id, country, claimed, current, confirmed) in rows {
        let due = match Bonus::parse(&claimed) {
            Bonus::None => Bonus::None,
            _ if countries.contains(&country) => Bonus::None,
            Bonus::Continent if continents.contains(&continent(&country)) => Bonus::Country,
            claimed => claimed,
        };
        let current = Bonus::parse(&current);
        let settled = if !confirmed || id == just_confirmed {
            due
        } else {
            due.max(current)
        };
        if settled != Bonus::None {
            countries.push(country.clone());
        }
        if settled == Bonus::Continent {
            continents.push(continent(&country));
        }
        if confirmed && settled != current {
            sqlx::query("UPDATE additions SET bonus = ? WHERE place_id = ?")
                .bind(settled.as_str())
                .bind(&id)
                .execute(&mut *tx)
                .await?;
        }
    }
    Ok(())
}

/// The same name once case, punctuation and extra words like "Restaurant" are ignored:
/// one name's words all appear in the other.
// ponytail: no accent folding or typo tolerance ("Café" ≠ "Cafe"), add if matches are missed.
pub(super) fn similar_names(a: &str, b: &str) -> bool {
    let words = |s: &str| -> Vec<String> {
        s.split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(str::to_lowercase)
            .collect()
    };
    let (a, b) = (words(a), words(b));
    let within = |x: &[String], y: &[String]| !x.is_empty() && x.iter().all(|w| y.contains(w));
    within(&a, &b) || within(&b, &a)
}
