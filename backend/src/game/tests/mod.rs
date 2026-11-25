use chrono::NaiveDate;

use super::*;

mod months;
mod quests;
mod suggestion;
mod summary;
mod xp;

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
