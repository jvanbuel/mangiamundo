//! Push notifications: celebration and event reminders for browsers (Web Push), and, for
//! those who opt in, news of food from a country they haven't tasted yet becoming available
//! in their area, for browsers and for the iOS (APNs) and Android (FCM) apps. The apps
//! schedule celebration reminders themselves.
//!
//! Every few minutes the server looks at each subscription in the subscriber's own
//! time zone. From 10:00 on the day a celebration or event starts it sends one
//! notification, but only when the subscriber's area has places of that country.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration as StdDuration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{Duration, NaiveDate, Timelike};
use serde::Serialize;
use sqlx::{Row, SqlitePool};
use web_push::{
    request_builder, ContentEncoding, SubscriptionInfo, VapidSignatureBuilder,
    WebPushMessageBuilder,
};

pub mod apns;
pub mod fcm;

use crate::providers::osm;
use crate::store::Area;
use crate::{calendar, catalog, store, AppState};

/// Reminders are sent from this local hour until the end of the evening.
const FIRST_HOUR: u32 = 10;
const LAST_HOUR: u32 = 20;
/// "Last days" reminders for events go out this many days before the end.
const LAST_CHANCE_DAYS: i64 = 3;
/// A country gone from the area this long is forgotten, and announced again if it comes
/// back. Shorter absences (a closed report, a reverted edit) are not announced twice.
const FORGET_AFTER_SECS: i64 = 28 * 86_400;
/// At most this many cuisines are named in one announcement; the rest are counted.
const NAMED_CUISINES: usize = 3;

#[derive(Clone)]
pub struct Pusher {
    http: reqwest::Client,
    /// Raw P-256 private key, base64url.
    private_key: String,
    /// Public key for browsers (`applicationServerKey`), base64url.
    pub public_key: String,
    /// Contact for push services: a `mailto:` or `https:` URL.
    subject: String,
    /// For the iOS app, when an APNs key is configured.
    pub apns: Option<Arc<apns::Apns>>,
    /// For the Android app, when a Firebase service account is configured.
    pub fcm: Option<Arc<fcm::Fcm>>,
}

/// The apps the server can send to: those with a key configured.
#[derive(Debug, Clone, Copy, Default)]
pub struct Apps {
    pub ios: bool,
    pub android: bool,
}

/// Where a subscription's notifications go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A browser, through its push service.
    Web,
    /// The iOS app, through APNs.
    Apns,
    /// A development build of the iOS app, through the APNs sandbox.
    ApnsSandbox,
    /// The Android app, through FCM.
    Fcm,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Web => "web",
            Kind::Apns => "apns",
            Kind::ApnsSandbox => "apns-sandbox",
            Kind::Fcm => "fcm",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "web" => Kind::Web,
            "apns" => Kind::Apns,
            "apns-sandbox" => Kind::ApnsSandbox,
            "fcm" => Kind::Fcm,
            _ => return None,
        })
    }

    /// The `endpoint` an app's device token is stored under.
    pub fn endpoint(self, device_token: &str) -> String {
        format!("{}:{device_token}", self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct Subscription {
    pub kind: Kind,
    /// A browser's push URL, or `<kind>:<device token>` for an app.
    pub endpoint: String,
    /// Web Push keys; empty for apps.
    pub p256dh: String,
    pub auth: String,
    /// The area notifications are about; `None` for subscriptions from before areas, until
    /// the app subscribes again.
    pub area_id: Option<String>,
    pub timezone: String,
    /// Celebration and event reminders. Always off for apps, which schedule them themselves.
    pub reminders: bool,
    /// News of food not tasted yet becoming available in the area.
    pub new_food: bool,
}

impl Subscription {
    /// The app's device token, after the `<kind>:` prefix.
    fn device_token(&self) -> &str {
        self.endpoint.split_once(':').map_or("", |(_, token)| token)
    }
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
        Ok(Self { http, private_key, public_key, subject, apns: None, fcm: None })
    }

    pub fn apps(&self) -> Apps {
        Apps { ios: self.apns.is_some(), android: self.fcm.is_some() }
    }

    /// Whether notifications of this kind can be sent.
    pub fn handles(&self, kind: Kind) -> bool {
        match kind {
            Kind::Web => true,
            Kind::Apns | Kind::ApnsSandbox => self.apns.is_some(),
            Kind::Fcm => self.fcm.is_some(),
        }
    }

    pub async fn send(&self, to: &Subscription, notification: &Notification) -> anyhow::Result<Delivery> {
        // Apple and Google check the signed time against their clocks, so use the real one.
        let now = store::now();
        match to.kind {
            Kind::Web => self.send_web(to, notification).await,
            Kind::Apns | Kind::ApnsSandbox => match &self.apns {
                Some(apns) => apns.send(to.device_token(), to.kind == Kind::ApnsSandbox, notification, now).await,
                None => anyhow::bail!("APNs is not configured"),
            },
            Kind::Fcm => match &self.fcm {
                Some(fcm) => fcm.send(to.device_token(), notification, now).await,
                None => anyhow::bail!("FCM is not configured"),
            },
        }
    }

    async fn send_web(&self, to: &Subscription, notification: &Notification) -> anyhow::Result<Delivery> {
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
    join(&adjectives(codes), "or")
}

/// "Lebanese" for LB, or the country's name when it has no cuisine of its own.
fn adjectives(codes: &[&str]) -> Vec<String> {
    codes
        .iter()
        .filter_map(|code| catalog::cuisine_adjective(code).or_else(|| Some(catalog::country(code)?.name.clone())))
        .collect()
}

/// "a", "a or b", "a, b or c".
fn join(words: &[String], conjunction: &str) -> String {
    match words {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} {conjunction} {last}", rest.join(", ")),
    }
}

fn places(n: u32) -> String {
    if n == 1 { "1 place".into() } else { format!("{n} places") }
}

/// The reminder for a celebration, or None when none of its countries has places nearby.
/// The apps use the same text for the notifications they schedule themselves.
pub fn celebration_notification(celebration: &calendar::Celebration, counts: &HashMap<String, u32>) -> Option<Notification> {
    let nearby: Vec<&str> = celebration
        .countries
        .iter()
        .map(String::as_str)
        .filter(|c| counts.get(*c).copied().unwrap_or(0) > 0)
        .collect();
    let first = *nearby.first()?;
    let total: u32 = nearby.iter().map(|c| counts[*c]).sum();
    let when = if celebration.ends > celebration.starts { "starts today" } else { "is today" };
    Some(Notification {
        title: format!("{} {when}", celebration.title),
        body: format!(
            "{} Celebrate at one of {} serving {} food near you.",
            celebration.description,
            places(total),
            cuisine_names(&nearby)
        ),
        url: format!("/?country={first}"),
        tag: format!("celebration-{}", celebration.id),
    })
}

/// The announcement of food newly available in `area`: each new country with the names of
/// its places there. None when there is nothing new.
pub fn new_countries_notification(area: &str, new: &[(&str, Vec<&str>)]) -> Option<Notification> {
    let (first, first_places) = new.first()?;
    let codes: Vec<&str> = new.iter().map(|(code, _)| *code).collect();
    let total: usize = new.iter().map(|(_, names)| names.len()).sum();
    let (title, body) = match new {
        [_] => {
            let cuisine = cuisine_names(&codes);
            let name = first_places.first().copied().unwrap_or("A new place");
            let body = match first_places.len() {
                0 | 1 => format!("Try {name} in {area}."),
                2 => format!("Try {name} or 1 more place in {area}."),
                n => format!("Try {name} or {} more places in {area}.", n - 1),
            };
            (format!("{cuisine} food is now near you"), body)
        }
        _ if new.len() <= NAMED_CUISINES => (
            format!("{} food is now near you", join(&adjectives(&codes), "and")),
            format!("{} in {area} serve food you haven't tasted yet.", places(total as u32)),
        ),
        _ => {
            let mut named = adjectives(&codes[..NAMED_CUISINES]);
            named.push(format!("{} more", new.len() - NAMED_CUISINES));
            (
                format!("Food from {} more countries is now near you", new.len()),
                format!("{}: {} in {area}.", join(&named, "and"), places(total as u32)),
            )
        }
    };
    let url = if new.len() == 1 { format!("/?country={first}") } else { "/".into() };
    Some(Notification { title, body, url, tag: format!("new-food-{}", codes.join("-")) })
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
        if let Some(notification) = celebration_notification(&celebration, counts) {
            out.push((format!("celebration:{}:{day}", celebration.id), notification));
        }
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
            let done = store::visits_between(&state.db, user, &event.countries, event.starts, event.ends).await?;
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

/// A notification due for one subscriber.
struct Due {
    notification: Notification,
    /// Sent once ever, recorded in `push_sent` under this key.
    dedupe: Option<String>,
    /// Countries it announces, known to the subscriber once it is sent.
    announces: Vec<String>,
}

/// One pass over all subscriptions. Returns how many notifications were sent.
pub async fn run_once(state: &AppState, pusher: &Pusher) -> anyhow::Result<usize> {
    let now = chrono::DateTime::from_timestamp(state.now(), 0).unwrap_or_default();
    let mut sent = 0;
    for (user, sub) in store::push_subscriptions(&state.db).await? {
        if !pusher.handles(sub.kind) {
            continue;
        }
        let zone: chrono_tz::Tz = sub.timezone.parse().unwrap_or(chrono_tz::UTC);
        let local = now.with_timezone(&zone);
        let Some(area_id) = &sub.area_id else { continue };
        let Some(area) = current_area(state, area_id).await? else { continue };
        let places = store::area_country_places(&state.db, area_id).await?;
        let mut counts: HashMap<String, u32> = HashMap::new();
        for (country, _) in &places {
            *counts.entry(country.clone()).or_default() += 1;
        }
        let mut present: Vec<&str> = counts.keys().map(String::as_str).collect();
        present.sort_unstable();

        // A new subscription, or one that moved: take what the area has without announcing
        // it, at any hour, so subscribing doesn't bring a burst of notifications.
        let known = store::known_countries(&state.db, &sub.endpoint, area_id).await?;
        if known.is_none() {
            store::reset_known_countries(&state.db, &sub.endpoint, area_id, &present, state.now()).await?;
        }
        if !(FIRST_HOUR..=LAST_HOUR).contains(&local.hour()) {
            continue;
        }

        let mut due: Vec<Due> = Vec::new();
        if sub.reminders && sub.kind == Kind::Web {
            for (dedupe, notification) in self::due(state, &user, local.date_naive(), &counts).await? {
                due.push(Due { notification, dedupe: Some(dedupe), announces: vec![] });
            }
        }
        if let Some(known) = known {
            if sub.new_food {
                if let Some(announcement) = new_countries(state, &user, &sub.endpoint, &area, &places, &present, &known).await? {
                    due.push(announcement);
                }
            } else {
                // Keep up with the area, so opting in later starts from now.
                store::see_known_countries(&state.db, &sub.endpoint, &present, state.now()).await?;
            }
        }

        for item in due {
            if let Some(dedupe) = &item.dedupe {
                if !store::mark_push_sent(&state.db, &sub.endpoint, dedupe, state.now()).await? {
                    continue;
                }
            }
            match pusher.send(&sub, &item.notification).await {
                Ok(Delivery::Sent) => {
                    sent += 1;
                    let announced: Vec<&str> = item.announces.iter().map(String::as_str).collect();
                    store::see_known_countries(&state.db, &sub.endpoint, &announced, state.now()).await?;
                }
                Ok(Delivery::Gone) => {
                    store::remove_push_subscription(&state.db, &sub.endpoint).await?;
                    break;
                }
                Err(error) => {
                    // Let the next pass try again.
                    tracing::warn!(error = format!("{error:#}"), "push failed");
                    if let Some(dedupe) = &item.dedupe {
                        store::unmark_push_sent(&state.db, &sub.endpoint, dedupe).await?;
                    }
                }
            }
        }
    }
    Ok(sent)
}

/// The cached area, rebuilt first from the local OpenStreetMap copy when it is stale and
/// the copy covers it. Never asks Overpass or another live provider: that waits for
/// someone to open the app.
async fn current_area(state: &AppState, id: &str) -> anyhow::Result<Option<Area>> {
    let Some(mut area) = store::find_area(&state.db, id).await? else { return Ok(None) };
    if state.now() - area.fetched_at >= state.cache_ttl_secs {
        if let Some(found) = osm::local_places(&state.db, &area.boundary).await? {
            area.fetched_at = state.now();
            store::save_area(&state.db, &area, &found).await?;
            tracing::info!(area = area.id(), places = found.len(), "area refreshed for push");
        }
    }
    Ok(Some(area))
}

/// Compares the area's countries with the ones the subscriber knows it has. Countries the
/// user has tasted elsewhere become known quietly; the others are announced together.
async fn new_countries(
    state: &AppState,
    user: &str,
    endpoint: &str,
    area: &Area,
    places: &[(String, String)],
    present: &[&str],
    known: &HashSet<String>,
) -> anyhow::Result<Option<Due>> {
    let new: Vec<&str> = present.iter().copied().filter(|c| !known.contains(*c)).collect();
    let tasted = if new.is_empty() { HashSet::new() } else { store::visited_countries(&state.db, user).await? };
    let fresh: Vec<&str> = new.iter().copied().filter(|c| !tasted.contains(*c)).collect();
    let quiet: Vec<&str> = present.iter().copied().filter(|c| !fresh.contains(c)).collect();
    store::see_known_countries(&state.db, endpoint, &quiet, state.now()).await?;
    store::forget_known_countries(&state.db, endpoint, state.now() - FORGET_AFTER_SECS).await?;

    let new: Vec<(&str, Vec<&str>)> = fresh
        .iter()
        .map(|c| (*c, places.iter().filter(|(country, _)| country == c).map(|(_, name)| name.as_str()).collect()))
        .collect();
    Ok(new_countries_notification(&area.boundary.info.name, &new).map(|notification| Due {
        notification,
        dedupe: None,
        announces: fresh.iter().map(|c| c.to_string()).collect(),
    }))
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

/// Reads a subscription row; `None` for a kind this version doesn't know.
pub(crate) fn subscription_from_row(row: &sqlx::sqlite::SqliteRow) -> Option<(String, Subscription)> {
    Some((
        row.get("user_id"),
        Subscription {
            kind: Kind::parse(row.get("kind"))?,
            endpoint: row.get("endpoint"),
            p256dh: row.get("p256dh"),
            auth: row.get("auth"),
            area_id: row.get("area_id"),
            timezone: row.get("timezone"),
            reminders: row.get("reminders"),
            new_food: row.get("new_food"),
        },
    ))
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

    #[test]
    fn announces_new_food() {
        assert!(new_countries_notification("Leuven", &[]).is_none());

        let one = new_countries_notification("Leuven", &[("LB", vec!["Byblos"])]).unwrap();
        assert_eq!(one.title, "Lebanese food is now near you");
        assert_eq!(one.body, "Try Byblos in Leuven.");
        assert_eq!(one.url, "/?country=LB");

        let more = new_countries_notification("Leuven", &[("LB", vec!["Byblos", "Cedar", "Tyr"])]).unwrap();
        assert_eq!(more.body, "Try Byblos or 2 more places in Leuven.");

        let two = new_countries_notification("Leuven", &[("LB", vec!["Byblos"]), ("PE", vec!["Inca", "Lima"])]).unwrap();
        assert_eq!(two.title, "Lebanese and Peruvian food is now near you");
        assert_eq!(two.body, "3 places in Leuven serve food you haven't tasted yet.");
        assert_eq!(two.url, "/");
        assert_eq!(two.tag, "new-food-LB-PE");

        let many: Vec<(&str, Vec<&str>)> = ["LB", "PE", "TH", "VN", "JP"].iter().map(|c| (*c, vec!["x"])).collect();
        let many = new_countries_notification("Leuven", &many).unwrap();
        assert_eq!(many.title, "Food from 5 more countries is now near you");
        assert_eq!(many.body, "Lebanese, Peruvian, Thai and 2 more: 5 places in Leuven.");
    }
}
