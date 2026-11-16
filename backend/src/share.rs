//! Share cards: a PNG of the user's food map with their numbers, drawn on the server so
//! the web app and both native apps share one design and only need to show and share it.

use std::collections::HashMap;
use std::fmt::Write;
use std::sync::{Arc, OnceLock};

use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use resvg::{tiny_skia, usvg};
use serde::Deserialize;

use crate::api::{resolve_area, ApiError, AreaQuery, MaybeUser};
use crate::game::{self, Lifetime, Summary};
use crate::{store, AppState};

pub const WIDTH: u32 = 1080;
pub const HEIGHT: u32 = 1350;

const MAP_X: f64 = 72.0;
const MAP_Y: f64 = 470.0;
const MAP_W: f64 = 936.0;

#[derive(Deserialize)]
pub struct CardQuery {
    lat: f64,
    lng: f64,
    radius_km: Option<f64>,
    /// The name of the city shown on the card, as the client displays it.
    place: Option<String>,
    theme: Option<String>,
}

struct Palette {
    bg: &'static str,
    panel: &'static str,
    line: &'static str,
    text: &'static str,
    muted: &'static str,
    accent: &'static str,
    ocean: &'static str,
    land: &'static str,
    available: &'static str,
    discovered: &'static str,
    completed: &'static str,
}

/// The app's colour tokens (web/src/app.css).
const LIGHT: Palette = Palette {
    bg: "#f2f5f8",
    panel: "#ffffff",
    line: "#d5dee8",
    text: "#14263a",
    muted: "#5a6a7d",
    accent: "#2873c2",
    ocean: "#c4daef",
    land: "#e3e7eb",
    available: "#2873c2",
    discovered: "#c98508",
    completed: "#3da35a",
};

const DARK: Palette = Palette {
    bg: "#0d1a28",
    panel: "#132536",
    line: "#28425d",
    text: "#eaf1f8",
    muted: "#93a6bb",
    accent: "#5ba6ee",
    ocean: "#0f2133",
    land: "#233a52",
    available: "#3b8fd9",
    discovered: "#f2b53d",
    completed: "#54b96f",
};

pub(crate) async fn card(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    Query(query): Query<CardQuery>,
) -> Result<Response, ApiError> {
    let area_query = AreaQuery { lat: query.lat, lng: query.lng, radius_km: query.radius_km };
    let area = resolve_area(&state, &area_query).await?;
    let rows = store::area_rows(&state.db, &area.key, user.as_deref()).await?;
    let lifetime = match &user {
        Some(user) => store::lifetime(&state.db, user).await?,
        None => Lifetime::default(),
    };
    let summary = game::summarize(&rows, lifetime);
    let palette = if query.theme.as_deref() == Some("dark") { &DARK } else { &LIGHT };
    let place = query.place.as_deref().map(str::trim).filter(|p| !p.is_empty());
    let svg = card_svg(&summary, place, palette);
    let png = tokio::task::spawn_blocking(move || render(&svg))
        .await
        .map_err(|e| ApiError::Upstream(e.into()))?
        .map_err(ApiError::Upstream)?;
    Ok((
        [(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "private, no-store")],
        png,
    )
        .into_response())
}

/// Escapes text for use inside SVG.
fn esc(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Shortens a name to `max` characters, ending in an ellipsis when cut.
fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max - 1).collect();
    format!("{}…", cut.trim_end())
}

fn plural(n: u32, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn card_svg(summary: &Summary, place: Option<&str>, p: &Palette) -> String {
    let tasted = summary.world.discovered;
    let place = place.map(|name| esc(&shorten(name, 22)));
    let (headline, subline) = match (tasted, &place) {
        (0, Some(place)) => ("Eating the world".to_string(), format!("starting in {place}")),
        (0, None) => ("Eating the world".to_string(), "starting in my city".to_string()),
        (n, Some(place)) => (format!("I’ve tasted {}", plural(n, "country", "countries")), format!("without leaving {place}")),
        (n, None) => (format!("I’ve tasted {}", plural(n, "country", "countries")), "without leaving my city".to_string()),
    };

    let mut svg = String::new();
    let _ = write!(
        svg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {WIDTH} {HEIGHT}" font-family="Nunito">
<rect width="{WIDTH}" height="{HEIGHT}" fill="{bg}"/>
<image href="data:image/svg+xml;base64,{icon}" x="72" y="96" width="88" height="88"/>
<text x="180" y="156" font-size="44" font-weight="900" fill="{text}">Mangiamundo</text>
<text x="72" y="318" font-size="76" font-weight="900" fill="{text}">{headline}</text>
<text x="72" y="400" font-size="54" font-weight="800" fill="{accent}">{subline}</text>
"##,
        bg = p.bg,
        icon = icon_base64(),
        text = p.text,
        accent = p.accent,
    );

    // The map, on a panel with the app's chunky lower edge.
    let map_h = map_height();
    let pad = 24.0;
    let (px, py, pw, ph) = (MAP_X - pad, MAP_Y - pad, MAP_W + 2.0 * pad, map_h + 2.0 * pad);
    let _ = write!(
        svg,
        r##"<rect x="{px}" y="{py}" width="{pw}" height="{ph2}" rx="36" fill="{line}"/>
<rect x="{px}" y="{py}" width="{pw}" height="{ph}" rx="36" fill="{ocean}"/>
<clipPath id="map"><rect x="{MAP_X}" y="{MAP_Y}" width="{MAP_W}" height="{map_h}"/></clipPath>
<g clip-path="url(#map)" stroke="{ocean}" stroke-width="1.2" stroke-linejoin="round" fill-rule="evenodd">
"##,
        ph2 = ph + 8.0,
        line = p.line,
        ocean = p.ocean,
    );
    let status: HashMap<&str, &str> = summary
        .countries
        .iter()
        .map(|c| {
            let fill = if c.completed() {
                p.completed
            } else if c.discovered() {
                p.discovered
            } else {
                p.available
            };
            (c.code.as_str(), fill)
        })
        .collect();
    for (code, d) in world_paths() {
        let fill = status.get(code.as_str()).copied().unwrap_or(p.land);
        let _ = writeln!(svg, r#"<path d="{d}" fill="{fill}"/>"#);
    }
    svg.push_str("</g>\n");

    // Legend under the map.
    let legend_y = py + ph + 64.0;
    let mut x = 72.0;
    for (label, colour) in [("Tasted", p.discovered), ("Completed", p.completed), ("To discover", p.available)] {
        let _ = write!(
            svg,
            r##"<rect x="{x}" y="{y}" width="26" height="26" rx="7" fill="{colour}"/><text x="{tx}" y="{ty}" font-size="28" font-weight="700" fill="{muted}">{label}</text>
"##,
            y = legend_y - 22.0,
            tx = x + 38.0,
            ty = legend_y,
            muted = p.muted,
        );
        x += 38.0 + label.len() as f64 * 16.0 + 48.0;
    }

    // Three numbers.
    let profile = &summary.profile;
    let tiles = [
        (format!("{tasted}/{}", summary.world.available), "countries here".to_string()),
        (profile.visits.to_string(), if profile.visits == 1 { "check-in" } else { "check-ins" }.to_string()),
        (profile.dishes.to_string(), if profile.dishes == 1 { "signature dish" } else { "signature dishes" }.to_string()),
    ];
    let tile_y = legend_y + 52.0;
    let gap = 24.0;
    let tile_w = (WIDTH as f64 - 144.0 - 2.0 * gap) / 3.0;
    for (i, (value, label)) in tiles.iter().enumerate() {
        let tx = 72.0 + i as f64 * (tile_w + gap);
        let _ = write!(
            svg,
            r##"<rect x="{tx}" y="{tile_y}" width="{tile_w}" height="168" rx="28" fill="{line}"/>
<rect x="{tx}" y="{tile_y}" width="{tile_w}" height="160" rx="28" fill="{panel}"/>
<text x="{cx}" y="{vy}" text-anchor="middle" font-size="64" font-weight="900" fill="{text}">{value}</text>
<text x="{cx}" y="{ly}" text-anchor="middle" font-size="28" font-weight="700" fill="{muted}">{label}</text>
"##,
            line = p.line,
            panel = p.panel,
            cx = tx + tile_w / 2.0,
            vy = tile_y + 84.0,
            ly = tile_y + 128.0,
            text = p.text,
            muted = p.muted,
        );
    }

    let _ = write!(
        svg,
        r##"<text x="72" y="{fy}" font-size="30" font-weight="800" fill="{muted}">Level {level} · Eat the world, start with your city</text>
</svg>"##,
        fy = HEIGHT as f64 - 64.0,
        level = profile.level,
        muted = p.muted,
    );
    svg
}

fn icon_base64() -> &'static str {
    static ICON: OnceLock<String> = OnceLock::new();
    ICON.get_or_init(|| {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(include_str!("../../web/static/icon.svg"))
    })
}

/// Equal Earth projection (Šavrič, Patterson, Jenny 2018), λ and φ in radians.
fn equal_earth(lng: f64, lat: f64) -> (f64, f64) {
    const A1: f64 = 1.340264;
    const A2: f64 = -0.081106;
    const A3: f64 = 0.000893;
    const A4: f64 = 0.003796;
    let m = 3f64.sqrt() / 2.0;
    let theta = (m * lat.sin()).asin();
    let t2 = theta * theta;
    let t6 = t2 * t2 * t2;
    let x = lng * theta.cos() / (m * (A1 + 3.0 * A2 * t2 + t6 * (7.0 * A3 + 9.0 * A4 * t2)));
    let y = theta * (A1 + A2 * t2 + t6 * (A3 + A4 * t2));
    (x, y)
}

/// The latitudes the map shows: northern Greenland down to Tierra del Fuego.
const NORTH: f64 = 84.0;
const SOUTH: f64 = -57.0;

fn map_scale() -> f64 {
    let (right, _) = equal_earth(std::f64::consts::PI, 0.0);
    MAP_W / (2.0 * right)
}

fn map_height() -> f64 {
    let (_, top) = equal_earth(0.0, NORTH.to_radians());
    let (_, bottom) = equal_earth(0.0, SOUTH.to_radians());
    ((top - bottom) * map_scale()).round()
}

#[derive(Deserialize)]
struct WorldCountry {
    code: String,
    rings: Vec<Vec<f64>>,
}

/// SVG path data per country, projected into the card's map box. Built once.
fn world_paths() -> &'static [(String, String)] {
    static PATHS: OnceLock<Vec<(String, String)>> = OnceLock::new();
    PATHS.get_or_init(|| {
        let world: Vec<WorldCountry> =
            serde_json::from_str(include_str!("../../mobile/shared/world.json")).expect("world.json is valid");
        let scale = map_scale();
        let (_, top) = equal_earth(0.0, NORTH.to_radians());
        let cx = MAP_X + MAP_W / 2.0;
        world
            .into_iter()
            .map(|country| {
                let mut d = String::new();
                for ring in &country.rings {
                    for (i, pair) in ring.chunks_exact(2).enumerate() {
                        // Rings may run past 180° to stay continuous; the clip hides the overflow.
                        let (x, y) = equal_earth(pair[0].to_radians(), pair[1].to_radians());
                        let px = cx + x * scale;
                        let py = MAP_Y + (top - y) * scale;
                        let _ = write!(d, "{}{px:.1} {py:.1}", if i == 0 { "M" } else { "L" });
                    }
                    d.push('Z');
                }
                (country.code, d)
            })
            .collect()
    })
}

fn options() -> &'static usvg::Options<'static> {
    static OPTIONS: OnceLock<usvg::Options<'static>> = OnceLock::new();
    OPTIONS.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        for font in [
            &include_bytes!("../assets/fonts/nunito_bold.ttf")[..],
            &include_bytes!("../assets/fonts/nunito_extrabold.ttf")[..],
            &include_bytes!("../assets/fonts/nunito_black.ttf")[..],
        ] {
            db.load_font_data(font.to_vec());
        }
        usvg::Options { font_family: "Nunito".into(), fontdb: Arc::new(db), ..Default::default() }
    })
}

/// Rasterizes card SVG to PNG bytes.
pub fn render(svg: &str) -> anyhow::Result<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, options())?;
    let mut pixmap = tiny_skia::Pixmap::new(WIDTH, HEIGHT).ok_or_else(|| anyhow::anyhow!("empty canvas"))?;
    resvg::render(&tree, tiny_skia::Transform::identity(), &mut pixmap.as_mut());
    Ok(pixmap.encode_png()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::PlaceRow;

    fn row(place: &str, country: &str, visited: bool) -> PlaceRow {
        PlaceRow { place_id: place.into(), country: country.into(), visited }
    }

    #[test]
    fn headline_counts_countries_and_escapes_the_city() {
        let rows = [row("a", "JP", true), row("b", "IT", false)];
        let summary = game::summarize(&rows, Lifetime { visits: 3, countries: 1, ..Default::default() });
        let svg = card_svg(&summary, Some("Ghent <&> Co"), &LIGHT);
        assert!(svg.contains("I’ve tasted 1 country"));
        assert!(svg.contains("without leaving Ghent &lt;&amp;&gt; Co"));
        assert!(svg.contains("1/2"));
        let empty = card_svg(&game::summarize(&[], Lifetime::default()), None, &DARK);
        assert!(empty.contains("Eating the world"));
    }

    #[test]
    fn renders_a_png_with_text() {
        let rows = [row("a", "JP", true), row("b", "FR", true), row("c", "MX", false)];
        let summary = game::summarize(&rows, Lifetime { visits: 2, countries: 2, dishes: 1, most_at_one_place: 1 });
        let png = render(&card_svg(&summary, Some("Leuven"), &LIGHT)).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        // Text and the map make a card far bigger than a plain background.
        assert!(png.len() > 50_000, "only {} bytes", png.len());
    }

    #[test]
    fn long_city_names_are_shortened() {
        assert_eq!(shorten("Llanfairpwllgwyngyllgogerychwyrndrobwllllantysiliogogogoch", 10), "Llanfairp…");
        assert_eq!(shorten("Gent", 10), "Gent");
    }
}
