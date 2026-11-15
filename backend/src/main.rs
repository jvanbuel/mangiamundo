use mangiamundo::{api, config::Config, providers, store, AppState};
use tower_http::services::{ServeDir, ServeFile};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = Config::from_env()?;
    let (places, geocoder) = providers::build(&config)?;
    let state = AppState {
        db: store::connect(&config.database_url).await?,
        places,
        geocoder,
        cache_ttl_secs: config.cache_ttl_secs,
    };
    tracing::info!(provider = state.places.name(), "places provider ready");

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
