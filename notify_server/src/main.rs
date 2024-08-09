use anyhow::Result;
use notify_server::{get_router, AppConfig, AppState, PgNotify};
use tokio::net::TcpListener;
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::{fmt::Layer, layer::SubscriberExt, util::SubscriberInitExt, Layer as _};

#[tokio::main]
async fn main() -> Result<()> {
    let layer = Layer::new().with_filter(LevelFilter::DEBUG);
    tracing_subscriber::registry().with(layer).init();

    let config = AppConfig::load()?;
    let listener = PgNotify::new(&config.server.db_url).await?;
    let state = AppState::new(config, listener);
    let port = state.config.server.port;

    let app = get_router(state).await;

    let addr = format!("0.0.0.0:{}", port);

    let listener = TcpListener::bind(&addr).await?;
    info!("Listening on: {}", addr);

    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}
