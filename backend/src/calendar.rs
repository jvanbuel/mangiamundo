//! Country celebrations (national days, food festivals) and time-limited challenges.

use std::collections::HashMap;
use std::sync::OnceLock;

use chrono::{DateTime, Datelike, Duration, NaiveDate};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

use crate::catalog;

#[derive(Debug, Deserialize)]
struct RawCelebration {
    id: String,
    title: String,
    description: String,
    countries: Vec<String>,
    /// Same day every year, as "MM-DD".
    date: Option<String>,
    /// For dates that move: the first day per year.
    #[serde(default)]
    dates: HashMap<String, NaiveDate>,
    #[serde(default = "one")]
    days: u32,
    #[serde(default)]
    approximate: bool,
}

fn one() -> u32 {
    1
}

/// One occurrence of a celebration.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Celebration {
    pub id: String,
    pub title: String,
    pub description: String,
    pub countries: Vec<String>,
    pub starts: NaiveDate,
    /// Last day, inclusive.
    pub ends: NaiveDate,
    /// True for dates set by moon sighting or astronomy, which can shift by a day.
    pub approximate: bool,
}

fn celebrations() -> &'static [RawCelebration] {
    static DATA: OnceLock<Vec<RawCelebration>> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../data/celebrations.json"))
            .expect("data/celebrations.json is valid")
    })
}

impl RawCelebration {
    fn start_in(&self, year: i32) -> Option<NaiveDate> {
        match &self.date {
            Some(md) => {
                let (month, day) = md.split_once('-')?;
                NaiveDate::from_ymd_opt(year, month.parse().ok()?, day.parse().ok()?)
            }
            None => self.dates.get(&year.to_string()).copied(),
        }
    }

    fn occurrence(&self, year: i32) -> Option<Celebration> {
        let starts = self.start_in(year)?;
        Some(Celebration {
            id: self.id.clone(),
            title: self.title.clone(),
            description: self.description.clone(),
            countries: self.countries.clone(),
            starts,
            ends: starts + Duration::days(i64::from(self.days.max(1)) - 1),
            approximate: self.approximate,
        })
    }
}

/// Celebrations that are on, or start within `days` days of `today`, soonest first.
pub fn upcoming(today: NaiveDate, days: i64) -> Vec<Celebration> {
    let horizon = today + Duration::days(days);
    let mut found: Vec<Celebration> = celebrations()
        .iter()
        .flat_map(|c| [today.year() - 1, today.year(), today.year() + 1].map(|y| c.occurrence(y)))
        .flatten()
        .filter(|c| c.ends >= today && c.starts <= horizon)
        .collect();
    found.sort_by(|a, b| a.starts.cmp(&b.starts).then_with(|| a.title.cmp(&b.title)));
    found
}

/// Celebrations whose first day is `day`.
pub fn starting_on(day: NaiveDate) -> Vec<Celebration> {
    upcoming(day, 0).into_iter().filter(|c| c.starts == day).collect()
}

/// A time-limited challenge, like "Japan month". Earned badges are kept for good.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Event {
    pub id: String,
    pub title: String,
    pub description: String,
    pub countries: Vec<String>,
    pub starts: NaiveDate,
    /// Last day, inclusive.
    pub ends: NaiveDate,
    /// Number of different places to visit during the event.
    pub target: u32,
}

pub fn events() -> &'static [Event] {
    static DATA: OnceLock<Vec<Event>> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../data/events.json")).expect("data/events.json is valid")
    })
}

/// Events to show today: running, starting within `days` days, or already over (the
/// caller decides whether a finished one is worth showing).
pub fn relevant_events(today: NaiveDate, days: i64) -> impl Iterator<Item = &'static Event> {
    let horizon = today + Duration::days(days);
    events().iter().filter(move |e| e.starts <= horizon)
}

pub fn events_starting_on(day: NaiveDate) -> impl Iterator<Item = &'static Event> {
    events().iter().filter(move |e| e.starts == day)
}

/// The date at Unix time `now` in `zone`. Celebrations and events are calendar days, so
/// "today" is the user's day, not the server's.
pub fn local_date(now: i64, zone: Tz) -> NaiveDate {
    DateTime::from_timestamp(now, 0).unwrap_or_default().with_timezone(&zone).date_naive()
}

/// Checks the data files for unknown countries and impossible dates.
pub fn validate() -> Result<(), String> {
    for c in celebrations() {
        if c.date.is_none() && c.dates.is_empty() {
            return Err(format!("{} has no date", c.id));
        }
        if c.date.is_some() && c.start_in(2028).is_none() {
            return Err(format!("{} has an invalid date", c.id));
        }
        for code in &c.countries {
            catalog::country(code).ok_or_else(|| format!("{}: unknown country {code}", c.id))?;
        }
    }
    for e in events() {
        if e.ends < e.starts || e.target == 0 {
            return Err(format!("{} has invalid dates or target", e.id));
        }
        for code in &e.countries {
            catalog::country(code).ok_or_else(|| format!("{}: unknown country {code}", e.id))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn data_files_are_valid() {
        validate().unwrap();
    }

    #[test]
    fn finds_fixed_and_moving_dates() {
        let soon = upcoming(day("2026-11-16"), 14);
        let ids: Vec<_> = soon.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["lebanese-independence", "thanksgiving"]);
        assert_eq!(soon[1].starts, day("2026-11-26"));
    }

    #[test]
    fn multi_day_celebrations_count_while_running() {
        // Oktoberfest 2026 runs 19 September to 4 October.
        let during = upcoming(day("2026-10-02"), 0);
        let fest = during.iter().find(|c| c.id == "oktoberfest").expect("still on");
        assert_eq!(fest.ends, day("2026-10-04"));
        assert!(starting_on(day("2026-10-02")).iter().all(|c| c.id != "oktoberfest"));
    }

    #[test]
    fn looks_into_next_year() {
        let new_year = upcoming(day("2026-12-20"), 60);
        assert!(new_year.iter().any(|c| c.id == "burns-night" && c.starts == day("2027-01-25")));
        assert!(starting_on(day("2027-02-06")).iter().any(|c| c.id == "lunar-new-year"));
    }

    #[test]
    fn today_is_the_local_date() {
        let tokyo: Tz = "Asia/Tokyo".parse().unwrap();
        // 1 December 2026 00:00 in Tokyo is 30 November 15:00 UTC.
        assert_eq!(local_date(1_796_050_800, tokyo), day("2026-12-01"));
        assert_eq!(local_date(1_796_050_799, tokyo), day("2026-11-30"));
        assert_eq!(local_date(1_796_050_800, Tz::UTC), day("2026-11-30"));
    }

    #[test]
    fn years_without_a_listed_date_are_skipped() {
        assert!(upcoming(day("2031-01-01"), 400).iter().all(|c| c.id != "diwali"));
    }
}
