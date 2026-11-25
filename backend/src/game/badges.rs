//! Badges: what they reward and how far the user is with each.

use serde::Serialize;

use super::{GroupProgress, Lifetime, DIAMOND_PLACES};

/// What a badge rewards. Clients pick the badge's icon and colour from it.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BadgeKind {
    Visits,
    Countries,
    Completed,
    Continent,
    World,
    Everything,
    /// Brought a new country to the area by adding a restaurant.
    Pioneer,
    /// Checked in so many months in a row.
    Streak,
}

#[derive(Debug, Serialize, PartialEq, utoipa::ToSchema)]
pub struct Badge {
    pub id: String,
    pub kind: BadgeKind,
    pub title: String,
    pub description: String,
    pub progress: u32,
    pub target: u32,
    pub earned: bool,
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

/// Every badge with the user's progress: `continents` and `world` are the area's, `places`
/// and `visited` its places and the ones the user visited.
pub(super) fn badges(
    lifetime: Lifetime,
    continents: &[GroupProgress],
    world: &GroupProgress,
    places: u32,
    visited: u32,
) -> Vec<Badge> {
    use BadgeKind::{Completed, Continent, Countries, Everything, Pioneer, Streak, Visits, World};

    let mut badges = vec![
        badge(
            "first-bite",
            Visits,
            "First check-in",
            "Check in at any place.",
            lifetime.visits,
            1,
        ),
        badge(
            "flags-5",
            Countries,
            "5 countries",
            "Eat the food of 5 countries.",
            lifetime.countries,
            5,
        ),
        badge(
            "flags-10",
            Countries,
            "10 countries",
            "Eat the food of 10 countries.",
            lifetime.countries,
            10,
        ),
        badge(
            "flags-25",
            Countries,
            "25 countries",
            "Eat the food of 25 countries.",
            lifetime.countries,
            25,
        ),
        badge(
            "complete-1",
            Completed,
            "1 diamond country",
            format!(
                "Visit {DIAMOND_PLACES} places of a country here, or all of them if it has fewer."
            ),
            world.completed,
            1,
        ),
        badge(
            "complete-5",
            Completed,
            "5 diamond countries",
            "Reach diamond in 5 countries here.",
            world.completed,
            5,
        ),
        badge(
            "pioneer",
            Pioneer,
            "Pioneer",
            "Add a restaurant that brings a new country to your city.",
            lifetime.country_bonuses,
            1,
        ),
    ];
    for months in STREAK_BADGES {
        badges.push(badge(
            format!("streak-{months}"),
            Streak,
            format!("{months} months in a row"),
            format!("Check in somewhere every month for {months} months."),
            lifetime.best_streak,
            months,
        ));
    }
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
        visited,
        places,
    ));
    badges
}

const STREAK_BADGES: [u32; 3] = [3, 6, 12];
