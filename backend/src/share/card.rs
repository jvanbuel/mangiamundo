//! The card image: a flat world map coloured by the player's progress, with a caption.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::hash::BuildHasher;
use std::sync::{Arc, OnceLock};

use resvg::{tiny_skia, usvg};
use serde::Deserialize;

use super::escape;
use crate::game::Status;

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
        serde_json::from_str(include_str!("../../../shared/world.json"))
            .expect("world.json is valid")
    })
}

fn fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_font_data(
                include_bytes!("../../../shared/fonts/res/font/nunito_extrabold.ttf").to_vec(),
            );
            db.load_font_data(
                include_bytes!("../../../shared/fonts/res/font/nunito_semibold.ttf").to_vec(),
            );
            Arc::new(db)
        })
        .clone()
}

/// Cuts text that wouldn't fit on the card's line, at about `max` characters.
pub(super) fn fit(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max - 1).collect();
    format!("{}…", cut.trim_end())
}

/// The card as SVG: a flat world map coloured like the app's (light theme), and the caption.
pub(super) fn card_svg<S: BuildHasher>(
    statuses: &HashMap<&str, Status, S>,
    headline: &str,
    caption: &str,
) -> String {
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
pub fn card_png<S: BuildHasher>(
    statuses: &HashMap<&str, Status, S>,
    headline: &str,
    caption: &str,
) -> anyhow::Result<Vec<u8>> {
    let options = usvg::Options {
        fontdb: fonts(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&card_svg(statuses, headline, caption), &options)?;
    let mut pixmap = tiny_skia::Pixmap::new(1200, 630).expect("the card has a size");
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap.encode_png()?)
}
