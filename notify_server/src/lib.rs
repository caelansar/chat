mod config;
mod error;
mod handler;
mod notify;

pub use crate::config::AppConfig;
use crate::error::AppError;
use crate::notify::{AppEvent, Notification};
use axum::middleware::{from_fn, from_fn_with_state};
use axum::{
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use chat_core::middlewares::{log_headers, verify_token, TokenVerify};
use chat_core::{DecodingKey, User};
use dashmap::DashMap;
use futures::StreamExt;
use handler::sse_handler;
use sqlx::postgres::PgListener;
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::services::ServeDir;
use tracing::{info, warn};

const INDEX_HTML: &str = include_str!("../assets/index.html");

pub type UserMap = Arc<DashMap<u64, broadcast::Sender<Arc<AppEvent>>>>;

#[derive(Clone)]
pub struct AppState(Arc<AppStateInner>);

pub struct AppStateInner {
    pub config: AppConfig,
    users: UserMap,
    dk: DecodingKey,
}

pub async fn get_router(state: AppState) -> Router {
    setup_pg_listener(state.clone()).await.unwrap();

    Router::new()
        .route("/events", get(sse_handler))
        .layer(from_fn(log_headers))
        .layer(from_fn_with_state(state.clone(), verify_token::<AppState>))
        .route("/", get(index_handler))
        .nest_service("/assets", ServeDir::new("assets"))
        .with_state(state.clone())
}

pub async fn setup_pg_listener(state: AppState) -> anyhow::Result<()> {
    let mut listener = PgListener::connect(&state.config.server.db_url).await?;
    listener.listen("chat_updated").await?;
    listener.listen("chat_message_created").await?;

    let mut stream = listener.into_stream();

    tokio::spawn(async move {
        while let Some(Ok(notification)) = stream.next().await {
            info!("Received notification: {:?}", notification);
            match Notification::load(notification.channel(), notification.payload()) {
                Ok(notification) => {
                    info!("user_ids: {:?}", notification.user_ids);
                    for user_id in notification.user_ids {
                        if let Some(tx) = state.users.get(&(user_id as u64)) {
                            info!("sending notification to user: {}", user_id);
                            if let Err(err) = tx.send(notification.event.clone()) {
                                warn!(
                                    "failed to send notification to user: {}, err: {:?}",
                                    user_id, err
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("failed to load notification: {:?}", e);
                }
            }
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

#[cfg(test)]
mod tests {
    use crate::AppEvent;
    use chat_core::{Chat, ChatType};
    use chrono::DateTime;

    #[test]
    fn test_deserialize_app_event() {
        let event = AppEvent::NewChat(Chat {
            id: 1,
            ws_id: 1,
            name: Some("chat1".to_string()),
            r#type: ChatType::Single,
            members: vec![1, 2, 3],
            created_at: DateTime::from_timestamp(1722751531, 0).unwrap(),
        });
        let data = serde_json::to_string(&event).unwrap();

        assert_eq!(
            r#"{"event":"NewChat","id":1,"ws_id":1,"name":"chat1","type":"single","members":[1,2,3],"created_at":"2024-08-04T06:05:31Z"}"#,
            data
        );

        let event1: AppEvent = serde_json::from_str(&data).unwrap();

        assert_eq!(event, event1);
    }
}
