//! Overpass queries.

use crate::geo::Bbox;

/// The kinds of place that count, as an Overpass tag filter.
const AMENITIES: &str = "[\"amenity\"~\"^(restaurant|fast_food|cafe|pub|bar)$\"]";

/// One element by its place id (`osm-node-123`), only while it is still a restaurant or the
/// like. `None` for ids that aren't OpenStreetMap elements.
pub(super) fn element_query(id: &str) -> Option<String> {
    let (kind, n) = id.strip_prefix("osm-")?.split_once('-')?;
    let n: u64 = n.parse().ok()?;
    matches!(kind, "node" | "way" | "relation")
        .then(|| format!("[out:json][timeout:25];{kind}(id:{n}){AMENITIES};out center tags;"))
}

/// Restaurants in the area's bounding box; [`Overpass::search`] keeps the ones inside the
/// outline. A box query works for every area, also postal codes Overpass has no area for.
///
/// [`Overpass::search`]: super::Overpass
pub(super) fn query(b: Bbox, timeout_secs: u64) -> String {
    // Full precision: rounding could cut off a place just inside an edge.
    let b = format!("({},{},{},{})", b.south, b.west, b.north, b.east);
    format!("[out:json][timeout:{timeout_secs}];nwr{AMENITIES}[\"cuisine\"][\"name\"]{b};out center tags;")
}

/// Which places can have unknown food: cafés, pubs and bars without a cuisine mostly serve
/// none to speak of.
const MEALS: &str = "[\"amenity\"~\"^(restaurant|fast_food)$\"]";

/// [`query`], plus the named restaurants with no `cuisine` at all, for the import.
pub(super) fn import_query(b: Bbox, timeout_secs: u64) -> String {
    let b = format!("({},{},{},{})", b.south, b.west, b.north, b.east);
    format!(
        "[out:json][timeout:{timeout_secs}];\
         (nwr{AMENITIES}[\"cuisine\"][\"name\"]{b};nwr{MEALS}[!\"cuisine\"][\"name\"]{b};);\
         out center tags;"
    )
}
