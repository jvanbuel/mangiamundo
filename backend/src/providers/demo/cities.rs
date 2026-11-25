//! The made-up cities, their postal codes and their restaurants.

use super::rng::Rng;
use crate::catalog::{self, CUISINES};
use crate::geo::{Bbox, Shape};
use crate::providers::{AreaInfo, AreaKind, Boundary, FoundPlace};

/// Typical restaurant hours; `None` for places whose hours nobody mapped.
const HOURS: &[Option<&str>] = &[
    Some("Mo-Su 11:30-14:00,18:00-22:00"),
    Some("Tu-Su 12:00-22:00"),
    Some("Mo-Sa 17:00-23:00"),
    Some("Mo-Fr 11:00-15:00"),
    Some("24/7"),
    None,
];

const NAMES: &[&str] = &[
    "Casa", "Chez", "Little", "Golden", "Mama's", "Old Town", "Royal", "Taste of",
];

pub(super) struct City {
    slug: &'static str,
    pub(super) name: &'static str,
    country: &'static str,
    lat: f64,
    lng: f64,
    /// Postal codes of the west and east halves.
    pub(super) codes: [&'static str; 2],
}

pub(super) const CITIES: &[City] = &[
    City {
        slug: "amsterdam",
        name: "Amsterdam",
        country: "Netherlands",
        lat: 52.3676,
        lng: 4.9041,
        codes: ["1011", "1091"],
    },
    City {
        slug: "antwerp",
        name: "Antwerp",
        country: "Belgium",
        lat: 51.2194,
        lng: 4.4025,
        codes: ["2000", "2060"],
    },
    City {
        slug: "berlin",
        name: "Berlin",
        country: "Germany",
        lat: 52.52,
        lng: 13.405,
        codes: ["10115", "10243"],
    },
    City {
        slug: "brussels",
        name: "Brussels",
        country: "Belgium",
        lat: 50.8503,
        lng: 4.3517,
        codes: ["1000", "1040"],
    },
    City {
        slug: "ghent",
        name: "Ghent",
        country: "Belgium",
        lat: 51.0543,
        lng: 3.7174,
        codes: ["9000", "9050"],
    },
    City {
        slug: "leuven",
        name: "Leuven",
        country: "Belgium",
        lat: 50.8798,
        lng: 4.7005,
        codes: ["3000", "3010"],
    },
    City {
        slug: "london",
        name: "London",
        country: "United Kingdom",
        lat: 51.5072,
        lng: -0.1276,
        codes: ["WC2N", "E1"],
    },
    City {
        slug: "new-york",
        name: "New York",
        country: "United States",
        lat: 40.7128,
        lng: -74.006,
        codes: ["10007", "10002"],
    },
    City {
        slug: "paris",
        name: "Paris",
        country: "France",
        lat: 48.8566,
        lng: 2.3522,
        codes: ["75001", "75011"],
    },
    City {
        slug: "tokyo",
        name: "Tokyo",
        country: "Japan",
        lat: 35.6762,
        lng: 139.6503,
        codes: ["100-0001", "130-0001"],
    },
];

impl City {
    /// About 7 by 7 km around the centre.
    pub(super) fn bbox(&self) -> Bbox {
        let dlng = 0.03 / self.lat.to_radians().cos();
        Bbox {
            south: self.lat - 0.03,
            west: self.lng - dlng,
            north: self.lat + 0.03,
            east: self.lng + dlng,
        }
    }

    fn half(&self, index: usize) -> Bbox {
        let b = self.bbox();
        let middle = self.lng;
        if index == 0 {
            Bbox { east: middle, ..b }
        } else {
            Bbox { west: middle, ..b }
        }
    }

    pub(super) fn info(&self) -> AreaInfo {
        AreaInfo {
            id: format!("demo-{}", self.slug),
            name: self.name.into(),
            kind: AreaKind::City,
            detail: Some(self.country.into()),
            lat: self.lat,
            lng: self.lng,
        }
    }

    pub(super) fn postal_code(&self, index: usize) -> AreaInfo {
        let b = self.half(index);
        AreaInfo {
            id: format!("demo-{}-{}", self.slug, self.codes[index].to_lowercase()),
            name: format!("{} {}", self.codes[index], self.name),
            kind: AreaKind::PostalCode,
            detail: Some(self.country.into()),
            lat: f64::midpoint(b.south, b.north),
            lng: f64::midpoint(b.west, b.east),
        }
    }

    pub(super) fn boundaries(&self) -> [Boundary; 3] {
        [
            Boundary {
                info: self.info(),
                shape: Shape::rect(self.bbox()),
            },
            Boundary {
                info: self.postal_code(0),
                shape: Shape::rect(self.half(0)),
            },
            Boundary {
                info: self.postal_code(1),
                shape: Shape::rect(self.half(1)),
            },
        ]
    }

    /// The city's restaurants; a postal code shows the ones inside it.
    pub(super) fn places(&self) -> Vec<FoundPlace> {
        let b = self.bbox();
        let mut places = Vec::new();
        for cuisine in CUISINES {
            // Seeded per city and cuisine, so growing the catalog leaves existing places be.
            let seed = format!("{}/{}", self.slug, cuisine.country)
                .bytes()
                .fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
                    (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
                });
            let mut rng = Rng(seed);
            let country = catalog::country(cuisine.country).expect("catalog is consistent");
            // Roughly three in ten cuisines exist in any given city, and its own one always.
            if rng.unit() > 0.3 && country.name != self.country {
                continue;
            }
            let count = 1 + (rng.next() % 4) as usize;
            for n in 0..count {
                let lat = b.south + rng.unit() * (b.north - b.south);
                let lng = b.west + rng.unit() * (b.east - b.west);
                let prefix = rng.pick(NAMES);
                places.push(FoundPlace {
                    id: format!("demo-{}-{}-{n}", self.slug, cuisine.country),
                    name: format!("{prefix} {}", country.name),
                    lat,
                    lng,
                    address: Some(format!("{} Demo Street", 1 + rng.next() % 200)),
                    url: None,
                    cuisine: Some(cuisine.osm[0].to_string()),
                    opening_hours: rng.pick(HOURS).map(str::to_string),
                    countries: vec![cuisine.country],
                });
            }
        }
        places
    }
}

pub(super) fn city_of(id: &str) -> Option<&'static City> {
    CITIES
        .iter()
        .find(|c| id == format!("demo-{}", c.slug) || id.starts_with(&format!("demo-{}-", c.slug)))
}
