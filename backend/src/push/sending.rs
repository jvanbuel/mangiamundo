//! The loop that sends each subscriber what is due.

use std::time::Duration as StdDuration;

use chrono::Timelike;

use super::{
    due, new_food_notification, Delivery, Notification, Pusher, Subscription, FIRST_HOUR, LAST_HOUR,
};
use crate::{areas, progress, store, AppState};

/// Sends one notification. None when it failed and the next pass should try again.
async fn deliver(
    pusher: &Pusher,
    to: &Subscription,
    notification: &Notification,
) -> Option<Delivery> {
    match pusher.send(to, notification).await {
        Ok(delivery) => Some(delivery),
        Err(error) => {
            tracing::warn!(error = format!("{error:#}"), "push failed");
            None
        }
    }
}

/// One pass over all subscriptions. Returns how many notifications were sent. A subscriber
/// whose reminders fail is logged and tried again on the next pass; the others still get theirs.
pub async fn run_once(state: &AppState, pusher: &Pusher) -> anyhow::Result<usize> {
    let mut sent = 0;
    for (user, sub) in store::push_subscriptions(&state.db).await? {
        match remind(state, pusher, &user, &sub).await {
            Ok(n) => sent += n,
            Err(error) => tracing::warn!(
                error = format!("{error:#}"),
                user,
                "reminders failed for a subscriber"
            ),
        }
    }
    Ok(sent)
}

/// Sends one subscriber what is due now. Returns how many notifications were sent.
async fn remind(
    state: &AppState,
    pusher: &Pusher,
    user: &str,
    sub: &Subscription,
) -> anyhow::Result<usize> {
    let now = chrono::DateTime::from_timestamp(state.now(), 0).unwrap_or_default();
    let zone: chrono_tz::Tz = sub.timezone.parse().unwrap_or(chrono_tz::UTC);
    let local = now.with_timezone(&zone);
    if !(FIRST_HOUR..=LAST_HOUR).contains(&local.hour()) {
        return Ok(0);
    }
    let Some(area_id) = &sub.area_id else {
        return Ok(0);
    };
    // Only areas already in the cache: never wait on a provider from here.
    let Some(area) = areas::cached(state, area_id).await? else {
        return Ok(0);
    };
    let rows = store::area_rows(&state.db, area_id, None).await?;
    let counts = progress::places_per_country(&rows);

    let day = local.date_naive();
    let mut sent = 0;
    let mut gone = false;
    for (dedupe, notification) in due(state, Some(user), day, area_id, &rows, &counts).await? {
        if !store::mark_push_sent(&state.db, &sub.endpoint, &dedupe, state.now()).await? {
            continue;
        }
        match deliver(pusher, sub, &notification).await {
            Some(Delivery::Sent) => sent += 1,
            Some(Delivery::Gone) => {
                gone = true;
                break;
            }
            None => store::unmark_push_sent(&state.db, &sub.endpoint, &dedupe).await?,
        }
    }

    if !gone {
        let mut present: Vec<String> = counts
            .keys()
            .map(std::string::ToString::to_string)
            .collect();
        present.sort();
        // Read before marking, so a failure here leaves the countries for the next pass.
        let visited = store::visited_countries(&state.db, user).await?;
        let new = store::take_new_countries(&state.db, &sub.endpoint, area_id, &present).await?;
        let untried: Vec<&str> = new
            .iter()
            .filter(|c| !visited.contains(c))
            .map(String::as_str)
            .collect();
        if let Some(notification) = new_food_notification(&untried, &rows, &area.boundary.info.name)
        {
            match deliver(pusher, sub, &notification).await {
                Some(Delivery::Sent) => sent += 1,
                Some(Delivery::Gone) => gone = true,
                None => store::forget_countries(&state.db, &sub.endpoint, &new).await?,
            }
        }
    }
    if gone {
        store::remove_push_subscription(&state.db, &sub.endpoint).await?;
    }
    Ok(sent)
}

/// Runs `run_once` forever, every `every`.
pub async fn run(state: AppState, pusher: Pusher, every: StdDuration) {
    let mut ticker = tokio::time::interval(every);
    loop {
        ticker.tick().await;
        match run_once(&state, &pusher).await {
            Ok(0) => {}
            Ok(n) => tracing::info!(sent = n, "push reminders sent"),
            Err(error) => tracing::error!(error = format!("{error:#}"), "push pass failed"),
        }
    }
}
