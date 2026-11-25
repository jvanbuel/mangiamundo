//! Overpass: restaurants by area, asked of several servers.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::stream::{FuturesUnordered, StreamExt};

use super::elements::{elements, places, Element};
use super::queries::{element_query, query};
use crate::geo::Bbox;
use crate::providers::{Boundary, FoundPlace, Lookup, PlacesProvider};

/// For an area: how long a server gets before the next one is asked too.
const NEXT_SERVER_AFTER: Duration = Duration::from_secs(15);
/// At most this many area requests at once, however many servers are configured.
const MAX_RUNNING: usize = 3;
/// Expected time of an area query until one has been measured.
const FIRST_GUESS_SECS: f64 = 20.0;

pub struct Overpass {
    pub(super) http: reqwest::Client,
    urls: Vec<String>,
    /// Running average of how long successful area queries took, in seconds.
    pub(super) usual_secs: Mutex<f64>,
}

impl Overpass {
    pub fn new(http: reqwest::Client, urls: Vec<String>) -> Self {
        Self {
            http,
            urls,
            usual_secs: Mutex::new(FIRST_GUESS_SECS),
        }
    }

    async fn fetch<T>(
        &self,
        url: &str,
        query: &str,
        timeout: Duration,
        read: fn(Vec<Element>) -> T,
    ) -> anyhow::Result<T> {
        let body = self
            .http
            .post(url)
            .timeout(timeout)
            .form(&[("data", query)])
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        Ok(read(elements(&body)?))
    }

    /// Asks the servers in order and returns the first answer. With `next_after`, the next
    /// server is also asked when the current one fails or hasn't answered in that time; the
    /// earlier requests keep running, as a slow server may still answer first. Without it, one
    /// server at a time, so a heavy query never runs twice.
    pub(super) async fn fetch_any<'a>(
        &'a self,
        query: &'a str,
        timeout: Duration,
        next_after: Option<Duration>,
        on_next_server: &(dyn Fn() + Send + Sync),
    ) -> anyhow::Result<Vec<FoundPlace>> {
        self.fetch_any_with(query, timeout, next_after, on_next_server, places)
            .await
    }

    /// [`Self::fetch_any`], reading the elements with `read`.
    pub(super) async fn fetch_any_with<'a, T: Send + 'a>(
        &'a self,
        query: &'a str,
        timeout: Duration,
        next_after: Option<Duration>,
        on_next_server: &(dyn Fn() + Send + Sync),
        read: fn(Vec<Element>) -> T,
    ) -> anyhow::Result<T> {
        let mut last_error = anyhow::anyhow!("no Overpass server configured");
        let mut urls = self.urls.iter();
        let ask = |url: &'a str| async move { (url, self.fetch(url, query, timeout, read).await) };
        let mut running = FuturesUnordered::new();
        let first = urls
            .next()
            .ok_or_else(|| anyhow::anyhow!("no Overpass server configured"))?;
        running.push(ask(first));
        loop {
            // Another server only while fewer than MAX_RUNNING requests are out.
            let wait = next_after.filter(|_| urls.len() > 0 && running.len() < MAX_RUNNING);
            let next = tokio::select! {
                Some((url, result)) = running.next() => match result {
                    Ok(answer) => return Ok(answer),
                    Err(error) => {
                        tracing::warn!(url, error = format!("{error:#}"), "overpass server failed, trying the next");
                        last_error = error;
                        true
                    }
                },
                () = tokio::time::sleep(wait.unwrap_or_default()), if wait.is_some() => {
                    tracing::info!("overpass server slow, asking the next one too");
                    true
                }
            };
            if next {
                match urls.next() {
                    Some(url) => {
                        on_next_server();
                        running.push(ask(url));
                    }
                    None if running.is_empty() => return Err(last_error),
                    None => {}
                }
            }
        }
    }

    /// Remembers how long a successful area query took.
    pub(super) fn measured(&self, secs: f64) {
        let mut usual = self.usual_secs.lock().unwrap();
        *usual = 0.7 * *usual + 0.3 * secs;
    }
}

#[async_trait]
impl PlacesProvider for Overpass {
    fn name(&self) -> &'static str {
        "osm"
    }

    async fn search(
        &self,
        area: &Boundary,
        on_next_server: &(dyn Fn() + Send + Sync),
    ) -> anyhow::Result<Vec<FoundPlace>> {
        let started = Instant::now();
        let query = query(area.shape.bbox(), 60);
        let mut places = self
            .fetch_any(
                &query,
                Duration::from_secs(75),
                Some(NEXT_SERVER_AFTER),
                on_next_server,
            )
            .await?;
        let secs = started.elapsed().as_secs_f64();
        tracing::info!(secs, places = places.len(), "overpass area query");
        self.measured(secs);
        places.retain(|p| area.shape.contains(p.lat, p.lng));
        Ok(places)
    }

    fn expected_secs(&self, _area: &Boundary) -> f64 {
        *self.usual_secs.lock().unwrap()
    }

    async fn place(&self, id: &str) -> anyhow::Result<Lookup> {
        let query =
            element_query(id).ok_or_else(|| anyhow::anyhow!("not an OpenStreetMap place: {id}"))?;
        match self.read_osm_api(id).await {
            Some(Ok(lookup)) => return Ok(lookup),
            Some(Err(error)) => tracing::warn!(
                id,
                error = format!("{error:#}"),
                "osm api failed, asking overpass"
            ),
            None => {}
        }
        let places = self
            .fetch_any(
                &query,
                Duration::from_secs(30),
                Some(Duration::from_secs(8)),
                &|| (),
            )
            .await?;
        Ok(places
            .into_iter()
            .next()
            .map_or(Lookup::Gone, Lookup::Found))
    }

    async fn nearby(&self, lat: f64, lng: f64, radius_m: f64) -> anyhow::Result<Vec<FoundPlace>> {
        let dlat = radius_m / 111_320.0;
        let dlng = dlat / lat.to_radians().cos().max(0.01);
        let b = Bbox {
            south: lat - dlat,
            west: lng - dlng,
            north: lat + dlat,
            east: lng + dlng,
        };
        let mut places = self
            .fetch_any(
                &query(b, 25),
                Duration::from_secs(30),
                Some(Duration::from_secs(8)),
                &|| (),
            )
            .await?;
        places.retain(|p| crate::geo::distance_m(lat, lng, p.lat, p.lng) <= radius_m);
        Ok(places)
    }
}
