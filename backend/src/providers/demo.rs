//! Offline provider with made-up restaurants in a few cities, each split into two
//! made-up postal codes. Useful for development and tests: the same area always yields
//! the same places, and nothing leaves the machine.

use async_trait::async_trait;

use super::{AreaDirectory, AreaInfo, AreaKind, Boundary, FoundPlace, Located, PlacesProvider};
use crate::catalog::{self, CUISINES};
use crate::geo::{Bbox, Shape};

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

const NAMES: &[&str] = &[
    "Casa", "Chez", "Little", "Golden", "Mama's", "Old Town", "Royal", "Taste of",
];

struct City {
    slug: &'static str,
    name: &'static str,
    country: &'static str,
    lat: f64,
    lng: f64,
    /// Postal codes of the west and east halves.
    codes: [&'static str; 2],
}

const CITIES: &[City] = &[
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
    fn bbox(&self) -> Bbox {
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

    fn info(&self) -> AreaInfo {
        AreaInfo {
            id: format!("demo-{}", self.slug),
            name: self.name.into(),
            kind: AreaKind::City,
            detail: Some(self.country.into()),
            lat: self.lat,
            lng: self.lng,
        }
    }

    fn postal_code(&self, index: usize) -> AreaInfo {
        let b = self.half(index);
        AreaInfo {
            id: format!("demo-{}-{}", self.slug, self.codes[index].to_lowercase()),
            name: format!("{} {}", self.codes[index], self.name),
            kind: AreaKind::PostalCode,
            detail: Some(self.country.into()),
            lat: (b.south + b.north) / 2.0,
            lng: (b.west + b.east) / 2.0,
        }
    }

    fn boundaries(&self) -> [Boundary; 3] {
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
    fn places(&self) -> Vec<FoundPlace> {
        let b = self.bbox();
        let mut places = Vec::new();
        for cuisine in CUISINES {
            // Seeded per city and cuisine, so growing the catalog leaves existing places be.
            let seed = format!("{}/{}", self.slug, cuisine.country)
                .bytes()
                .fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
                    (h ^ b as u64).wrapping_mul(0x100_0000_01b3)
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
                let prefix = NAMES[(rng.next() % NAMES.len() as u64) as usize];
                places.push(FoundPlace {
                    id: format!("demo-{}-{}-{n}", self.slug, cuisine.country),
                    name: format!("{prefix} {}", country.name),
                    lat,
                    lng,
                    address: Some(format!("{} Demo Street", 1 + rng.next() % 200)),
                    url: None,
                    cuisine: Some(cuisine.osm[0].to_string()),
                    opening_hours: HOURS[(rng.next() % HOURS.len() as u64) as usize]
                        .map(str::to_string),
                    countries: vec![cuisine.country],
                });
            }
        }
        places
    }
}

fn city_of(id: &str) -> Option<&'static City> {
    CITIES
        .iter()
        .find(|c| id == format!("demo-{}", c.slug) || id.starts_with(&format!("demo-{}-", c.slug)))
}

#[async_trait]
impl PlacesProvider for Demo {
    fn name(&self) -> &'static str {
        "demo"
    }

    async fn search(
        &self,
        area: &Boundary,
        _on_next_server: &(dyn Fn() + Send + Sync),
    ) -> anyhow::Result<Vec<FoundPlace>> {
        let Some(city) = city_of(&area.info.id) else {
            return Ok(Vec::new());
        };
        Ok(city
            .places()
            .into_iter()
            .filter(|p| area.shape.contains(p.lat, p.lng))
            .collect())
    }
}

#[async_trait]
impl AreaDirectory for Demo {
    async fn search(&self, query: &str) -> anyhow::Result<Vec<AreaInfo>> {
        let needle = query.trim().to_lowercase();
        let cities = CITIES
            .iter()
            .filter(|c| c.name.to_lowercase().contains(&needle))
            .map(City::info);
        let codes = CITIES.iter().flat_map(|c| {
            (0..2)
                .filter(|&i| c.codes[i].to_lowercase().starts_with(&needle))
                .map(|i| c.postal_code(i))
                .collect::<Vec<_>>()
        });
        Ok(cities.chain(codes).collect())
    }

    async fn locate(&self, lat: f64, lng: f64) -> anyhow::Result<Located> {
        let Some(city) = CITIES
            .iter()
            .find(|c| Shape::rect(c.bbox()).contains(lat, lng))
        else {
            return Ok(Located::default());
        };
        let [_, west, east] = city.boundaries();
        let postal_code = [west, east]
            .into_iter()
            .find(|b| b.shape.contains(lat, lng))
            .map(|b| b.info);
        Ok(Located {
            city: Some(city.info()),
            postal_code,
        })
    }

    async fn boundary(&self, id: &str) -> anyhow::Result<Option<Boundary>> {
        Ok(city_of(id).and_then(|c| c.boundaries().into_iter().find(|b| b.info.id == id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn postal_codes_split_the_city() {
        let city = Demo.boundary("demo-leuven").await.unwrap().unwrap();
        let west = Demo.boundary("demo-leuven-3000").await.unwrap().unwrap();
        let east = Demo.boundary("demo-leuven-3010").await.unwrap().unwrap();
        let all = PlacesProvider::search(&Demo, &city, &|| ()).await.unwrap();
        let (w, e) = (
            PlacesProvider::search(&Demo, &west, &|| ()).await.unwrap(),
            PlacesProvider::search(&Demo, &east, &|| ()).await.unwrap(),
        );
        assert!(all.len() > 20);
        assert_eq!(w.len() + e.len(), all.len());
        assert!(
            w.iter().all(|p| all.contains(p)),
            "same ids in the city and its postal codes"
        );
        assert!(Demo.boundary("demo-nowhere").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn locates_city_and_postal_code() {
        let here = Demo.locate(50.8798, 4.69).await.unwrap();
        assert_eq!(here.city.unwrap().id, "demo-leuven");
        assert_eq!(here.postal_code.unwrap().name, "3000 Leuven");
        assert_eq!(Demo.locate(0.0, 0.0).await.unwrap(), Located::default());
        let found = AreaDirectory::search(&Demo, "300").await.unwrap();
        assert_eq!(found[0].id, "demo-leuven-3000");
    }
}
