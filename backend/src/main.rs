use std::sync::Arc;

use mangiamundo::config::{Config, ProviderKind};
use mangiamundo::providers::{self, osm};
use mangiamundo::{api, calendar, push, store, AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = Config::from_env()?;
    let (mut places, areas) = providers::build(&config)?;
    let db = store::connect(&config.database_url).await?;
    if config.provider == ProviderKind::Osm {
        // Belgium comes from a daily import, so searches there skip the slow live query.
        let http = reqwest::Client::builder().user_agent(concat!("mangiamundo/", env!("CARGO_PKG_VERSION"))).build()?;
        tokio::spawn(osm::run_imports(db.clone(), osm::Overpass::new(http, config.overpass_urls.clone())));
        places = Arc::new(osm::WithImport::new(db.clone(), places));
    }
    let mut pusher = push::Pusher::load(&db, config.vapid_private_key.clone(), config.vapid_subject.clone()).await?;
    let push_http = reqwest::Client::builder().timeout(std::time::Duration::from_secs(20)).build()?;
    match (&config.apns_key, &config.apns_key_id, &config.apns_team_id) {
        (Some(key), Some(key_id), Some(team_id)) => {
            let apns = push::apns::Apns::new(push_http.clone(), key, key_id.clone(), team_id.clone(), config.apns_topic.clone())?;
            pusher.apns = Some(Arc::new(apns));
        }
        (None, None, None) => {}
        _ => anyhow::bail!("set all of APNS_KEY, APNS_KEY_ID and APNS_TEAM_ID, or none"),
    }
    if let Some(account) = &config.fcm_service_account {
        pusher.fcm = Some(Arc::new(push::fcm::Fcm::new(push_http, account)?));
    }
    tracing::info!(ios = pusher.apns.is_some(), android = pusher.fcm.is_some(), "app push");
    let state = AppState {
        loads: Default::default(),
        limits: Default::default(),
        db,
        places,
        areas,
        cache_ttl_secs: config.cache_ttl_secs,
        clock: Arc::new(store::now),
        push_key: Some(pusher.public_key.clone()),
        push_apps: pusher.apps(),
        osm_notes_url: config.osm_notes_url.clone(),
        osm_notes_token: config.osm_notes_token.clone(),
    };
    calendar::validate().map_err(anyhow::Error::msg)?;
    tracing::info!(provider = state.places.name(), "places provider ready");

    if config.push_interval_secs > 0 {
        tokio::spawn(push::run(state.clone(), pusher, std::time::Duration::from_secs(config.push_interval_secs)));
    }

    let mut app = api::router(state);
    if let Some(dir) = config.static_dir.filter(|dir| dir.join("index.html").is_file()) {
        tracing::info!(dir = %dir.display(), "serving web app");
        app = api::with_web_app(app, &dir);
    }

    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    tracing::info!("listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
