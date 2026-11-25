//! Reading Nominatim's and Photon's answers as areas.

use serde_json::Value;

use crate::providers::{AreaInfo, AreaKind};

/// What counts as a city: the municipality level, not districts or regions.
pub(super) const CITY_TYPES: &[&str] = &["city", "town", "village", "municipality"];

pub(super) fn results(found: &Value) -> impl Iterator<Item = &Value> {
    found.as_array().into_iter().flatten()
}

pub(super) fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str().filter(|s| !s.is_empty())
}

/// A Nominatim result as an area, when it is a city or postal-code boundary.
pub(super) fn area_info(r: &Value) -> Option<AreaInfo> {
    if text(r, "osm_type")? != "relation" || text(r, "category")? != "boundary" {
        return None;
    }
    let address = r.get("address").cloned().unwrap_or_default();
    let place = CITY_TYPES.iter().find_map(|k| text(&address, k));
    let (kind, name) = match (text(r, "type")?, text(r, "addresstype")) {
        ("postal_code", _) => {
            let code = text(&address, "postcode").or(text(r, "name"))?;
            (
                AreaKind::PostalCode,
                place.map_or(code.to_string(), |p| format!("{code} {p}")),
            )
        }
        ("administrative", Some(t)) if CITY_TYPES.contains(&t) => {
            (AreaKind::City, text(r, "name").or(place)?.to_string())
        }
        _ => return None,
    };
    let detail = [
        text(&address, "state").or(text(&address, "county")),
        text(&address, "country"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(", ");
    Some(AreaInfo {
        id: format!("osm-relation-{}", r.get("osm_id")?.as_u64()?),
        name,
        kind,
        detail: (!detail.is_empty()).then_some(detail),
        lat: text(r, "lat")?.parse().ok()?,
        lng: text(r, "lon")?.parse().ok()?,
    })
}

/// The cities and postal codes among Photon's results.
pub(super) fn photon_areas(found: &Value) -> Vec<AreaInfo> {
    let features = found
        .get("features")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();
    features
        .filter_map(|f| {
            let p = f.get("properties")?;
            if text(p, "osm_type")? != "R" {
                return None;
            }
            let name = text(p, "name")?;
            let (kind, name) = match (text(p, "type")?, text(p, "osm_value")) {
                ("city", _) => (AreaKind::City, name.to_string()),
                (_, Some("postcode")) => (
                    AreaKind::PostalCode,
                    text(p, "city").map_or(name.to_string(), |city| format!("{name} {city}")),
                ),
                _ => return None,
            };
            let detail = [text(p, "state").or(text(p, "county")), text(p, "country")]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(", ");
            let point = f.get("geometry")?.get("coordinates")?.as_array()?;
            Some(AreaInfo {
                id: format!("osm-relation-{}", p.get("osm_id")?.as_u64()?),
                name,
                kind,
                detail: (!detail.is_empty()).then_some(detail),
                lat: point.get(1)?.as_f64()?,
                lng: point.first()?.as_f64()?,
            })
        })
        .collect()
}
