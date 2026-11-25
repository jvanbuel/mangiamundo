//! The words of each notification.

use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;

use super::Notification;
use crate::game::PlaceRow;
use crate::{calendar, catalog};

/// "Lebanese", "Chinese or Vietnamese", "Chinese, Korean or Vietnamese", with `last_word`
/// before the last name.
fn cuisine_names(codes: &[&str], last_word: &str) -> String {
    let names: Vec<String> = codes
        .iter()
        .filter_map(|code| {
            catalog::cuisine_adjective(code).or_else(|| Some(catalog::country(code)?.name.clone()))
        })
        .collect();
    match names.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} {last_word} {last}", rest.join(", ")),
    }
}

pub(super) fn places(n: u32) -> String {
    if n == 1 {
        "1 place".into()
    } else {
        format!("{n} places")
    }
}

/// "There is 1 place", "There are 3 places".
fn there_are(n: u32) -> String {
    let verb = if n == 1 { "is" } else { "are" };
    format!("There {verb} {}", places(n))
}

/// "the one place", "one of 3 places".
fn one_of(n: u32) -> String {
    if n == 1 {
        "the one place".into()
    } else {
        format!("one of {}", places(n))
    }
}

/// The reminder for a celebration, or None when none of its countries has places nearby.
pub fn celebration_notification<S: BuildHasher>(
    celebration: &calendar::Celebration,
    counts: &HashMap<&str, u32, S>,
) -> Option<Notification> {
    let nearby: Vec<&str> = celebration
        .countries
        .iter()
        .map(String::as_str)
        .filter(|c| counts.get(c).copied().unwrap_or(0) > 0)
        .collect();
    let first = *nearby.first()?;
    let total: u32 = nearby.iter().map(|c| counts[*c]).sum();
    let when = if celebration.ends > celebration.starts {
        "starts today"
    } else {
        "is today"
    };
    Some(Notification {
        title: format!("{} {when}", celebration.title),
        body: format!(
            "{} Celebrate at {} serving {} food near you.",
            celebration.description,
            one_of(total),
            cuisine_names(&nearby, "or")
        ),
        url: format!("/?country={first}"),
        tag: format!("celebration-{}", celebration.id),
    })
}

/// The alert for food from `countries` newly in the area, or None when there are none.
///
/// `rows` are the area's places; one serving several of the countries counts once.
pub fn new_food_notification(
    countries: &[&str],
    rows: &[PlaceRow],
    area: &str,
) -> Option<Notification> {
    let first = *countries.first()?;
    let serving: HashSet<&str> = rows
        .iter()
        .filter(|r| countries.contains(&r.country.as_str()))
        .map(|r| r.place_id.as_str())
        .collect();
    let total = crate::count(serving.len());
    let them = if countries.len() == 1 { "it" } else { "them" };
    Some(Notification {
        title: format!("{} food is now near you", cuisine_names(countries, "and")),
        body: format!("{} in {area}. You haven't tried {them} yet.", places(total)),
        url: format!("/?country={first}"),
        tag: "new-food".into(),
    })
}

/// The reminder on an event's first day, when `nearby` places serve its countries' food.
pub(super) fn event_start_notification(event: &calendar::Event, nearby: u32) -> Notification {
    Notification {
        title: format!("{} starts today", event.title),
        body: format!("{} {} nearby.", event.description, there_are(nearby)),
        url: "/?tab=badges".into(),
        tag: format!("event-{}", event.id),
    }
}

/// The reminder of the month's quests, on its first day.
pub fn quests_notification(monthly: &crate::game::MonthlyQuests) -> Notification {
    let titles: Vec<&str> = monthly.quests.iter().map(|q| q.title.as_str()).collect();
    Notification {
        title: "New quests this month".into(),
        body: format!("{}.", titles.join(", ")),
        url: "/?tab=badges".into(),
        tag: "quests".into(),
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, NaiveDate};

    use super::*;

    #[test]
    fn names_cuisines_naturally() {
        assert_eq!(cuisine_names(&["LB"], "or"), "Lebanese");
        assert_eq!(cuisine_names(&["CN", "VN"], "or"), "Chinese or Vietnamese");
        assert_eq!(
            cuisine_names(&["CN", "KR", "VN"], "or"),
            "Chinese, Korean or Vietnamese"
        );
    }

    #[test]
    fn one_place_is_singular() {
        let day = NaiveDate::from_ymd_opt(2026, 12, 1).unwrap();
        let event = calendar::Event {
            id: "japan".into(),
            title: "Japan month".into(),
            description: "Eat at 3 Japanese places.".into(),
            countries: vec!["JP".into()],
            starts: day,
            ends: day + Duration::days(30),
            target: 3,
        };
        let body = |nearby| event_start_notification(&event, nearby).body;
        assert_eq!(
            body(1),
            "Eat at 3 Japanese places. There is 1 place nearby."
        );
        assert_eq!(
            body(4),
            "Eat at 3 Japanese places. There are 4 places nearby."
        );

        let celebration = calendar::Celebration {
            id: "culture-day".into(),
            title: "Culture Day".into(),
            description: "A day for the arts.".into(),
            countries: vec!["JP".into()],
            starts: day,
            ends: day,
            approximate: false,
        };
        let body = |places| {
            let counts = HashMap::from([("JP", places)]);
            celebration_notification(&celebration, &counts)
                .unwrap()
                .body
        };
        assert_eq!(
            body(1),
            "A day for the arts. Celebrate at the one place serving Japanese food near you."
        );
        assert_eq!(
            body(2),
            "A day for the arts. Celebrate at one of 2 places serving Japanese food near you."
        );
    }

    #[test]
    fn groups_new_food_in_one_alert() {
        let row = |place: &str, country: &str| PlaceRow {
            place_id: place.into(),
            country: country.into(),
            visited: false,
        };
        // "b" serves both and counts once.
        let rows = [
            row("a", "ET"),
            row("b", "PE"),
            row("b", "ET"),
            row("c", "PE"),
            row("d", "JP"),
        ];
        let one = new_food_notification(&["JP"], &rows, "Leuven").unwrap();
        assert_eq!(one.title, "Japanese food is now near you");
        assert_eq!(one.body, "1 place in Leuven. You haven't tried it yet.");
        let two = new_food_notification(&["ET", "PE"], &rows, "Leuven").unwrap();
        assert_eq!(two.title, "Ethiopian and Peruvian food is now near you");
        assert_eq!(two.body, "3 places in Leuven. You haven't tried them yet.");
        assert_eq!(two.url, "/?country=ET");
        assert!(new_food_notification(&[], &rows, "Leuven").is_none());
    }
}
