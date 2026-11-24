//! Gamification: turns "which places exist here and which did the user visit" into
//! per-country and per-continent progress, badges, XP and a level.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use chrono::{Datelike, Duration, NaiveDate};
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
    let level = (xp as f64 / 100.0).sqrt().floor() as u32 + 1;
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
                goal: places.min(DIAMOND_PLACES),
                status: Status::of(places, visited),
            })
        })
        .collect();

    let group = |code: &str, name: &str, members: &[&CountryProgress]| GroupProgress {
        code: code.to_string(),
        name: name.to_string(),
        available: members.len() as u32,
        discovered: members.iter().filter(|c| c.visited > 0).count() as u32,
        completed: members
            .iter()
            .filter(|c| c.status == Status::Completed)
            .count() as u32,
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

const STREAK_BADGES: [u32; 3] = [3, 6, 12];

/// The first day of `day`'s month.
pub fn month_start(day: NaiveDate) -> NaiveDate {
    day.with_day(1).expect("every month has a first day")
}

/// The first day of the month before `day`'s.
fn previous_month(day: NaiveDate) -> NaiveDate {
    month_start(month_start(day) - Duration::days(1))
}

/// The current streak and the longest one, from the local days the user checked in. The
/// current one counts back from this month, or from last month while this one has none yet.
pub fn streaks(days: &[NaiveDate], today: NaiveDate) -> (u32, u32) {
    let months: BTreeSet<NaiveDate> = days.iter().map(|d| month_start(*d)).collect();
    let (mut run, mut best, mut last) = (0, 0, None);
    for &month in &months {
        run = if last == Some(previous_month(month)) {
            run + 1
        } else {
            1
        };
        best = best.max(run);
        last = Some(month);
    }
    let alive = last.is_some_and(|m| m >= previous_month(today));
    (if alive { run } else { 0 }, best)
}

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
/// change during the month and can always be done. `tasted` is all of the user's history,
/// any area; `celebrating` the countries with a celebration on some day this month.
pub fn monthly_quests(
    area_id: &str,
    today: NaiveDate,
    rows: &[PlaceRow],
    tasted: &[Tasted],
    celebrating: &[String],
) -> MonthlyQuests {
    let first = month_start(today);
    let before: Vec<&Tasted> = tasted.iter().filter(|t| t.first < first).collect();
    let this_month: Vec<&Tasted> = tasted.iter().filter(|t| t.last >= first).collect();
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
        e.1 += before_places.contains(&(row.place_id.as_str(), row.country.as_str())) as u32;
        e.2 += row.visited as u32;
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
        .map(|c| c.to_string())
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
            new.len() as u32,
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
            eaten.len() as u32,
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
            visited.len() as u32,
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
            raised as u32,
            1,
        ));
    }

    // ponytail: a byte sum is enough to vary the pick per area; a real hash if it looks samey.
    let seed = area_id.bytes().map(usize::from).sum::<usize>()
        + first.month() as usize
        + first.year() as usize;
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
        places_this_month.len() as u32,
        2,
    ));

    MonthlyQuests {
        month: first.format("%Y-%m").to_string(),
        starts: first,
        ends: month_start(first + Duration::days(31)) - Duration::days(1),
        quests: candidates,
    }
}

/// A place that can be suggested: the countries it serves, at least one, and how far it is.
pub struct Candidate<'a> {
    pub countries: &'a [String],
    pub distance_m: u32,
}

/// Picks one of `candidates` at random: its index, and the country the globe lands on,
/// an untasted one when the place serves several. Countries not in `tasted` and nearer
/// places are more likely. None when there are no candidates.
pub fn suggest(candidates: &[Candidate], tasted: &[String]) -> Option<(usize, String)> {
    if candidates.is_empty() {
        return None;
    }
    let weights: Vec<f64> = candidates
        .iter()
        .map(|c| suggestion_weight(c, tasted))
        .collect();
    let index = pick(&weights, random_unit());
    let countries = candidates[index].countries;
    let country = countries
        .iter()
        .find(|c| !tasted.contains(c))
        .or(countries.first())?;
    Some((index, country.clone()))
}

/// How likely a place is suggested: three times as likely when it serves a country not
/// tasted yet, and halving every 2 km away.
// ponytail: no levels (#72) or quests (#73) in the weighting yet; add them here.
fn suggestion_weight(place: &Candidate, tasted: &[String]) -> f64 {
    let new = place.countries.iter().any(|c| !tasted.contains(c));
    let near = 0.5f64.powf(f64::from(place.distance_m) / 2000.0);
    if new {
        3.0 * near
    } else {
        near
    }
}

/// The index `r` (in `[0, 1)`) lands on when each index takes its weight's share of the line.
fn pick(weights: &[f64], r: f64) -> usize {
    let mut left = r * weights.iter().sum::<f64>();
    for (i, w) in weights.iter().enumerate() {
        if left < *w {
            return i;
        }
        left -= w;
    }
    weights.len() - 1
}

fn random_unit() -> f64 {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).expect("the system has no randomness");
    (u64::from_le_bytes(bytes) >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(place: &str, country: &str, visited: bool) -> PlaceRow {
        PlaceRow {
            place_id: place.into(),
            country: country.into(),
            visited,
        }
    }

    fn earned(summary: &Summary, id: &str) -> bool {
        summary
            .badges
            .iter()
            .find(|b| b.id == id)
            .unwrap_or_else(|| panic!("no badge {id}"))
            .earned
    }

    fn day(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    fn tasted(place: &str, country: &str, first: &str, last: &str) -> Tasted {
        Tasted {
            place_id: place.into(),
            country: country.into(),
            first: day(first),
            last: day(last),
        }
    }

    fn ids(w: &MonthlyQuests) -> Vec<&str> {
        w.quests.iter().map(|q| q.id.as_str()).collect()
    }

    #[test]
    fn streak_counts_months_in_a_row() {
        let today = day("2026-11-25");
        assert_eq!(streaks(&[], today), (0, 0));
        // This month, last month (twice) and the month before: 3, across a year end too.
        let days = [
            day("2026-11-03"),
            day("2026-10-30"),
            day("2026-10-01"),
            day("2026-09-12"),
        ];
        assert_eq!(streaks(&days, today), (3, 3));
        assert_eq!(
            streaks(&[day("2026-12-31"), day("2027-01-01")], day("2027-01-20")),
            (2, 2)
        );
        // Nothing yet this month doesn't break it.
        assert_eq!(streaks(&days[1..], today), (2, 2));
        // A whole month without one does; the best run stays.
        let old = [day("2026-08-20"), day("2026-07-01"), day("2026-06-30")];
        assert_eq!(streaks(&old, today), (0, 3));
        let mixed = [&old[..], &days[..1]].concat();
        assert_eq!(streaks(&mixed, today), (1, 3));
    }

    #[test]
    fn streak_badges_use_the_best_streak() {
        let s = summarize(
            &[],
            Lifetime {
                streak: 1,
                best_streak: 4,
                ..Default::default()
            },
        );
        assert!(earned(&s, "streak-3"));
        assert!(!earned(&s, "streak-6"));
        assert_eq!((s.profile.streak, s.profile.best_streak), (1, 4));
    }

    #[test]
    fn finished_quests_add_xp() {
        let p = profile(Lifetime {
            quest_xp: 150,
            ..Default::default()
        });
        assert_eq!(p.xp, 150);
    }

    #[test]
    fn every_area_always_has_a_quest() {
        let w = monthly_quests("empty", day("2026-11-25"), &[], &[], &[]);
        assert_eq!(ids(&w), ["check-ins"]);
        assert_eq!(
            (w.month.as_str(), w.starts, w.ends),
            ("2026-11", day("2026-11-01"), day("2026-11-30"))
        );
        assert_eq!(
            monthly_quests("x", day("2028-02-10"), &[], &[], &[]).ends,
            day("2028-02-29")
        );
        // Someone who tried everything in a one-country area still gets one.
        let rows = [row("a", "IT", true)];
        let history = [tasted("a", "IT", "2026-01-01", "2026-01-01")];
        let w = monthly_quests("one", day("2026-11-25"), &rows, &history, &[]);
        assert!(ids(&w).contains(&"check-ins"));
        assert!(!ids(&w).contains(&"new-country"));
    }

    #[test]
    fn quests_are_fixed_for_the_month_and_vary_by_area_and_month() {
        let rows = [
            row("a", "IT", true),
            row("b", "IT", false),
            row("c", "JP", false),
            row("d", "MX", false),
        ];
        let before = [tasted("a", "IT", "2026-10-01", "2026-10-01")];
        let start = monthly_quests("leuven", day("2026-11-01"), &rows, &before, &["MX".into()]);
        assert_eq!(start.quests.len(), 3);
        // Checking in during the month changes progress, not which quests run.
        let mut later = before.to_vec();
        later.push(tasted("c", "JP", "2026-11-26", "2026-11-26"));
        let end = monthly_quests("leuven", day("2026-11-30"), &rows, &later, &["MX".into()]);
        assert_eq!(ids(&start), ids(&end));

        let picks: HashSet<String> = ["leuven", "gent", "brussel", "antwerpen"]
            .iter()
            .flat_map(|area| {
                ["2026-11-10", "2026-12-10"].map(|d| {
                    ids(&monthly_quests(
                        area,
                        day(d),
                        &rows,
                        &before,
                        &["MX".into()],
                    ))
                    .join(",")
                })
            })
            .collect();
        assert!(picks.len() > 1, "always the same quests: {picks:?}");
    }

    #[test]
    fn quest_progress() {
        let rows = [
            row("a", "IT", true),
            row("b", "IT", true),
            row("c", "JP", true),
            row("d", "MX", true),
            row("e", "MX", false),
        ];
        let history = [
            tasted("a", "IT", "2026-10-01", "2026-11-24"),
            tasted("b", "IT", "2026-11-24", "2026-11-24"),
            tasted("c", "JP", "2026-11-25", "2026-11-25"),
            tasted("d", "MX", "2026-11-25", "2026-11-25"),
        ];
        // Try every area id until each kind showed up once, so the test doesn't depend on the pick.
        let mut seen = std::collections::HashMap::new();
        for area in (0..40).map(|i| format!("area-{i}")) {
            let w = monthly_quests(
                &area,
                day("2026-11-26"),
                &rows,
                &history,
                &["MX".into(), "FR".into()],
            );
            for q in w.quests {
                seen.entry(q.id.clone()).or_insert(q);
            }
        }
        let q = |id: &str| seen.get(id).unwrap_or_else(|| panic!("no quest {id}"));
        // JP and MX are new this month; IT was known.
        assert_eq!(
            (q("new-country").progress, q("new-country").done),
            (1, true)
        );
        // Europe, Asia and the Americas.
        assert!(q("continents").done);
        // MX celebrates (FR has no places here); one of its two places visited.
        assert_eq!(q("celebration").countries, ["MX"]);
        assert_eq!((q("celebration").progress, q("celebration").target), (1, 2));
        // IT went from 1 of 2 places (gold) to 2 of 2 (diamond).
        assert_eq!(q("level-up").countries, ["IT"]);
        assert!(q("level-up").done);
        assert_eq!((q("check-ins").progress, q("check-ins").done), (2, true));
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
        let s = summarize(
            &rows,
            Lifetime {
                visits: 3,
                countries: 2,
                ..Default::default()
            },
        );

        let status = |code: &str| s.countries.iter().find(|c| c.code == code).unwrap().status;
        assert_eq!(status("IT"), Status::Completed);
        assert_eq!(status("FR"), Status::Discovered);
        assert_eq!(status("JP"), Status::Available);

        let europe = s.continents.iter().find(|c| c.code == "EU").unwrap();
        assert_eq!(
            (europe.available, europe.discovered, europe.completed),
            (2, 2, 1)
        );
        assert_eq!(
            (s.world.available, s.world.discovered, s.world.completed),
            (3, 2, 1)
        );
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
        let s = summarize(
            &rows,
            Lifetime {
                visits: 1,
                countries: 2,
                ..Default::default()
            },
        );
        assert_eq!((s.places, s.places_visited), (1, 1));
        assert_eq!(s.world.discovered, 2);
        assert!(earned(&s, "world") && earned(&s, "world-complete"));
    }

    #[test]
    fn countries_go_gold_platinum_diamond() {
        use Status::*;
        assert_eq!(Status::of(30, 0), Available);
        assert_eq!(Status::of(30, 1), Discovered);
        assert_eq!(Status::of(30, 4), Discovered);
        assert_eq!(Status::of(30, 5), Platinum);
        assert_eq!(Status::of(30, 9), Platinum);
        assert_eq!(Status::of(30, 10), Completed);
        // Small countries reach diamond by visiting every place, skipping platinum.
        assert_eq!(Status::of(3, 2), Discovered);
        assert_eq!(Status::of(3, 3), Completed);
        assert_eq!(Status::of(7, 6), Platinum);
        assert_eq!(Status::of(7, 7), Completed);
    }

    #[test]
    fn verified_check_ins_earn_extra_xp() {
        let p = profile(Lifetime {
            visits: 2,
            verified: 1,
            ..Default::default()
        });
        assert_eq!(p.xp, 10 + 25);
    }

    #[test]
    fn levels_follow_the_quadratic_curve() {
        assert_eq!(profile(Lifetime::default()).level, 1);
        // 1 visit in a new country: 60 XP, still level 1.
        let p = profile(Lifetime {
            visits: 1,
            countries: 1,
            ..Default::default()
        });
        assert_eq!(
            (p.xp, p.level, p.level_xp, p.next_level_xp),
            (60, 1, 0, 100)
        );
        // 5 visits over 5 countries: 300 XP, level 2 (100..400).
        let p = profile(Lifetime {
            visits: 5,
            countries: 5,
            ..Default::default()
        });
        assert_eq!(
            (p.xp, p.level, p.level_xp, p.next_level_xp),
            (300, 2, 100, 400)
        );
        assert_eq!(
            profile(Lifetime {
                visits: 10,
                countries: 6,
                ..Default::default()
            })
            .level,
            3
        );
    }

    #[test]
    fn added_restaurants_earn_xp_with_bonuses_for_new_countries_and_continents() {
        assert_eq!(
            (Bonus::None.xp(), Bonus::Country.xp(), Bonus::Continent.xp()),
            (30, 130, 380)
        );
        // One plain addition, one new country, one new continent (which is a new country too).
        let lifetime = Lifetime {
            added: 3,
            country_bonuses: 2,
            continent_bonuses: 1,
            ..Default::default()
        };
        let p = profile(lifetime);
        assert_eq!((p.xp, p.added), (30 + 130 + 380, 3));
        let s = summarize(&[], lifetime);
        assert!(earned(&s, "pioneer"));
        assert!(!earned(
            &summarize(
                &[],
                Lifetime {
                    added: 1,
                    ..Default::default()
                }
            ),
            "pioneer"
        ));
    }

    #[test]
    fn pick_follows_the_weights() {
        let w = [1.0, 0.0, 3.0];
        assert_eq!(pick(&w, 0.0), 0);
        assert_eq!(pick(&w, 0.24), 0);
        assert_eq!(pick(&w, 0.25), 2);
        assert_eq!(pick(&w, 0.999), 2);
        let tasted = ["JP".to_string()];
        let (pe, jp) = (["PE".to_string()], ["JP".to_string()]);
        let weight = |countries, distance_m| {
            let place = Candidate {
                countries,
                distance_m,
            };
            suggestion_weight(&place, &tasted)
        };
        assert_eq!(weight(&pe, 0), 3.0);
        assert_eq!(weight(&jp, 0), 1.0);
        assert_eq!(weight(&jp, 2000), 0.5);
        let r = random_unit();
        assert!((0.0..1.0).contains(&r));
    }

    #[test]
    fn suggests_a_place_and_lands_on_an_untasted_country() {
        assert_eq!(suggest(&[], &[]), None);
        let tasted = ["JP".to_string()];
        let both = ["JP".to_string(), "PE".to_string()];
        let one = [Candidate {
            countries: &both,
            distance_m: 100,
        }];
        assert_eq!(suggest(&one, &tasted), Some((0, "PE".to_string())));
        let all = ["JP".to_string(), "PE".to_string()];
        assert_eq!(suggest(&one, &all), Some((0, "JP".to_string())));
    }
}
