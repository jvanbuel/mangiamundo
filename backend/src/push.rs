//! Web Push: celebration and event reminders for browsers.
//!
//! Every few minutes the server looks at each subscription in the subscriber's own
//! time zone. From 10:00 on the day a celebration or event starts it sends one
//! notification, but only when the subscriber's area has places of that country.

use std::collections::HashMap;
use std::time::Duration as StdDuration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{Duration, NaiveDate, Timelike};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use web_push::{
    request_builder, ContentEncoding, SubscriptionInfo, VapidSignatureBuilder,
    WebPushMessageBuilder,
};

use crate::{calendar, catalog, store, AppState};

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
    pub lat: f64,
    pub lng: f64,
    pub radius_km: f64,
    pub timezone: String,
}

#[derive(Debug, Serialize)]
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
    pub async fn load(db: &SqlitePool, configured: Option<String>, subject: String) -> anyhow::Result<Self> {
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
        let http = reqwest::Client::builder().timeout(StdDuration::from_secs(20)).build()?;
        Ok(Self { http, private_key, public_key, subject })
    }

    pub async fn send(&self, to: &Subscription, notification: &Notification) -> anyhow::Result<Delivery> {
        let info = SubscriptionInfo::new(to.endpoint.as_str(), to.p256dh.as_str(), to.auth.as_str());
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
            status => anyhow::bail!("push service answered {status}: {}", response.text().await.unwrap_or_default()),
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

/// "Lebanese", "Chinese or Vietnamese", "Chinese, Korean or Vietnamese".
fn cuisine_names(codes: &[&str]) -> String {
    let names: Vec<String> = codes
        .iter()
        .filter_map(|code| catalog::cuisine_adjective(code).or_else(|| Some(catalog::country(code)?.name.clone())))
        .collect();
    match names.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
    }
}

fn places(n: u32) -> String {
    if n == 1 { "1 place".into() } else { format!("{n} places") }
}

/// The reminders due for one subscriber on `day`, keyed for de-duplication.
async fn due(
    state: &AppState,
    user: &str,
    day: NaiveDate,
    counts: &HashMap<String, u32>,
) -> anyhow::Result<Vec<(String, Notification)>> {
    let mut out = Vec::new();
    for celebration in calendar::starting_on(day) {
        let nearby: Vec<&str> = celebration
            .countries
            .iter()
            .map(String::as_str)
            .filter(|c| counts.get(*c).copied().unwrap_or(0) > 0)
            .collect();
        if nearby.is_empty() {
            continue;
        }
        let total: u32 = nearby.iter().map(|c| counts[*c]).sum();
        let when = if celebration.ends > celebration.starts { "starts today" } else { "is today" };
        out.push((
            format!("celebration:{}:{day}", celebration.id),
            Notification {
                title: format!("{} {when}", celebration.title),
                body: format!(
                    "{} Celebrate at one of {} serving {} food near you.",
                    celebration.description,
                    places(total),
                    cuisine_names(&nearby)
                ),
                url: format!("/?country={}", nearby[0]),
                tag: format!("celebration-{}", celebration.id),
            },
        ));
    }

    for event in calendar::events() {
        let nearby: u32 = event.countries.iter().map(|c| counts.get(c).copied().unwrap_or(0)).sum();
        if nearby == 0 {
            continue;
        }
        if event.starts == day {
            out.push((
                format!("event-start:{}", event.id),
                Notification {
                    title: format!("{} starts today", event.title),
                    body: format!("{} There are {} nearby.", event.description, places(nearby)),
                    url: "/?tab=badges".into(),
                    tag: format!("event-{}", event.id),
                },
            ));
        } else if event.ends - Duration::days(LAST_CHANCE_DAYS - 1) == day {
            let from = day_start(event.starts);
            let until = day_start(event.ends + Duration::days(1));
            let done = store::visits_between(&state.db, user, &event.countries, from, until).await?;
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

fn day_start(day: NaiveDate) -> i64 {
    day.and_hms_opt(0, 0, 0).map(|t| t.and_utc().timestamp()).unwrap_or(0)
}

/// One pass over all subscriptions. Returns how many notifications were sent.
pub async fn run_once(state: &AppState, pusher: &Pusher) -> anyhow::Result<usize> {
    let now = chrono::DateTime::from_timestamp(state.now(), 0).unwrap_or_default();
    let mut sent = 0;
    for (user, sub) in store::push_subscriptions(&state.db).await? {
        let zone: chrono_tz::Tz = sub.timezone.parse().unwrap_or(chrono_tz::UTC);
        let local = now.with_timezone(&zone);
        if !(FIRST_HOUR..=LAST_HOUR).contains(&local.hour()) {
            continue;
        }
        // Only look at areas already in the cache: never call a provider from here.
        let (lat, lng) = store::snap(sub.lat, sub.lng);
        let radius_m = (sub.radius_km * 1000.0).round() as u32;
        let key = store::area_key(state.places.name(), lat, lng, radius_m);
        if store::find_area(&state.db, &key).await?.is_none() {
            continue;
        }
        let mut counts: HashMap<String, u32> = HashMap::new();
        for row in store::area_rows(&state.db, &key, None).await? {
            *counts.entry(row.country).or_default() += 1;
        }

        for (dedupe, notification) in due(state, &user, local.date_naive(), &counts).await? {
            if !store::mark_push_sent(&state.db, &sub.endpoint, &dedupe, state.now()).await? {
                continue;
            }
            match pusher.send(&sub, &notification).await {
                Ok(Delivery::Sent) => sent += 1,
                Ok(Delivery::Gone) => {
                    store::remove_push_subscription(&state.db, &sub.endpoint).await?;
                    break;
                }
                Err(error) => {
                    // Let the next pass try again.
                    tracing::warn!(error = format!("{error:#}"), "push failed");
                    store::unmark_push_sent(&state.db, &sub.endpoint, &dedupe).await?;
                }
            }
        }
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
            lat: row.get("lat"),
            lng: row.get("lng"),
            radius_km: row.get("radius_km"),
            timezone: row.get("timezone"),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_cuisines_naturally() {
        assert_eq!(cuisine_names(&["LB"]), "Lebanese");
        assert_eq!(cuisine_names(&["CN", "VN"]), "Chinese or Vietnamese");
        assert_eq!(cuisine_names(&["CN", "KR", "VN"]), "Chinese, Korean or Vietnamese");
    }
}
