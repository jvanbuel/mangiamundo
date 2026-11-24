//! A player's progress in an area: what the countries, badges, events and quests look
//! like for them right now. Reads the store and applies the rules in [`game`].

use std::collections::HashMap;

use chrono::NaiveDate;
use serde::Serialize;
use utoipa::ToSchema;

use crate::game::{self, BadgeKind, Lifetime, MonthlyQuests, PlaceRow, Status, Summary};
use crate::{calendar, catalog, store, AppState};

/// Everything the game shows about one area for one player, or for nobody in particular.
#[derive(Serialize, ToSchema)]
pub struct Progress {
    #[serde(flatten)]
    pub summary: Summary,
    pub events: Vec<EventProgress>,
    pub quests: MonthlyQuests,
}

/// The progress of `user` in a cached area on `today`, their local date. Without a user:
/// what the area offers, with nothing visited.
pub async fn load(
    state: &AppState,
    user: Option<&str>,
    area_id: &str,
    today: NaiveDate,
) -> sqlx::Result<Progress> {
    let rows = store::area_rows(&state.db, area_id, user).await?;
    let quests = monthly_quests(state, user, area_id, today, &rows).await?;
    // After the rewards, so finished quests count in the profile.
    let lifetime = match user {
        Some(user) => store::lifetime(&state.db, user, today).await?,
        None => Lifetime::default(),
    };
    let events = event_progress(state, user, &rows, today).await?;
    Ok(Progress {
        summary: game::summarize(&rows, lifetime),
        events,
        quests,
    })
}

#[derive(Serialize, PartialEq, Debug, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventStatus {
    Upcoming,
    Active,
    Ended,
}

/// A time-limited challenge and how far the user got.
#[derive(Serialize, ToSchema)]
pub struct EventProgress {
    #[serde(flatten)]
    pub event: calendar::Event,
    pub status: EventStatus,
    pub progress: u32,
    pub earned: bool,
    /// Places in the area that count towards the event.
    pub places_nearby: u32,
}

/// How many places each country has in an area.
pub fn places_per_country(rows: &[PlaceRow]) -> HashMap<&str, u32> {
    let mut counts = HashMap::new();
    for row in rows {
        *counts.entry(row.country.as_str()).or_default() += 1;
    }
    counts
}

/// Events to show: upcoming within a month, running, or finished and earned.
async fn event_progress(
    state: &AppState,
    user: Option<&str>,
    rows: &[PlaceRow],
    today: NaiveDate,
) -> sqlx::Result<Vec<EventProgress>> {
    let counts = places_per_country(rows);
    let mut out = Vec::new();
    for event in calendar::relevant_events(today, 30) {
        let progress = match user {
            Some(user) => {
                store::visits_between(&state.db, user, &event.countries, event.starts, event.ends)
                    .await?
            }
            None => 0,
        };
        let earned = progress >= event.target;
        let status = if today < event.starts {
            EventStatus::Upcoming
        } else if today <= event.ends {
            EventStatus::Active
        } else {
            EventStatus::Ended
        };
        if status == EventStatus::Ended && !earned {
            continue;
        }
        out.push(EventProgress {
            places_nearby: event
                .countries
                .iter()
                .map(|c| counts.get(c.as_str()).copied().unwrap_or(0))
                .sum(),
            event: event.clone(),
            status,
            progress: progress.min(event.target),
            earned,
        });
    }
    Ok(out)
}

/// The quests of `today`'s month in the area and how far the user is. Only reads, so it
/// can be asked about any month.
pub async fn quests(
    state: &AppState,
    user: Option<&str>,
    area_id: &str,
    today: NaiveDate,
    rows: &[PlaceRow],
) -> sqlx::Result<MonthlyQuests> {
    let tasted = match user {
        Some(user) => store::tasted(&state.db, user).await?,
        None => Vec::new(),
    };
    let first = game::month_start(today);
    let days = (first + chrono::Months::new(1) - first).num_days() - 1;
    let celebrating: Vec<String> = calendar::upcoming(first, days)
        .into_iter()
        .flat_map(|c| c.countries)
        .collect();
    Ok(game::monthly_quests(
        area_id,
        today,
        rows,
        &tasted,
        &celebrating,
    ))
}

/// This month's quests in the area, keeping the XP of the ones the user finished.
pub async fn monthly_quests(
    state: &AppState,
    user: Option<&str>,
    area_id: &str,
    today: NaiveDate,
    rows: &[PlaceRow],
) -> sqlx::Result<MonthlyQuests> {
    let monthly = quests(state, user, area_id, today, rows).await?;
    if let Some(user) = user {
        let done: Vec<(&str, u32)> = monthly
            .quests
            .iter()
            .filter(|q| q.done)
            .map(|q| (q.id.as_str(), q.xp))
            .collect();
        store::reward_quests(&state.db, user, &monthly.month, &done).await?;
    }
    Ok(monthly)
}

/// What a check-in unlocked, announced to the player.
#[derive(Debug, Serialize, PartialEq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnlockedKind {
    /// A country reached gold, platinum or diamond.
    Country,
    Badge,
    /// The badge of a time-limited event.
    Event,
    Level,
}

/// One thing a check-in unlocked, with the words to announce it.
#[derive(Debug, Serialize, PartialEq, ToSchema)]
pub struct Unlocked {
    pub kind: UnlockedKind,
    /// For `country`: its code, to show its flag.
    #[schema(required = true)]
    pub country: Option<String>,
    /// For `badge`: which medal to show.
    #[schema(required = true)]
    pub badge: Option<BadgeKind>,
    /// For `level`: the level reached.
    #[schema(required = true)]
    pub level: Option<u32>,
    pub title: String,
    pub text: String,
}

impl Unlocked {
    fn new(kind: UnlockedKind, title: String, text: String) -> Self {
        Unlocked {
            kind,
            country: None,
            badge: None,
            level: None,
            title,
            text,
        }
    }
}

/// Everything that is in `after` and wasn't in `before`: countries that reached a new
/// status, badges, event badges and a new level, in that order. `area` is the area's name.
pub fn unlocked(before: &Progress, after: &Progress, area: &str) -> Vec<Unlocked> {
    let mut out = Vec::new();
    for now in &after.summary.countries {
        let was = before.summary.countries.iter().find(|c| c.code == now.code);
        let was = was.map_or(Status::Available, |c| c.status);
        if now.status <= was {
            continue;
        }
        let Some(country) = catalog::country(&now.code) else {
            continue;
        };
        let more = format!(
            "{} more for diamond.",
            now.goal.saturating_sub(now.visited)
        );
        let (medal, text) = match now.status {
            Status::Available => continue,
            Status::Discovered => ("gold", more),
            Status::Platinum => ("platinum", more),
            Status::Completed => (
                "diamond",
                format!("You visited {} of its places in {area}.", now.visited),
            ),
        };
        out.push(Unlocked {
            country: Some(now.code.clone()),
            ..Unlocked::new(
                UnlockedKind::Country,
                format!("{}: {medal}", country.name),
                text,
            )
        });
    }
    for badge in after.summary.badges.iter().filter(|b| b.earned) {
        let had = before.summary.badges.iter();
        if !had.filter(|b| b.earned).any(|b| b.id == badge.id) {
            out.push(Unlocked {
                badge: Some(badge.kind),
                ..Unlocked::new(
                    UnlockedKind::Badge,
                    format!("Badge earned: {}", badge.title),
                    badge.description.clone(),
                )
            });
        }
    }
    for event in after.events.iter().filter(|e| e.earned) {
        let had = before.events.iter();
        if !had.filter(|e| e.earned).any(|e| e.event.id == event.event.id) {
            out.push(Unlocked::new(
                UnlockedKind::Event,
                format!("Badge earned: {}", event.event.title),
                "Earned while it lasted.".into(),
            ));
        }
    }
    let profile = &after.summary.profile;
    if profile.level > before.summary.profile.level {
        out.push(Unlocked {
            level: Some(profile.level),
            ..Unlocked::new(
                UnlockedKind::Level,
                format!("Level {}", profile.level),
                format!("{} XP so far.", profile.xp),
            )
        });
    }
    out
}
