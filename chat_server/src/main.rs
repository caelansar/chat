use anyhow::Result;
use chat_server::{get_router, AppConfig, AppState};
use tokio::net::TcpListener;
use tokio::signal;
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::{fmt::Layer, layer::SubscriberExt, util::SubscriberInitExt, Layer as _};

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::load()?;

    let layer = Layer::new().with_filter(LevelFilter::from_level(config.log_level));
    tracing_subscriber::registry().with(layer).init();

    let addr = format!("0.0.0.0:{}", config.server.port);

    let state = AppState::try_new(config).await?;

    let app = get_router(state).await?;
    let listener = TcpListener::bind(&addr).await?;
    info!("Listening on: {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {info!("shutdown gracefully")},
        _ = terminate => {},
    }
}
