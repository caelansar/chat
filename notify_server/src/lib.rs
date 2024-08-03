mod config;
mod error;
mod sse;

pub use crate::config::AppConfig;
use crate::error::AppError;
use axum::{
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use chat_core::middlewares::TokenVerify;
use chat_core::{Chat, DecodingKey, Message, User};
use dashmap::DashMap;
use futures::StreamExt;
use sqlx::postgres::PgListener;
use sse::sse_handler;
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::info;

const INDEX_HTML: &str = include_str!("../index.html");

pub type UserMap = Arc<DashMap<u64, broadcast::Sender<Arc<AppEvent>>>>;

#[derive(Clone)]
pub struct AppState(Arc<AppStateInner>);

pub struct AppStateInner {
    pub config: AppConfig,
    #[allow(unused)]
    users: UserMap,
    dk: DecodingKey,
}

pub enum AppEvent {
    NewChat(Chat),
    AddToChat(Chat),
    RemoveFromChat(Chat),
    NewMessage(Message),
}

pub fn get_router(config: AppConfig) -> Router {
    let state = AppState::new(config);

    Router::new()
        .route("/", get(index_handler))
        .route("/events", get(sse_handler))
        .with_state(state.clone())
}

pub async fn setup_pg_listener(config: AppConfig) -> anyhow::Result<()> {
    let mut listener = PgListener::connect(&config.server.db_url).await?;
    listener.listen("chat_updated").await?;
    listener.listen("chat_message_created").await?;

    let mut stream = listener.into_stream();

    tokio::spawn(async move {
        while let Some(Ok(notification)) = stream.next().await {
            info!("Received notification: {:?}", notification);
        }
    });

    Ok(())
}

async fn index_handler() -> impl IntoResponse {
    Html(INDEX_HTML)
}

impl TokenVerify for AppState {
    type Error = AppError;

    fn verify(&self, token: &str) -> Result<User, Self::Error> {
        Ok(self.dk.verify(token)?)
    }
}

impl Deref for AppState {
    type Target = AppStateInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let dk = DecodingKey::load(&config.auth.pk).expect("Failed to load public key");
        let users = Arc::new(DashMap::new());
        Self(Arc::new(AppStateInner { config, dk, users }))
    }
}
