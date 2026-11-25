use std::time::{Duration, Instant};

use super::elements::{elements, is_closed, parse, places, unknown};
use super::local_copy::covered;
use super::osm_api::parse_api;
use super::queries::{element_query, import_query, query};
use super::results::{area_info, photon_areas, results};
use super::*;
use crate::geo::Bbox;
use crate::providers::{
    AreaDirectory, AreaInfo, AreaKind, FoundPlace, Lookup, PlacesProvider, Refused,
};

mod elements;
mod local_copy;
mod nominatim;
mod osm_api;
mod overpass;
mod queries;
