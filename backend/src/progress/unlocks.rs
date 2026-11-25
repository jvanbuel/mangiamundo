//! What a check-in unlocked: the difference between a player's progress before and after.

use serde::Serialize;
use utoipa::ToSchema;

use super::Progress;
use crate::catalog;
use crate::game::{BadgeKind, Status};

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
        let more = format!("{} more for diamond.", now.goal.saturating_sub(now.visited));
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
        if !had
            .filter(|e| e.earned)
            .any(|e| e.event.id == event.event.id)
        {
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
