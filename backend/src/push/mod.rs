//! Web Push: celebration and event reminders for browsers.
//!
//! Every few minutes the server looks at each subscription in the subscriber's own
//! time zone. From 10:00 on the day a celebration or event starts it sends one
//! notification, but only when the subscriber's area has places of that country. It also
//! says when food from a country the subscriber hasn't eaten yet appears in their area,
//! and on the first of the month what that month's quests are.

use std::collections::HashMap;

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::game::PlaceRow;
use crate::{calendar, progress, store, AppState};

mod messages;
mod pusher;
mod sending;

pub use messages::{celebration_notification, new_food_notification, quests_notification};
pub use pusher::{Delivery, Pusher};
pub use sending::{run, run_once};

use messages::{event_start_notification, places};

/// Reminders are sent from this local hour until the end of the evening.
const FIRST_HOUR: u32 = 10;
const LAST_HOUR: u32 = 20;
/// "Last days" reminders for events go out this many days before the end.
const LAST_CHANCE_DAYS: i64 = 3;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Subscription {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    /// The area reminders are about; `None` for subscriptions from before areas, until
    /// the app subscribes again.
    pub area_id: Option<String>,
    pub timezone: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct Notification {
    pub title: String,
    pub body: String,
    /// Where tapping the notification leads, relative to the app.
    pub url: String,
    /// Collapses repeated notifications of the same thing.
    pub tag: String,
}

/// The reminders due on `day` in an area, keyed for de-duplication. Only reads, so it can
/// be asked about any day. Without a user: as for someone who has no visits.
async fn due(
    state: &AppState,
    user: Option<&str>,
    day: NaiveDate,
    area_id: &str,
    rows: &[PlaceRow],
    counts: &HashMap<&str, u32>,
) -> sqlx::Result<Vec<(String, Notification)>> {
    let mut out = Vec::new();
    if day.day() == 1 {
        let monthly = progress::quests(state, user, area_id, day, rows).await?;
        out.push((
            format!("quests:{}", monthly.month),
            quests_notification(&monthly),
        ));
    }
    for celebration in calendar::starting_on(day) {
        if let Some(notification) = celebration_notification(&celebration, counts) {
            out.push((
                format!("celebration:{}:{day}", celebration.id),
                notification,
            ));
        }
    }

    for event in calendar::events() {
        let nearby: u32 = event
            .countries
            .iter()
            .map(|c| counts.get(c.as_str()).copied().unwrap_or(0))
            .sum();
        if nearby == 0 {
            continue;
        }
        if event.starts == day {
            out.push((
                format!("event-start:{}", event.id),
                event_start_notification(event, nearby),
            ));
        } else if event.ends - Duration::days(LAST_CHANCE_DAYS - 1) == day {
            let done = match user {
                Some(user) => {
                    let (from, to) = (event.starts, event.ends);
                    store::visits_between(&state.db, user, &event.countries, from, to).await?
                }
                None => 0,
            };
            if done < event.target {
                let left = event.target - done;
                out.push((
                    format!("event-last:{}", event.id),
                    Notification {
                        title: format!("Last days of {}", event.title),
                        body: format!("{} more to earn the badge before it ends.", places(left)),
                        url: "/?tab=badges".into(),
                        tag: format!("event-{}", event.id),
                    },
                ));
            }
        }
    }
    Ok(out)
}

/// A reminder that will be sent, for apps that schedule their own notifications.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct Reminder {
    /// Stable for this reminder: scheduling it again under the same key replaces it.
    pub key: String,
    /// The day it is due, in the user's time zone.
    pub local_date: NaiveDate,
    /// The local hour from which it is sent that day.
    pub hour: u32,
    #[serde(flatten)]
    pub notification: Notification,
}

/// The reminders that become due in an area from `today` up to `days` days later, soonest
/// first: what the push loop would send on each of those days, given what the user has
/// visited so far. Sends and records nothing.
pub async fn plan(
    state: &AppState,
    user: Option<&str>,
    area_id: &str,
    today: NaiveDate,
    days: i64,
) -> sqlx::Result<Vec<Reminder>> {
    let rows = store::area_rows(&state.db, area_id, None).await?;
    let counts = progress::places_per_country(&rows);
    let mut out = Vec::new();
    for local_date in today
        .iter_days()
        .take(usize::try_from(days).unwrap_or(0) + 1)
    {
        for (key, notification) in due(state, user, local_date, area_id, &rows, &counts).await? {
            out.push(Reminder {
                key,
                local_date,
                hour: FIRST_HOUR,
                notification,
            });
        }
    }
    Ok(out)
}

/// Reads a subscription row.
pub(crate) fn subscription_from_row(row: &sqlx::sqlite::SqliteRow) -> (String, Subscription) {
    (
        row.get("user_id"),
        Subscription {
            endpoint: row.get("endpoint"),
            p256dh: row.get("p256dh"),
            auth: row.get("auth"),
            area_id: row.get("area_id"),
            timezone: row.get("timezone"),
        },
    )
}
