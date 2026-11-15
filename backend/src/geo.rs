const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// Great-circle distance in metres (haversine).
pub fn distance_m(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = p2 - p1;
    let dl = (lng2 - lng1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_M * a.sqrt().asin()
}

/// The point `distance_m` metres from the origin along `bearing` (radians). Accurate
/// enough for city-scale offsets.
pub fn offset(lat: f64, lng: f64, distance_m: f64, bearing: f64) -> (f64, f64) {
    let dlat = (distance_m * bearing.cos() / EARTH_RADIUS_M).to_degrees();
    let dlng = (distance_m * bearing.sin() / (EARTH_RADIUS_M * lat.to_radians().cos())).to_degrees();
    (lat + dlat, lng + dlng)
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
    fn offset_round_trips() {
        let (lat, lng) = offset(50.0, 4.0, 5_000.0, 1.0);
        assert!((distance_m(50.0, 4.0, lat, lng) - 5_000.0).abs() < 20.0);
    }
}
