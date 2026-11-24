//! Web Push: celebration and event reminders for browsers.
//!
//! Every few minutes the server looks at each subscription in the subscriber's own
//! time zone. From 10:00 on the day a celebration or event starts it sends one
//! notification, but only when the subscriber's area has places of that country. It also
//! says when food from a country the subscriber hasn't eaten yet appears in their area,
//! and on the first of the month what that month's quests are.

use std::collections::{HashMap, HashSet};
use std::time::Duration as StdDuration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{Datelike, Duration, NaiveDate, Timelike};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use web_push::{
    request_builder, ContentEncoding, SubscriptionInfo, VapidSignatureBuilder,
    WebPushMessageBuilder,
};

use crate::game::PlaceRow;
use crate::{areas, calendar, catalog, progress, store, AppState};

/// Reminders are sent from this local hour until the end of the evening.
const FIRST_HOUR: u32 = 10;
const LAST_HOUR: u32 = 20;
/// "Last days" reminders for events go out this many days before the end.
const LAST_CHANCE_DAYS: i64 = 3;

#[derive(Clone)]
pub struct Pusher {
    http: reqwest::Client,
    /// Raw P-256 private key, base64url.
    private_key: String,
    /// Public key for browsers (`applicationServerKey`), base64url.
    pub public_key: String,
    /// Contact for push services: a `mailto:` or `https:` URL.
    subject: String,
}

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

#[derive(Debug, PartialEq)]
pub enum Delivery {
    Sent,
    /// The browser unsubscribed; the subscription should be forgotten.
    Gone,
}

impl Pusher {
    /// Uses the key in `VAPID_PRIVATE_KEY`, or one stored in the database, creating it on first run.
    pub async fn load(
        db: &SqlitePool,
        configured: Option<String>,
        subject: String,
    ) -> anyhow::Result<Self> {
        let private_key = match configured {
            Some(key) => key,
            None => match store::setting(db, "vapid_private_key").await? {
                Some(key) => key,
                None => {
                    let key = new_private_key()?;
                    store::set_setting(db, "vapid_private_key", &key).await?;
                    key
                }
            },
        };
        let public_key = URL_SAFE_NO_PAD.encode(
            VapidSignatureBuilder::from_base64_no_sub(&private_key)
                .map_err(|e| anyhow::anyhow!("invalid VAPID key: {e}"))?
                .get_public_key(),
        );
        let http = reqwest::Client::builder()
            .timeout(StdDuration::from_secs(20))
            .build()?;
        Ok(Self {
            http,
            private_key,
            public_key,
            subject,
        })
    }

    pub async fn send(
        &self,
        to: &Subscription,
        notification: &Notification,
    ) -> anyhow::Result<Delivery> {
        let info =
            SubscriptionInfo::new(to.endpoint.as_str(), to.p256dh.as_str(), to.auth.as_str());
        let mut signature = VapidSignatureBuilder::from_base64(&self.private_key, &info)?;
        signature.add_claim("sub", self.subject.as_str());
        let payload = serde_json::to_vec(notification)?;
        let mut message = WebPushMessageBuilder::new(&info);
        message.set_payload(ContentEncoding::Aes128Gcm, &payload);
        message.set_vapid_signature(signature.build()?);
        message.set_ttl(12 * 3600);
        let request: http::Request<Vec<u8>> = request_builder::build_request(message.build()?);

        // web-push speaks an older `http` crate than reqwest; copy the request across.
        let mut outgoing = self.http.post(request.uri().to_string());
        for (name, value) in request.headers() {
            outgoing = outgoing.header(name.as_str(), value.as_bytes());
        }
        let response = outgoing.body(request.into_body()).send().await?;
        match response.status().as_u16() {
            200..=299 => Ok(Delivery::Sent),
            404 | 410 => Ok(Delivery::Gone),
            status => anyhow::bail!(
                "push service answered {status}: {}",
                response.text().await.unwrap_or_default()
            ),
        }
    }
}

fn new_private_key() -> anyhow::Result<String> {
    // Any 32 random bytes are a valid P-256 key except with negligible probability;
    // the builder rejects the rare invalid one, so try again in that case.
    loop {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("no randomness: {e}"))?;
        let key = URL_SAFE_NO_PAD.encode(bytes);
        if VapidSignatureBuilder::from_base64_no_sub(&key).is_ok() {
            return Ok(key);
        }
    }
}

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

fn places(n: u32) -> String {
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
pub fn celebration_notification(
    celebration: &calendar::Celebration,
    counts: &HashMap<&str, u32>,
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
    let total = serving.len() as u32;
    let them = if countries.len() == 1 { "it" } else { "them" };
    Some(Notification {
        title: format!("{} food is now near you", cuisine_names(countries, "and")),
        body: format!("{} in {area}. You haven't tried {them} yet.", places(total)),
        url: format!("/?country={first}"),
        tag: "new-food".into(),
    })
}

/// The reminder on an event's first day, when `nearby` places serve its countries' food.
fn event_start_notification(event: &calendar::Event, nearby: u32) -> Notification {
    Notification {
        title: format!("{} starts today", event.title),
        body: format!("{} {} nearby.", event.description, there_are(nearby)),
        url: "/?tab=badges".into(),
        tag: format!("event-{}", event.id),
    }
}

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
    for local_date in today.iter_days().take(days.max(0) as usize + 1) {
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
        let mut present: Vec<String> = counts.keys().map(|c| c.to_string()).collect();
        present.sort();
        // Read before marking, so a failure here leaves the countries for the next pass.
        let visited = store::visited_countries(&state.db, user).await?;
        let new = store::take_new_countries(&state.db, &sub.endpoint, area_id, &present).await?;
        let untried: Vec<&str> = new
            .iter()
            .filter(|c| !visited.contains(c))
            .map(String::as_str)
            .collect();
        if let Some(notification) =
            new_food_notification(&untried, &rows, &area.boundary.info.name)
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

#[cfg(test)]
mod tests {
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
