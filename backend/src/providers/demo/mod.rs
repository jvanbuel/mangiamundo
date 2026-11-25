//! Offline provider with made-up restaurants in a few cities, each split into two
//! made-up postal codes. Useful for development and tests: the same area always yields
//! the same places, and nothing leaves the machine.

use async_trait::async_trait;

use super::{AreaDirectory, AreaInfo, Boundary, FoundPlace, Located, PlacesProvider};
use crate::geo::Shape;

mod cities;
mod rng;

use cities::{city_of, City, CITIES};

pub struct Demo;

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
