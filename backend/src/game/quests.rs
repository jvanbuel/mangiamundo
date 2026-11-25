//! Monthly quests.

use std::collections::{BTreeMap, HashSet};

use chrono::{Datelike, Duration, NaiveDate};
use serde::Serialize;

use super::{month_start, PlaceRow, Status};
use crate::catalog;

/// A place the user checked in at, with the local days of the first and latest check-in.
#[derive(Debug, Clone)]
pub struct Tasted {
    pub place_id: String,
    pub country: String,
    pub first: NaiveDate,
    pub last: NaiveDate,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuestKind {
    /// Eat a country's food for the first time.
    NewCountry,
    /// Eat from two continents.
    Continents,
    /// Visit places of a country with a celebration this month.
    Celebration,
    /// Raise a country you already tried to its next level (gold, platinum, diamond).
    LevelUp,
    /// Check in at two places. Always on, so there is always something to do.
    CheckIns,
}

impl QuestKind {
    pub fn as_str(self) -> &'static str {
        match self {
            QuestKind::NewCountry => "new-country",
            QuestKind::Continents => "continents",
            QuestKind::Celebration => "celebration",
            QuestKind::LevelUp => "level-up",
            QuestKind::CheckIns => "check-ins",
        }
    }

    pub fn xp(self) -> u32 {
        match self {
            QuestKind::NewCountry | QuestKind::LevelUp => 100,
            QuestKind::Continents | QuestKind::Celebration => 75,
            QuestKind::CheckIns => 50,
        }
    }
}

#[derive(Debug, Serialize, PartialEq, utoipa::ToSchema)]
pub struct Quest {
    /// Stable within the month, e.g. `new-country`.
    pub id: String,
    pub kind: QuestKind,
    pub title: String,
    pub description: String,
    /// The countries that count, for quests about particular ones; empty otherwise.
    pub countries: Vec<String>,
    pub progress: u32,
    pub target: u32,
    pub done: bool,
    pub xp: u32,
}

#[derive(Debug, Serialize, PartialEq, utoipa::ToSchema)]
pub struct MonthlyQuests {
    /// e.g. `2026-11`.
    pub month: String,
    /// First and last day, inclusive.
    pub starts: NaiveDate,
    pub ends: NaiveDate,
    pub quests: Vec<Quest>,
}

/// How many of the area-specific quests run each month, besides check-ins.
const QUESTS_PER_MONTH: usize = 2;

/// This month's quests in the area. Which ones run depends on the area and the month, and
/// on what the area has and what the user had done before the month began, so they don't
/// change during the month and can always be done. They are done in the area: only this
/// month's check-ins at its places count. `tasted` is all of the user's history, any area,
/// which is what makes a country new; `celebrating` the countries with a celebration on
/// some day this month.
pub fn monthly_quests(
    area_id: &str,
    today: NaiveDate,
    rows: &[PlaceRow],
    tasted: &[Tasted],
    celebrating: &[String],
) -> MonthlyQuests {
    let first = month_start(today);
    let before: Vec<&Tasted> = tasted.iter().filter(|t| t.first < first).collect();
    let here: HashSet<&str> = rows.iter().map(|r| r.place_id.as_str()).collect();
    let this_month: Vec<&Tasted> = tasted
        .iter()
        .filter(|t| t.last >= first && here.contains(t.place_id.as_str()))
        .collect();
    let tried: HashSet<&str> = before.iter().map(|t| t.country.as_str()).collect();
    let continent = |code: &str| catalog::country(code).map(|c| c.continent.as_str());

    // Places per country in the area, and how many of them the user had before this month and has now.
    let mut by_country: BTreeMap<&str, (u32, u32, u32)> = BTreeMap::new();
    let before_places: HashSet<(&str, &str)> = before
        .iter()
        .map(|t| (t.place_id.as_str(), t.country.as_str()))
        .collect();
    for row in rows {
        let e = by_country.entry(row.country.as_str()).or_default();
        e.0 += 1;
        e.1 += u32::from(before_places.contains(&(row.place_id.as_str(), row.country.as_str())));
        e.2 += u32::from(row.visited);
    }

    let quest =
        |kind: QuestKind, title: &str, description: &str, countries, progress, target| Quest {
            id: kind.as_str().to_string(),
            kind,
            title: title.to_string(),
            description: description.to_string(),
            countries,
            progress: u32::min(progress, target),
            target,
            done: progress >= target,
            xp: kind.xp(),
        };
    let mut candidates = Vec::new();
    let untried: Vec<String> = by_country
        .keys()
        .filter(|c| !tried.contains(*c))
        .map(std::string::ToString::to_string)
        .collect();
    if !untried.is_empty() {
        let new: HashSet<&str> = this_month
            .iter()
            .map(|t| t.country.as_str())
            .filter(|c| !tried.contains(c))
            .collect();
        candidates.push(quest(
            QuestKind::NewCountry,
            "Taste a new country",
            "Eat the food of a country you haven't tried yet.",
            Vec::new(),
            crate::count(new.len()),
            1,
        ));
    }
    let continents: HashSet<&str> = by_country.keys().filter_map(|c| continent(c)).collect();
    if continents.len() >= 2 {
        let eaten: HashSet<&str> = this_month
            .iter()
            .filter_map(|t| continent(&t.country))
            .collect();
        candidates.push(quest(
            QuestKind::Continents,
            "Two continents",
            "Eat from two different continents this month.",
            Vec::new(),
            crate::count(eaten.len()),
            2,
        ));
    }
    let festive: Vec<String> = celebrating
        .iter()
        .filter(|c| by_country.contains_key(c.as_str()))
        .cloned()
        .collect();
    if !festive.is_empty() {
        let places: u32 = festive.iter().map(|c| by_country[c.as_str()].0).sum();
        let visited: HashSet<&str> = this_month
            .iter()
            .filter(|t| festive.contains(&t.country))
            .map(|t| t.place_id.as_str())
            .collect();
        candidates.push(quest(
            QuestKind::Celebration,
            "Join the celebration",
            "Visit places of a country celebrating this month.",
            festive,
            crate::count(visited.len()),
            places.min(2),
        ));
    }
    let growing: Vec<(&str, &(u32, u32, u32))> = by_country
        .iter()
        .filter(|(_, (places, before, _))| {
            *before > 0 && Status::of(*places, *before) != Status::Completed
        })
        .map(|(c, e)| (*c, e))
        .collect();
    if !growing.is_empty() {
        let raised = growing.iter().any(|(_, (places, before, now))| {
            Status::of(*places, *now) > Status::of(*places, *before)
        });
        candidates.push(quest(
            QuestKind::LevelUp,
            "Level up a country",
            "Go back to a country you tried and reach its next level.",
            growing.iter().map(|(c, _)| c.to_string()).collect(),
            u32::from(raised),
            1,
        ));
    }

    // ponytail: a byte sum is enough to vary the pick per area; a real hash if it looks samey.
    let seed = area_id.bytes().map(usize::from).sum::<usize>()
        + first.month() as usize
        + first.year().unsigned_abs() as usize;
    // Rotate the four kinds (in enum order) by the seed and keep the first that apply.
    let n = QuestKind::CheckIns as usize;
    candidates.sort_by_key(|q: &Quest| (q.kind as usize + n - seed % n) % n);
    candidates.truncate(QUESTS_PER_MONTH);
    let places_this_month: HashSet<&str> = this_month.iter().map(|t| t.place_id.as_str()).collect();
    candidates.push(quest(
        QuestKind::CheckIns,
        "Two check-ins",
        "Check in at two places this month.",
        Vec::new(),
        crate::count(places_this_month.len()),
        2,
    ));

    MonthlyQuests {
        month: first.format("%Y-%m").to_string(),
        starts: first,
        ends: month_start(first + Duration::days(31)) - Duration::days(1),
        quests: candidates,
    }
}
