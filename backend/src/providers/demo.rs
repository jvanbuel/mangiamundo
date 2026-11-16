//! Offline provider with made-up restaurants. Useful for development and tests:
//! the same location always yields the same places, and nothing leaves the machine.

use async_trait::async_trait;

use super::{FoundPlace, GeoResult, Geocoder, PlacesProvider};
use crate::catalog::{self, CUISINES};
use crate::geo;

pub struct Demo;

/// Small deterministic generator (splitmix64) so results are stable across runs.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Typical restaurant hours; `None` for places whose hours nobody mapped.
const HOURS: &[Option<&str>] = &[
    Some("Mo-Su 11:30-14:00,18:00-22:00"),
    Some("Tu-Su 12:00-22:00"),
    Some("Mo-Sa 17:00-23:00"),
    Some("Mo-Fr 11:00-15:00"),
    Some("24/7"),
    None,
];

const NAMES: &[&str] = &["Casa", "Chez", "Little", "Golden", "Mama's", "Old Town", "Royal", "Taste of"];

#[async_trait]
impl PlacesProvider for Demo {
    fn name(&self) -> &'static str {
        "demo"
    }

    async fn search(&self, lat: f64, lng: f64, radius_m: u32) -> anyhow::Result<Vec<FoundPlace>> {
        let seed = ((lat * 100.0).round() as i64 as u64) << 32 ^ ((lng * 100.0).round() as i64 as u64);
        let mut rng = Rng(seed);
        let mut places = Vec::new();
        for cuisine in CUISINES {
            // Roughly four in ten cuisines exist in any given city.
            if rng.unit() > 0.4 {
                continue;
            }
            let country = catalog::country(cuisine.country).expect("catalog is consistent");
            let count = 1 + (rng.next() % 4) as usize;
            for n in 0..count {
                let distance = rng.unit().sqrt() * radius_m as f64 * 0.95;
                let bearing = rng.unit() * std::f64::consts::TAU;
                let (plat, plng) = geo::offset(lat, lng, distance, bearing);
                let prefix = NAMES[(rng.next() % NAMES.len() as u64) as usize];
                places.push(FoundPlace {
                    id: format!("demo-{seed:x}-{}-{n}", cuisine.country),
                    name: format!("{prefix} {}", country.name),
                    lat: plat,
                    lng: plng,
                    address: Some(format!("{} Demo Street", 1 + rng.next() % 200)),
                    url: None,
                    cuisine: Some(cuisine.osm[0].to_string()),
                    opening_hours: HOURS[(rng.next() % HOURS.len() as u64) as usize].map(str::to_string),
                    countries: vec![cuisine.country],
                });
            }
        }
        Ok(places)
    }
}

const CITIES: &[(&str, f64, f64)] = &[
    ("Amsterdam, Netherlands", 52.3676, 4.9041),
    ("Antwerp, Belgium", 51.2194, 4.4025),
    ("Berlin, Germany", 52.52, 13.405),
    ("Brussels, Belgium", 50.8503, 4.3517),
    ("Ghent, Belgium", 51.0543, 3.7174),
    ("Leuven, Belgium", 50.8798, 4.7005),
    ("London, United Kingdom", 51.5072, -0.1276),
    ("New York, United States", 40.7128, -74.006),
    ("Paris, France", 48.8566, 2.3522),
    ("Tokyo, Japan", 35.6762, 139.6503),
];

#[async_trait]
impl Geocoder for Demo {
    async fn search(&self, query: &str) -> anyhow::Result<Vec<GeoResult>> {
        let needle = query.trim().to_lowercase();
        Ok(CITIES
            .iter()
            .filter(|(name, _, _)| name.to_lowercase().contains(&needle))
            .map(|(name, lat, lng)| GeoResult { name: name.to_string(), lat: *lat, lng: *lng })
            .collect())
    }

    async fn reverse(&self, lat: f64, lng: f64) -> anyhow::Result<Option<String>> {
        Ok(CITIES
            .iter()
            .map(|(name, clat, clng)| (name, geo::distance_m(lat, lng, *clat, *clng)))
            .filter(|(_, d)| *d < 30_000.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(name, _)| name.split(',').next().unwrap_or(name).to_string()))
    }
}
