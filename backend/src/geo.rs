const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// Great-circle distance in metres (haversine).
pub fn distance_m(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = p2 - p1;
    let dl = (lng2 - lng1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_M * a.sqrt().asin()
}

/// [`distance_m`] in whole metres.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a distance on Earth is positive and far below u32::MAX metres"
)]
pub fn rounded_m(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> u32 {
    distance_m(lat1, lng1, lat2, lng2).round() as u32
}

/// An area's outline: polygons as in a GeoJSON MultiPolygon, `[lng, lat]` pairs, each
/// polygon an outer ring followed by its holes.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Shape(pub Vec<Vec<Vec<[f64; 2]>>>);

/// South, west, north, east, in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bbox {
    pub south: f64,
    pub west: f64,
    pub north: f64,
    pub east: f64,
}

impl Shape {
    /// From GeoJSON geometry: a Polygon or MultiPolygon. Anything else has no area.
    pub fn from_geojson(geometry: &serde_json::Value) -> Option<Shape> {
        let coordinates = geometry.get("coordinates")?.clone();
        let shape = match geometry.get("type")?.as_str()? {
            "Polygon" => Shape(vec![serde_json::from_value(coordinates).ok()?]),
            "MultiPolygon" => Shape(serde_json::from_value(coordinates).ok()?),
            _ => return None,
        };
        (!shape.0.is_empty()
            && shape
                .0
                .iter()
                .all(|p| p.first().is_some_and(|ring| ring.len() >= 4)))
        .then_some(shape)
    }

    /// A rectangle, for made-up areas.
    pub fn rect(b: Bbox) -> Shape {
        Shape(vec![vec![vec![
            [b.west, b.south],
            [b.east, b.south],
            [b.east, b.north],
            [b.west, b.north],
            [b.west, b.south],
        ]]])
    }

    pub fn bbox(&self) -> Bbox {
        let mut b = Bbox {
            south: f64::MAX,
            west: f64::MAX,
            north: f64::MIN,
            east: f64::MIN,
        };
        for [lng, lat] in self.0.iter().filter_map(|p| p.first()).flatten() {
            b.south = b.south.min(*lat);
            b.north = b.north.max(*lat);
            b.west = b.west.min(*lng);
            b.east = b.east.max(*lng);
        }
        b
    }

    pub fn contains(&self, lat: f64, lng: f64) -> bool {
        self.0.iter().any(|polygon| {
            let mut rings = polygon.iter();
            rings.next().is_some_and(|outer| in_ring(outer, lat, lng))
                && !rings.any(|hole| in_ring(hole, lat, lng))
        })
    }
}

/// Even-odd ray casting; fine at city scale, where the earth is flat enough.
fn in_ring(ring: &[[f64; 2]], lat: f64, lng: f64) -> bool {
    let mut inside = false;
    let mut j = ring.len().saturating_sub(1);
    for i in 0..ring.len() {
        let ([xi, yi], [xj, yj]) = (ring[i], ring[j]);
        if (yi > lat) != (yj > lat) && lng < (xj - xi) * (lat - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brussels_to_paris_is_about_264_km() {
        let d = distance_m(50.8503, 4.3517, 48.8566, 2.3522);
        assert!((d - 264_000.0).abs() < 3_000.0, "{d}");
    }

    #[test]
    fn shapes_contain_points_but_not_their_holes() {
        let geometry = serde_json::json!({
            "type": "Polygon",
            "coordinates": [
                [[4.0, 50.0], [5.0, 50.0], [5.0, 51.0], [4.0, 51.0], [4.0, 50.0]],
                [[4.4, 50.4], [4.6, 50.4], [4.6, 50.6], [4.4, 50.6], [4.4, 50.4]]
            ]
        });
        let shape = Shape::from_geojson(&geometry).unwrap();
        assert!(shape.contains(50.2, 4.2));
        assert!(!shape.contains(50.5, 4.5), "in the hole");
        assert!(!shape.contains(52.0, 4.5), "outside");
        assert_eq!(
            shape.bbox(),
            Bbox {
                south: 50.0,
                west: 4.0,
                north: 51.0,
                east: 5.0
            }
        );
        assert!(Shape::from_geojson(
            &serde_json::json!({"type": "Point", "coordinates": [4.0, 50.0]})
        )
        .is_none());
    }
}
