//! OpenStreetMap: Overpass for restaurants, Nominatim for areas (cities and postal-code
//! boundaries). No API key needed.

mod elements;
mod local_copy;
mod nominatim;
mod notes;
mod osm_api;
mod overpass;
mod queries;
mod results;

pub use local_copy::{import, run_imports, WithImport, IMPORT_BBOX};
pub use nominatim::Nominatim;
pub use notes::{closed_note, cuisine_note, link, missing_place_note, note_added, note_closed};
pub use overpass::Overpass;

#[cfg(test)]
mod tests;
