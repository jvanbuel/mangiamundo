//! An area's progress: per country, per continent and for the whole world.

use std::collections::{BTreeMap, HashSet};

use serde::Serialize;

use super::badges::{badges, Badge};
use super::xp::{profile, Profile};
use super::{Lifetime, PlaceRow, Status, DIAMOND_PLACES};
use crate::catalog::{self, CONTINENTS};

#[derive(Debug, Serialize, PartialEq, utoipa::ToSchema)]
pub struct CountryProgress {
    pub code: String,
    pub continent: String,
    /// Places of this country in the area.
    pub places: u32,
    pub visited: u32,
    /// Places to visit for diamond: [`DIAMOND_PLACES`], or every place when there are fewer.
    pub goal: u32,
    pub status: Status,
}

#[derive(Debug, Serialize, PartialEq, utoipa::ToSchema)]
pub struct GroupProgress {
    pub code: String,
    pub name: String,
    /// Countries of this group that have at least one place in the area.
    pub available: u32,
    /// Available countries with at least one visited place.
    pub discovered: u32,
    /// Available countries at diamond.
    pub completed: u32,
}

#[derive(Debug, Serialize, PartialEq, utoipa::ToSchema)]
pub struct Summary {
    pub countries: Vec<CountryProgress>,
    pub continents: Vec<GroupProgress>,
    pub world: GroupProgress,
    pub places: u32,
    pub places_visited: u32,
    pub badges: Vec<Badge>,
    pub profile: Profile,
}

pub fn summarize(rows: &[PlaceRow], lifetime: Lifetime) -> Summary {
    let mut by_country: BTreeMap<&str, (u32, u32)> = BTreeMap::new();
    let mut all_places = HashSet::new();
    let mut visited_places = HashSet::new();
    for row in rows {
        let entry = by_country.entry(row.country.as_str()).or_default();
        entry.0 += 1;
        entry.1 += u32::from(row.visited);
        all_places.insert(row.place_id.as_str());
        if row.visited {
            visited_places.insert(row.place_id.as_str());
        }
    }

    let countries: Vec<CountryProgress> = by_country
        .into_iter()
        .filter_map(|(code, (places, visited))| {
            Some(CountryProgress {
                code: code.to_string(),
                continent: catalog::country(code)?.continent.clone(),
                places,
                visited,
                goal: places.min(DIAMOND_PLACES),
                status: Status::of(places, visited),
            })
        })
        .collect();

    let group = |code: &str, name: &str, members: &[&CountryProgress]| GroupProgress {
        code: code.to_string(),
        name: name.to_string(),
        available: crate::count(members.len()),
        discovered: crate::count(members.iter().filter(|c| c.visited > 0).count()),
        completed: crate::count(
            members
                .iter()
                .filter(|c| c.status == Status::Completed)
                .count(),
        ),
    };

    let continents: Vec<GroupProgress> = CONTINENTS
        .iter()
        .map(|(code, name)| {
            let members: Vec<_> = countries.iter().filter(|c| c.continent == *code).collect();
            group(code, name, &members)
        })
        .collect();
    let world = group("WORLD", "World", &countries.iter().collect::<Vec<_>>());

    let badges = badges(
        lifetime,
        &continents,
        &world,
        crate::count(all_places.len()),
        crate::count(visited_places.len()),
    );

    Summary {
        countries,
        continents,
        world,
        places: crate::count(all_places.len()),
        places_visited: crate::count(visited_places.len()),
        badges,
        profile: profile(lifetime),
    }
}
