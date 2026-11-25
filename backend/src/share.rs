//! Sharing: a public page of a user's progress (`/p/{slug}`), shared check-ins
//! (`/c/{slug}`), and the card image both use, which doubles as the link preview.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::{Arc, OnceLock};

use resvg::{tiny_skia, usvg};
use serde::Deserialize;
use sqlx::{Row, SqlitePool};

use crate::account::random_code;
use crate::catalog;
use crate::game::{Status, Summary};

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
        .map(|d| d.format("%-d %b %Y").to_string())
        .unwrap_or_else(|_| date.to_string())
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

// ---- The card ----

const W: f64 = 1200.0;
const H: f64 = 630.0;
/// The caption strip under the map.
const STRIP: f64 = 150.0;
/// The flat map's top edge; at 1200 px for 360° its 480 px reach down to 60° south, so
/// Antarctica and the far north (mostly ice) stay out.
const NORTH: f64 = 84.0;

#[derive(Deserialize)]
struct WorldCountry {
    code: String,
    /// Each ring is a flat list: lng, lat, lng, lat, ... Rings crossing the antimeridian
    /// run past ±180 instead of jumping.
    rings: Vec<Vec<f64>>,
}

/// The country outlines the apps draw too.
fn world() -> &'static [WorldCountry] {
    static WORLD: OnceLock<Vec<WorldCountry>> = OnceLock::new();
    WORLD.get_or_init(|| {
        serde_json::from_str(include_str!("../../shared/world.json")).expect("world.json is valid")
    })
}

fn fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_font_data(
                include_bytes!("../../shared/fonts/res/font/nunito_extrabold.ttf").to_vec(),
            );
            db.load_font_data(
                include_bytes!("../../shared/fonts/res/font/nunito_semibold.ttf").to_vec(),
            );
            Arc::new(db)
        })
        .clone()
}

/// Cuts text that wouldn't fit on the card's line, at about `max` characters.
fn fit(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max - 1).collect();
    format!("{}…", cut.trim_end())
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The card as SVG: a flat world map coloured like the app's (light theme), and the caption.
fn card_svg(statuses: &HashMap<&str, Status>, headline: &str, caption: &str) -> String {
    let map_h = H - STRIP;
    let scale = W / 360.0;
    let top = NORTH;
    // Countries without places nearby go in `land`; the others by their status.
    let mut paths: HashMap<Option<Status>, String> = HashMap::new();
    for country in world() {
        let status = statuses.get(country.code.as_str()).copied();
        let path = paths.entry(status).or_default();
        for ring in &country.rings {
            let lngs = ring.iter().step_by(2);
            let (min, max) = lngs.fold((f64::MAX, f64::MIN), |(a, b), &x| (a.min(x), b.max(x)));
            // A ring that runs past the antimeridian is drawn again on the other side.
            let mut shifts = vec![0.0];
            if min < -180.0 {
                shifts.push(360.0);
            }
            if max > 180.0 {
                shifts.push(-360.0);
            }
            for shift in shifts {
                for (i, [lng, lat]) in ring.as_chunks::<2>().0.iter().enumerate() {
                    let x = (lng + shift + 180.0) * scale;
                    let y = (top - lat) * scale;
                    let _ = write!(path, "{}{:.1} {:.1}", if i == 0 { "M" } else { "L" }, x, y);
                }
                path.push('Z');
            }
        }
    }
    let fill = |status: Option<Status>| match status {
        None => ("#fbfaf7", "#cfd8e2", 0.6),
        Some(Status::Available) => ("#2873c2", "#1d5a9e", 0.8),
        Some(Status::Discovered) => ("#e0a100", "#8a6200", 0.8),
        Some(Status::Platinum) => ("#c3c8d0", "#5f6773", 0.8),
        Some(Status::Completed) => ("#e3f5fe", "#2f8fc4", 1.5),
    };
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">
<rect width="{W}" height="{map_h}" fill="#dbe5ef"/>
<g clip-path="url(#map)" fill-rule="evenodd" stroke-linejoin="round">
<clipPath id="map"><rect width="{W}" height="{map_h}"/></clipPath>
"##
    );
    // Land first, so lit countries' borders draw on top.
    for status in [
        None,
        Some(Status::Available),
        Some(Status::Discovered),
        Some(Status::Platinum),
        Some(Status::Completed),
    ] {
        if let Some(d) = paths.get(&status) {
            let (fill, stroke, width) = fill(status);
            let _ = writeln!(
                svg,
                r#"<path d="{d}" fill="{fill}" stroke="{stroke}" stroke-width="{width}"/>"#
            );
        }
    }
    let _ = write!(
        svg,
        r##"</g>
<rect y="{map_h}" width="{W}" height="{STRIP}" fill="#ffffff"/>
<text x="48" y="{}" font-family="Nunito" font-weight="800" font-size="52" fill="#14263a">{}</text>
<text x="48" y="{}" font-family="Nunito" font-weight="600" font-size="30" fill="#5a6a7d">{}</text>
</svg>"##,
        map_h + 70.0,
        // About 36 of Nunito's widest-looking characters fit at 52 px, 60 at 30 px.
        escape(&fit(headline, 36)),
        map_h + 116.0,
        escape(&fit(caption, 60)),
    );
    svg
}

/// The card as PNG, 1200 × 630 like link previews.
pub fn card_png(
    statuses: &HashMap<&str, Status>,
    headline: &str,
    caption: &str,
) -> anyhow::Result<Vec<u8>> {
    let options = usvg::Options {
        fontdb: fonts(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&card_svg(statuses, headline, caption), &options)?;
    let mut pixmap = tiny_skia::Pixmap::new(W as u32, H as u32).expect("the card has a size");
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap.encode_png()?)
}

// ---- The pages ----

/// A small standalone page with link-preview tags, outside the web app.
pub fn page(title: &str, description: &str, image: &str, url: &str, body: &str) -> String {
    let (title, description) = (escape(title), escape(description));
    format!(
        r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · Mangiamundo</title>
<meta name="description" content="{description}">
<meta property="og:type" content="website">
<meta property="og:site_name" content="Mangiamundo">
<meta property="og:title" content="{title}">
<meta property="og:description" content="{description}">
<meta property="og:image" content="{image}">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:url" content="{url}">
<meta name="twitter:card" content="summary_large_image">
<link rel="icon" href="/icon.svg">
<style>
  body {{ margin: 0; background: #f2f5f8; color: #14263a; font: 16px/1.5 system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif; }}
  main {{ max-width: 640px; margin: 0 auto; padding: 24px 16px 48px; }}
  img {{ width: 100%; height: auto; border-radius: 14px; display: block; }}
  h1 {{ font-size: 26px; margin: 20px 0 4px; }}
  .muted {{ color: #5a6a7d; }}
  ul {{ list-style: none; padding: 0; margin: 16px 0; display: grid; gap: 6px; }}
  li {{ background: #fff; border-radius: 10px; padding: 10px 14px; display: flex; justify-content: space-between; gap: 12px; }}
  h2 {{ font-size: 18px; margin: 24px 0 0; }}
  a.start {{ display: inline-block; margin-top: 24px; background: #2873c2; color: #fff; font-weight: 800; padding: 10px 18px; border-radius: 12px; text-decoration: none; box-shadow: 0 3px 0 #1d5a9e; }}
</style>
</head>
<body><main>
<img src="{image}" alt="{title}" width="1200" height="630">
{body}
<a class="start" href="/">Start your own map</a>
</main></body>
</html>
"##
    )
}

/// The public page's content: level, counts per continent and earned badges. No places
/// or dates.
pub fn profile_body(summary: &Summary, area: &str) -> String {
    let mut body = format!(
        "<h1>{}</h1>\n<p class=\"muted\">{} · Level {} · {} places visited</p>\n<ul>\n",
        escape(&profile_headline(summary)),
        escape(area),
        summary.profile.level,
        summary.places_visited,
    );
    for continent in summary.continents.iter().filter(|c| c.available > 0) {
        let _ = writeln!(
            body,
            "<li><span>{}</span><span class=\"muted\">{} of {}</span></li>",
            escape(&continent.name),
            continent.discovered,
            continent.available
        );
    }
    body.push_str("</ul>\n");
    let earned: Vec<_> = summary.badges.iter().filter(|b| b.earned).collect();
    if !earned.is_empty() {
        body.push_str("<h2>Badges</h2>\n<ul>\n");
        for badge in earned {
            let _ = writeln!(
                body,
                "<li><strong>{}</strong><span class=\"muted\">{}</span></li>",
                escape(&badge.title),
                escape(&badge.description)
            );
        }
        body.push_str("</ul>\n");
    }
    body
}

pub fn visit_body(visit: &SharedVisit) -> String {
    format!(
        "<h1>{}</h1>\n<p class=\"muted\">{}</p>\n",
        escape(&visit_headline(visit)),
        pretty_date(&visit.date)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_renders_a_png_with_the_world_and_text() {
        let statuses = HashMap::from([
            ("PE", Status::Discovered),
            ("JP", Status::Completed),
            ("BE", Status::Available),
        ]);
        let svg = card_svg(
            &statuses,
            "2 of 3 countries tasted",
            "Brussels & co · Level 2",
        );
        assert!(
            svg.contains("fill=\"#e0a100\""),
            "tasted countries are gold"
        );
        assert!(svg.contains("Brussels &amp; co"), "text is escaped");
        let png = card_png(&statuses, "2 of 3 countries tasted", "Brussels · Level 2").unwrap();
        assert!(png.starts_with(b"\x89PNG"));
        assert!(png.len() > 10_000, "a real picture, not a blank one");
    }

    #[test]
    fn visit_headlines_name_the_food() {
        let visit = |countries: &[&str]| SharedVisit {
            place: "Casa".into(),
            countries: countries.iter().map(|c| c.to_string()).collect(),
            date: "2026-11-22".into(),
        };
        assert_eq!(visit_headline(&visit(&["PE"])), "Peruvian food at Casa");
        assert_eq!(visit_headline(&visit(&[])), "Dinner at Casa");
        assert_eq!(pretty_date("2026-11-02"), "2 Nov 2026");
        assert_eq!(fit("Short", 10), "Short");
        assert_eq!(fit("A very long restaurant name", 10), "A very lo…");
    }
}
