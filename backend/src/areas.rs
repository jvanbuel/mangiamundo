//! The areas the game is played in, and the restaurants cached for each.
//!
//! Callers ask for an area by id and get it with its places in the store. Whether that
//! took a provider request, a read of the local copy or nothing at all is decided here,
//! as is what a progress wheel shows meanwhile.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::store::{self, Area};
use crate::AppState;

/// The largest area served, in degrees of latitude and longitude: a big city fits, a
/// region or a country (which would make for a huge Overpass query) doesn't.
const MAX_AREA_DEGREES: f64 = 1.5;

/// Why an area can't be served.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid area")]
    Invalid,
    #[error("area not found")]
    NotFound,
    #[error("{0} is too large; pick a postal code or a smaller town")]
    TooLarge(String),
    #[error("the places provider is unavailable")]
    Upstream(#[source] anyhow::Error),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 100
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Returns the cached area, asking the provider when the cache is missing or stale. A
/// stale cache is still used when the provider is down.
pub async fn resolve(state: &AppState, id: &str) -> Result<Area, Error> {
    if !valid_id(id) {
        return Err(Error::Invalid);
    }
    let cached = store::find_area(&state.db, id).await?;
    if let Some(area) = &cached {
        if fresh(state, area) {
            return Ok(area.clone());
        }
    }

    let _done = state.loads.begin(id);
    // The outline rarely changes; keep it for refreshes.
    let boundary = if let Some(area) = &cached {
        area.boundary.clone()
    } else {
        state.loads.step(id, 0.0, 15.0, "Finding the area", 2.0);
        let boundary = state
            .areas
            .boundary(id)
            .await
            .map_err(Error::Upstream)?
            .ok_or(Error::NotFound)?;
        let b = boundary.shape.bbox();
        if b.north - b.south > MAX_AREA_DEGREES || b.east - b.west > MAX_AREA_DEGREES {
            return Err(Error::TooLarge(boundary.info.name));
        }
        boundary
    };
    let expected = state.places.expected_secs(&boundary);
    state
        .loads
        .step(id, 15.0, 95.0, "Looking for restaurants", expected);
    // Another server starts the estimate over from where the wheel is.
    let next_server = || state.loads.restart(id, "Trying another server", expected);
    match state.places.search(&boundary, &next_server).await {
        Ok(found) => {
            state.loads.step(id, 95.0, 100.0, "Counting countries", 0.5);
            let area = Area {
                boundary,
                provider: state.places.name().into(),
                fetched_at: state.now(),
            };
            store::save_area(&state.db, &area, &found).await?;
            tracing::info!(area = area.id(), places = found.len(), "area refreshed");
            Ok(area)
        }
        Err(error) => match cached {
            Some(area) => {
                tracing::warn!(
                    error = format!("{error:#}"),
                    "provider failed, serving stale area"
                );
                Ok(area)
            }
            None => Err(Error::Upstream(error)),
        },
    }
}

/// The area when it is already cached, or `None`. For background work, which must never
/// wait on a remote server: a stale area is refreshed from the provider's local copy when
/// it has one, and served as it is otherwise.
pub async fn cached(state: &AppState, id: &str) -> anyhow::Result<Option<Area>> {
    let Some(area) = store::find_area(&state.db, id).await? else {
        return Ok(None);
    };
    if fresh(state, &area) {
        return Ok(Some(area));
    }
    let Some(found) = state.places.local(&area.boundary).await? else {
        return Ok(Some(area));
    };
    let area = Area {
        fetched_at: state.now(),
        ..area
    };
    store::save_area(&state.db, &area, &found).await?;
    Ok(Some(area))
}

fn fresh(state: &AppState, area: &Area) -> bool {
    state.now() - area.fetched_at < state.cache_ttl_secs
}

/// The step an area's fetch is in: it runs from `from` to `to` percent and should take
/// about `expected_secs`.
struct LoadStep {
    from: f64,
    to: f64,
    text: &'static str,
    started: Instant,
    expected_secs: f64,
}

/// An area being fetched: how many requests are fetching it, and the latest step.
#[derive(Default)]
struct Load {
    running: usize,
    step: Option<LoadStep>,
}

/// Areas being fetched right now, by area id. In memory: the server runs as one machine.
#[derive(Clone, Default)]
pub struct Loads(Arc<Mutex<HashMap<String, Load>>>);

impl Loads {
    /// Counts a fetch of the area until the returned guard drops, also on an error.
    pub fn begin<'a>(&'a self, id: &'a str) -> LoadDone<'a> {
        self.0
            .lock()
            .unwrap()
            .entry(id.to_string())
            .or_default()
            .running += 1;
        LoadDone(self, id)
    }

    pub fn step(&self, id: &str, from: f64, to: f64, text: &'static str, expected_secs: f64) {
        if let Some(load) = self.0.lock().unwrap().get_mut(id) {
            load.step = Some(LoadStep {
                from,
                to,
                text,
                started: Instant::now(),
                expected_secs,
            });
        }
    }

    /// Starts a new stretch of the current step from where it is now, with a new text.
    fn restart(&self, id: &str, text: &'static str, expected_secs: f64) {
        if let Some((percent, _)) = self.progress(id) {
            self.step(id, f64::from(percent), 95.0, text, expected_secs);
        }
    }

    /// The percentage and what is happening, or None when the area isn't loading.
    pub fn progress(&self, id: &str) -> Option<(u8, &'static str)> {
        let loads = self.0.lock().unwrap();
        let step = loads.get(id)?.step.as_ref()?;
        let percent = estimate(
            step.from,
            step.to,
            step.started.elapsed().as_secs_f64(),
            step.expected_secs,
        );
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "steps run from 0 to 100"
        )]
        Some((percent as u8, step.text))
    }
}

/// Ends one fetch of an area; the progress goes when the last concurrent fetch ends.
pub struct LoadDone<'a>(&'a Loads, &'a str);

impl Drop for LoadDone<'_> {
    fn drop(&mut self) {
        let mut loads = self.0 .0.lock().unwrap();
        if let Some(load) = loads.get_mut(self.1) {
            load.running -= 1;
            if load.running == 0 {
                loads.remove(self.1);
            }
        }
    }
}

/// Progress within a step that reports none: 63% of the way after the expected time, 95%
/// after three times that, and never the end, which only the next step reaches.
fn estimate(from: f64, to: f64, elapsed_secs: f64, expected_secs: f64) -> f64 {
    from + (to - from) * (1.0 - (-elapsed_secs / expected_secs.max(0.1)).exp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_estimate_slows_down_and_never_reaches_the_end_of_its_step() {
        assert_eq!(estimate(15.0, 95.0, 0.0, 20.0), 15.0);
        let after_expected = estimate(15.0, 95.0, 20.0, 20.0);
        assert!((65.0..66.0).contains(&after_expected), "{after_expected}");
        assert!(estimate(15.0, 95.0, 600.0, 20.0) < 95.0);
    }

    #[test]
    fn progress_stays_until_the_last_concurrent_fetch_ends() {
        let loads = Loads::default();
        let first = loads.begin("leuven");
        let second = loads.begin("leuven");
        loads.step("leuven", 15.0, 95.0, "Looking for restaurants", 20.0);
        assert_eq!(
            loads.progress("leuven"),
            Some((15, "Looking for restaurants"))
        );
        drop(first);
        assert_eq!(
            loads.progress("leuven"),
            Some((15, "Looking for restaurants"))
        );
        drop(second);
        assert_eq!(loads.progress("leuven"), None);
    }
}
