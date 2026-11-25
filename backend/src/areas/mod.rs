//! The areas the game is played in, and the restaurants cached for each.
//!
//! Callers ask for an area by id and get it with its places in the store. Whether that
//! took a provider request, a read of the local copy or nothing at all is decided here,
//! as is what a progress wheel shows meanwhile.

use crate::store::{self, Area};
use crate::AppState;

mod loads;

pub use loads::{LoadDone, Loads};

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
