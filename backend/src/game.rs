//! Gamification: turns "which places exist here and which did the user visit" into
//! per-country and per-continent progress, badges, XP and a level.

use std::collections::{BTreeMap, HashSet};

use serde::Serialize;

use crate::catalog::{self, CONTINENTS};

/// One (place, country) pair inside the current area.
#[derive(Debug, Clone)]
pub struct PlaceRow {
    pub place_id: String,
    pub country: String,
    pub visited: bool,
}

/// Lifetime totals of a user, independent of the current area.
#[derive(Debug, Clone, Copy, Default)]
pub struct Lifetime {
    pub visits: u32,
    pub countries: u32,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct CountryProgress {
    pub code: String,
    pub continent: String,
    /// Places of this country in the area.
    pub places: u32,
    pub visited: u32,
}

impl CountryProgress {
    pub fn discovered(&self) -> bool {
        self.visited > 0
    }
    pub fn completed(&self) -> bool {
        self.visited == self.places
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub struct GroupProgress {
    pub code: String,
    pub name: String,
    /// Countries of this group that have at least one place in the area.
    pub available: u32,
    /// Available countries with at least one visited place.
    pub discovered: u32,
    /// Available countries where every place was visited.
    pub completed: u32,
}

/// What a badge rewards. Clients pick the badge's icon and colour from it.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BadgeKind {
    Visits,
    Countries,
    Completed,
    Continent,
    World,
    Everything,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Badge {
    pub id: String,
    pub kind: BadgeKind,
    pub title: String,
    pub description: String,
    pub progress: u32,
    pub target: u32,
    pub earned: bool,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Profile {
    pub xp: u32,
    pub level: u32,
    /// XP at which the current level started and at which the next one starts.
    pub level_xp: u32,
    pub next_level_xp: u32,
    pub visits: u32,
    pub countries: u32,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Summary {
    pub countries: Vec<CountryProgress>,
    pub continents: Vec<GroupProgress>,
    pub world: GroupProgress,
    pub places: u32,
    pub places_visited: u32,
    pub badges: Vec<Badge>,
    pub profile: Profile,
}

pub const XP_PER_VISIT: u32 = 10;
pub const XP_PER_COUNTRY: u32 = 50;

pub fn profile(lifetime: Lifetime) -> Profile {
    let xp = lifetime.visits * XP_PER_VISIT + lifetime.countries * XP_PER_COUNTRY;
    // Level n starts at 100 * (n - 1)^2 XP: 0, 100, 400, 900, ...
    let level = (xp as f64 / 100.0).sqrt().floor() as u32 + 1;
    Profile {
        xp,
        level,
        level_xp: 100 * (level - 1).pow(2),
        next_level_xp: 100 * level.pow(2),
        visits: lifetime.visits,
        countries: lifetime.countries,
    }
}

fn badge(
    id: impl Into<String>,
    kind: BadgeKind,
    title: impl Into<String>,
    description: impl Into<String>,
    progress: u32,
    target: u32,
) -> Badge {
    Badge {
        id: id.into(),
        kind,
        title: title.into(),
        description: description.into(),
        progress: progress.min(target),
        target,
        earned: target > 0 && progress >= target,
    }
}

pub fn summarize(rows: &[PlaceRow], lifetime: Lifetime) -> Summary {
    let mut by_country: BTreeMap<&str, (u32, u32)> = BTreeMap::new();
    let mut all_places = HashSet::new();
    let mut visited_places = HashSet::new();
    for row in rows {
        let entry = by_country.entry(row.country.as_str()).or_default();
        entry.0 += 1;
        entry.1 += row.visited as u32;
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
            })
        })
        .collect();

    let group = |code: &str, name: &str, members: &[&CountryProgress]| GroupProgress {
        code: code.to_string(),
        name: name.to_string(),
        available: members.len() as u32,
        discovered: members.iter().filter(|c| c.discovered()).count() as u32,
        completed: members.iter().filter(|c| c.completed()).count() as u32,
    };

    let continents: Vec<GroupProgress> = CONTINENTS
        .iter()
        .map(|(code, name)| {
            let members: Vec<_> = countries.iter().filter(|c| c.continent == *code).collect();
            group(code, name, &members)
        })
        .collect();
    let world = group("WORLD", "World", &countries.iter().collect::<Vec<_>>());

    use BadgeKind::*;
    let mut badges = vec![
        badge("first-bite", Visits, "First check-in", "Check in at any place.", lifetime.visits, 1),
        badge("flags-5", Countries, "5 countries", "Eat the food of 5 countries.", lifetime.countries, 5),
        badge("flags-10", Countries, "10 countries", "Eat the food of 10 countries.", lifetime.countries, 10),
        badge("flags-25", Countries, "25 countries", "Eat the food of 25 countries.", lifetime.countries, 25),
        badge("complete-1", Completed, "1 country complete", "Visit every place of a country here.", world.completed, 1),
        badge("complete-5", Completed, "5 countries complete", "Visit every place of 5 countries here.", world.completed, 5),
    ];
    for continent in continents.iter().filter(|c| c.available > 0) {
        badges.push(badge(
            format!("continent-{}", continent.code),
            Continent,
            continent.name.clone(),
            format!("Eat from every {} country found here.", continent.name),
            continent.discovered,
            continent.available,
        ));
    }
    badges.push(badge(
        "world",
        World,
        "Whole world",
        "Eat from every country found here.",
        world.discovered,
        world.available,
    ));
    badges.push(badge(
        "world-complete",
        Everything,
        "Every place",
        "Visit every single place here.",
        visited_places.len() as u32,
        all_places.len() as u32,
    ));

    Summary {
        countries,
        continents,
        world,
        places: all_places.len() as u32,
        places_visited: visited_places.len() as u32,
        badges,
        profile: profile(lifetime),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(place: &str, country: &str, visited: bool) -> PlaceRow {
        PlaceRow { place_id: place.into(), country: country.into(), visited }
    }

    fn earned(summary: &Summary, id: &str) -> bool {
        summary.badges.iter().find(|b| b.id == id).unwrap_or_else(|| panic!("no badge {id}")).earned
    }

    #[test]
    fn empty_area_earns_nothing() {
        let s = summarize(&[], Lifetime::default());
        assert_eq!(s.world.available, 0);
        assert!(s.badges.iter().all(|b| !b.earned));
        assert_eq!(s.profile.level, 1);
    }

    #[test]
    fn counts_countries_continents_and_world() {
        let rows = [
            row("a", "IT", true),
            row("b", "IT", true),
            row("c", "FR", true),
            row("d", "FR", false),
            row("e", "JP", false),
        ];
        let s = summarize(&rows, Lifetime { visits: 3, countries: 2 });

        let italy = s.countries.iter().find(|c| c.code == "IT").unwrap();
        assert!(italy.completed() && italy.discovered());
        let france = s.countries.iter().find(|c| c.code == "FR").unwrap();
        assert!(france.discovered() && !france.completed());

        let europe = s.continents.iter().find(|c| c.code == "EU").unwrap();
        assert_eq!((europe.available, europe.discovered, europe.completed), (2, 2, 1));
        assert_eq!((s.world.available, s.world.discovered, s.world.completed), (3, 2, 1));
        assert_eq!((s.places, s.places_visited), (5, 3));

        assert!(earned(&s, "first-bite"));
        assert!(earned(&s, "continent-EU"));
        assert!(!earned(&s, "continent-AS"));
        assert!(!earned(&s, "world"));
        assert!(earned(&s, "complete-1"));
        // Continents with nothing available in the city get no badge at all.
        assert!(s.badges.iter().all(|b| b.id != "continent-AF"));
    }

    #[test]
    fn a_place_serving_two_countries_counts_once_as_a_place() {
        let rows = [row("a", "TH", true), row("a", "VN", true)];
        let s = summarize(&rows, Lifetime { visits: 1, countries: 2 });
        assert_eq!((s.places, s.places_visited), (1, 1));
        assert_eq!(s.world.discovered, 2);
        assert!(earned(&s, "world") && earned(&s, "world-complete"));
    }

    #[test]
    fn levels_follow_the_quadratic_curve() {
        assert_eq!(profile(Lifetime { visits: 0, countries: 0 }).level, 1);
        // 1 visit in a new country: 60 XP, still level 1.
        let p = profile(Lifetime { visits: 1, countries: 1 });
        assert_eq!((p.xp, p.level, p.level_xp, p.next_level_xp), (60, 1, 0, 100));
        // 5 visits over 5 countries: 300 XP, level 2 (100..400).
        let p = profile(Lifetime { visits: 5, countries: 5 });
        assert_eq!((p.xp, p.level, p.level_xp, p.next_level_xp), (300, 2, 100, 400));
        assert_eq!(profile(Lifetime { visits: 10, countries: 6 }).level, 3);
    }
}
