//! XP and levels.

use serde::Serialize;

use super::Lifetime;

#[derive(Debug, Serialize, PartialEq, utoipa::ToSchema)]
pub struct Profile {
    pub xp: u32,
    pub level: u32,
    /// XP at which the current level started and at which the next one starts.
    pub level_xp: u32,
    pub next_level_xp: u32,
    pub visits: u32,
    pub countries: u32,
    /// Restaurants the user added that OpenStreetMap now has.
    pub added: u32,
    /// Months in a row with at least one check-in, up to this one. A month without one yet
    /// doesn't break it until the month is over.
    pub streak: u32,
    pub best_streak: u32,
}

pub const XP_PER_VISIT: u32 = 10;
/// Extra, once per place, for checking in on the spot.
pub const XP_VERIFIED: u32 = 15;
pub const XP_PER_COUNTRY: u32 = 50;
/// For a restaurant the user added, once OpenStreetMap has it.
pub const XP_PER_ADDITION: u32 = 30;
/// Extra when the addition's country had no places in the area yet.
pub const XP_NEW_COUNTRY: u32 = 100;
/// Extra, on top of the country's, when its continent had none either.
pub const XP_NEW_CONTINENT: u32 = 250;

pub fn profile(lifetime: Lifetime) -> Profile {
    let xp = lifetime.visits * XP_PER_VISIT
        + lifetime.verified * XP_VERIFIED
        + lifetime.countries * XP_PER_COUNTRY
        + lifetime.added * XP_PER_ADDITION
        + lifetime.country_bonuses * XP_NEW_COUNTRY
        + lifetime.continent_bonuses * XP_NEW_CONTINENT
        + lifetime.quest_xp;
    // Level n starts at 100 * (n - 1)^2 XP: 0, 100, 400, 900, ...
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the square root of a u32 over 100 fits a u32"
    )]
    let level = (f64::from(xp) / 100.0).sqrt().floor() as u32 + 1;
    Profile {
        xp,
        level,
        level_xp: 100 * (level - 1).pow(2),
        next_level_xp: 100 * level.pow(2),
        visits: lifetime.visits,
        countries: lifetime.countries,
        added: lifetime.added,
        streak: lifetime.streak,
        best_streak: lifetime.best_streak,
    }
}
