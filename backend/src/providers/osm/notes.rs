//! OSM notes: telling mappers about missing, untagged and closed places.

use std::fmt::Write as _;

use crate::{store, AppState};

/// The openstreetmap.org page of an OSM place, from an id like `osm-node-123`.
pub fn link(place_id: &str) -> Option<String> {
    let (kind, id) = place_id.strip_prefix("osm-")?.split_once('-')?;
    Some(format!("https://www.openstreetmap.org/{kind}/{id}"))
}

/// An OSM note mappers can act on: the tags to add, ready to copy.
pub fn missing_place_note(place: &store::NewPlace) -> String {
    let mut text = format!(
        "A Mangiamundo user who knows this place says a restaurant is missing here:\n\n\
         amenity=restaurant\nname={}\ncuisine={}\n",
        place.name, place.cuisine
    );
    if let Some(url) = &place.url {
        let _ = writeln!(text, "website={url}");
    }
    if let Some(address) = &place.address {
        let _ = write!(text, "\nAddress: {address}\n");
    }
    text + "\nCould someone check and add it?"
}

/// The OSM note asking mappers to tag a place's cuisine.
pub fn cuisine_note(place: &store::UnknownPlace, country: &str, cuisine: &str) -> String {
    let link = link(&place.id).map_or_else(String::new, |url| format!("\n\n{url}"));
    format!(
        "A Mangiamundo user who knows this place says {} serves food from {country}:\n\n\
         cuisine={cuisine}{link}\n\nCould someone check and add the tag?",
        place.name
    )
}

/// The OSM note saying a place has closed. `page` is its page from [`link`].
pub fn closed_note(name: &str, page: &str) -> String {
    format!(
        "Two Mangiamundo users reported that \"{name}\" ({page}) has closed. \
         Could someone check and update or remove it?"
    )
}

/// In the background, tells mappers about a place a user added (or suggested a cuisine for),
/// and keeps the note's id with the pending place. Nothing when notes are off.
pub fn note_added(state: &AppState, place_id: String, lat: f64, lng: f64, text: String) {
    let Some(url) = state.osm_notes_url.clone() else {
        return;
    };
    let (db, token) = (state.db.clone(), state.osm_notes_token.clone());
    tokio::spawn(async move {
        if let Some(note) = post_note(url, token, lat, lng, text).await {
            if let Err(error) = store::set_osm_note(&db, &place_id, note).await {
                tracing::warn!(%error, "could not keep the osm note id");
            }
        }
    });
}

/// In the background, tells mappers that an OSM place has closed. Nothing when notes are
/// off or the place isn't from OSM.
pub fn note_closed(state: &AppState, place_id: &str, name: &str, lat: f64, lng: f64) {
    let (Some(url), Some(page)) = (state.osm_notes_url.clone(), link(place_id)) else {
        return;
    };
    tokio::spawn(post_note(
        url,
        state.osm_notes_token.clone(),
        lat,
        lng,
        closed_note(name, &page),
    ));
}

/// Posts an OSM note, as the account `token` belongs to or anonymously, and returns its id.
/// Best effort: a failure is logged, not retried.
async fn post_note(
    url: String,
    token: Option<String>,
    lat: f64,
    lng: f64,
    text: String,
) -> Option<i64> {
    // The `.json` endpoint answers with the note as GeoJSON instead of XML.
    let result = async {
        let mut request = reqwest::Client::new()
            .post(format!("{url}.json"))
            .header(
                reqwest::header::USER_AGENT,
                concat!("mangiamundo/", env!("CARGO_PKG_VERSION")),
            )
            .form(&[
                ("lat", lat.to_string()),
                ("lon", lng.to_string()),
                ("text", text),
            ]);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        request
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await
    }
    .await;
    match result {
        Ok(note) => {
            let id = note["properties"]["id"].as_i64();
            tracing::info!(lat, lng, id, "posted osm note");
            id
        }
        Err(error) => {
            tracing::warn!(%error, "osm note failed");
            None
        }
    }
}
