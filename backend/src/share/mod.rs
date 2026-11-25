//! Sharing: a public page of a user's progress (`/p/{slug}`), shared check-ins
//! (`/c/{slug}`), and the card image both use, which doubles as the link preview.

use sqlx::{Row, SqlitePool};

use crate::account::random_code;
use crate::catalog;
use crate::game::Summary;

mod card;
mod pages;

pub use card::card_png;
pub use pages::{page, profile_body, visit_body};

/// 10 symbols, 50 bits: unguessable for a read-only page.
const SLUG_LEN: usize = 10;

fn new_slug() -> String {
    random_code(SLUG_LEN).to_ascii_lowercase()
}

/// Turns the user's public page on, or moves it to `area_id`, and returns its slug.
pub async fn publish_profile(
    db: &SqlitePool,
    user: &str,
    area_id: &str,
    now: i64,
) -> sqlx::Result<String> {
    sqlx::query_scalar(
        "INSERT INTO public_profiles (user_id, slug, area_id, created_at) VALUES (?, ?, ?, ?)
         ON CONFLICT (user_id) DO UPDATE SET area_id = excluded.area_id
         RETURNING slug",
    )
    .bind(user)
    .bind(new_slug())
    .bind(area_id)
    .bind(now)
    .fetch_one(db)
    .await
}

/// Turns the public page off; its link stops working.
pub async fn unpublish_profile(db: &SqlitePool, user: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM public_profiles WHERE user_id = ?")
        .bind(user)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn profile_slug(db: &SqlitePool, user: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT slug FROM public_profiles WHERE user_id = ?")
        .bind(user)
        .fetch_optional(db)
        .await
}

/// The user and area behind a public page.
pub async fn find_profile(db: &SqlitePool, slug: &str) -> sqlx::Result<Option<(String, String)>> {
    let row = sqlx::query("SELECT user_id, area_id FROM public_profiles WHERE slug = ?")
        .bind(slug)
        .fetch_optional(db)
        .await?;
    Ok(row.map(|r| (r.get("user_id"), r.get("area_id"))))
}

/// Shares one of the user's check-ins and returns its slug, the same one each time.
/// `None` when the user has no such check-in.
pub async fn share_visit(
    db: &SqlitePool,
    user: &str,
    visit_id: i64,
    now: i64,
) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar(
        "INSERT INTO shared_visits (slug, visit_id, created_at)
         SELECT ?, id, ? FROM visits WHERE id = ? AND user_id = ?
         ON CONFLICT (visit_id) DO UPDATE SET created_at = created_at
         RETURNING slug",
    )
    .bind(new_slug())
    .bind(now)
    .bind(visit_id)
    .bind(user)
    .fetch_optional(db)
    .await
}

/// A shared check-in, as shown on its page.
pub struct SharedVisit {
    pub place: String,
    pub countries: Vec<String>,
    /// YYYY-MM-DD, where the check-in was made.
    pub date: String,
}

pub async fn find_visit(db: &SqlitePool, slug: &str) -> sqlx::Result<Option<SharedVisit>> {
    let Some(row) = sqlx::query(
        "SELECT p.id, p.name, v.local_date FROM shared_visits s
         JOIN visits v ON v.id = s.visit_id JOIN places p ON p.id = v.place_id
         WHERE s.slug = ?",
    )
    .bind(slug)
    .fetch_optional(db)
    .await?
    else {
        return Ok(None);
    };
    let place_id: String = row.get("id");
    let countries = sqlx::query_scalar(
        "SELECT country FROM place_countries WHERE place_id = ? ORDER BY country",
    )
    .bind(&place_id)
    .fetch_all(db)
    .await?;
    Ok(Some(SharedVisit {
        place: row.get("name"),
        countries,
        date: row.get("local_date"),
    }))
}

/// "Peruvian", or the country's name when the catalog has no adjective.
fn food_of(code: &str) -> String {
    catalog::cuisine_adjective(code)
        .or_else(|| catalog::country(code).map(|c| c.name.clone()))
        .unwrap_or_else(|| code.to_string())
}

/// What a shared check-in says: "Peruvian food at Casa Peru".
pub fn visit_headline(visit: &SharedVisit) -> String {
    let foods: Vec<String> = visit.countries.iter().map(|c| food_of(c)).collect();
    match foods.as_slice() {
        [] => format!("Dinner at {}", visit.place),
        foods => format!("{} food at {}", foods.join(" and "), visit.place),
    }
}

/// "22 Nov 2026".
pub fn pretty_date(date: &str) -> String {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_or_else(|_| date.to_string(), |d| d.format("%-d %b %Y").to_string())
}

pub fn profile_headline(summary: &Summary) -> String {
    format!(
        "{} of {} countries tasted",
        summary.world.discovered, summary.world.available
    )
}

pub fn profile_text(summary: &Summary, area: &str) -> String {
    format!(
        "I've tasted food from {} of the {} countries I can eat in {}. Level {} on Mangiamundo.",
        summary.world.discovered, summary.world.available, area, summary.profile.level
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests;
