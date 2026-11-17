use std::sync::Arc;

use mangiamundo::{api, calendar, config::Config, providers, push, store, AppState};
use tower_http::services::{ServeDir, ServeFile};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = Config::from_env()?;
    let (places, areas) = providers::build(&config)?;
    let db = store::connect(&config.database_url).await?;
    let pusher = push::Pusher::load(&db, config.vapid_private_key.clone(), config.vapid_subject.clone()).await?;
    let state = AppState {
        db,
        places,
        areas,
        cache_ttl_secs: config.cache_ttl_secs,
        clock: Arc::new(store::now),
        push_key: Some(pusher.public_key.clone()),
    };
    calendar::validate().map_err(anyhow::Error::msg)?;
    tracing::info!(provider = state.places.name(), "places provider ready");

    if config.push_interval_secs > 0 {
        tokio::spawn(push::run(state.clone(), pusher, std::time::Duration::from_secs(config.push_interval_secs)));
    }

    let mut app = api::router(state);
    if let Some(dir) = config.static_dir.filter(|dir| dir.join("index.html").is_file()) {
        tracing::info!(dir = %dir.display(), "serving web app");
        // The web app is a single-page app: unknown paths fall back to index.html.
        app = app.fallback_service(ServeDir::new(&dir).fallback(ServeFile::new(dir.join("index.html"))));
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
