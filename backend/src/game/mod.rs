//! Gamification: turns "which places exist here and which did the user visit" into
//! per-country and per-continent progress, badges, XP and a level.

use serde::Serialize;

mod badges;
mod months;
mod quests;
mod suggestion;
mod summary;
mod xp;

pub use badges::{Badge, BadgeKind};
pub use months::{month_start, streaks};
pub use quests::{monthly_quests, MonthlyQuests, Quest, QuestKind, Tasted};
pub use suggestion::{suggest, Candidate};
pub use summary::{summarize, CountryProgress, GroupProgress, Summary};
pub use xp::{
    profile, Profile, XP_NEW_CONTINENT, XP_NEW_COUNTRY, XP_PER_ADDITION, XP_PER_COUNTRY,
    XP_PER_VISIT, XP_VERIFIED,
};

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
    /// Places with at least one check-in made on the spot.
    pub verified: u32,
    pub countries: u32,
    /// Confirmed restaurants the user added, and how many of them brought a new country,
    /// or a new continent, to their area. A new continent is a new country too.
    pub added: u32,
    pub country_bonuses: u32,
    pub continent_bonuses: u32,
    /// XP from finished monthly quests.
    pub quest_xp: u32,
    /// Months in a row with a check-in, see [`streaks`].
    pub streak: u32,
    pub best_streak: u32,
}

/// What a restaurant the user adds brings to their area, decided when they add it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Bonus {
    None,
    /// Its country had no places in the area yet.
    Country,
    /// Its continent had no places in the area yet.
    Continent,
}

impl Bonus {
    pub fn as_str(self) -> &'static str {
        match self {
            Bonus::None => "none",
            Bonus::Country => "country",
            Bonus::Continent => "continent",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "country" => Bonus::Country,
            "continent" => Bonus::Continent,
            _ => Bonus::None,
        }
    }

    /// XP for one confirmed addition with this bonus.
    pub fn xp(self) -> u32 {
        XP_PER_ADDITION
            + match self {
                Bonus::None => 0,
                Bonus::Country => XP_NEW_COUNTRY,
                Bonus::Continent => XP_NEW_COUNTRY + XP_NEW_CONTINENT,
            }
    }
}

/// How far the user got with a country's food in the area. Clients show `discovered` as
/// gold and `completed` as diamond.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Available,
    /// Gold: at least one place visited.
    Discovered,
    /// At least [`PLATINUM_PLACES`] places visited.
    Platinum,
    /// Diamond: [`DIAMOND_PLACES`] places visited, or every place when there are fewer.
    Completed,
}

pub const PLATINUM_PLACES: u32 = 5;
pub const DIAMOND_PLACES: u32 = 10;

impl Status {
    /// `visited` only counts places in the area, so it never exceeds `places`.
    pub fn of(places: u32, visited: u32) -> Self {
        if visited == 0 {
            Status::Available
        } else if visited >= places.min(DIAMOND_PLACES) {
            Status::Completed
        } else if visited >= PLATINUM_PLACES {
            Status::Platinum
        } else {
            Status::Discovered
        }
    }
}

#[cfg(test)]
mod tests;
