#![feature(impl_trait_in_assoc_type)]

mod config;
mod error;
mod handler;
mod notify;

pub use crate::config::AppConfig;
use crate::error::AppError;
use crate::notify::AppEvent;
use crate::notify::Listener;
use axum::middleware::{from_fn, from_fn_with_state};
use axum::{
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use chat_core::middlewares::{log_headers, verify_token, TokenVerify};
use chat_core::{DecodingKey, User};
use dashmap::DashMap;
use handler::sse_handler;
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::services::ServeDir;

pub use notify::PgNotify;

const INDEX_HTML: &str = include_str!("../assets/index.html");

pub type UserMap = Arc<DashMap<u64, broadcast::Sender<Arc<AppEvent>>>>;

#[derive(Clone)]
pub struct AppState<L: Clone>(Arc<AppStateInner<L>>);

pub struct AppStateInner<L: Clone> {
    pub config: AppConfig,
    dk: DecodingKey,
    listener: L,
}

pub async fn get_router(state: AppState<PgNotify>) -> Router {
    // setup_pg_listener(state.clone()).await.unwrap();

    Router::new()
        .route("/events", get(sse_handler))
        .layer(from_fn(log_headers))
        .layer(from_fn_with_state(
            state.clone(),
            verify_token::<AppState<PgNotify>>,
        ))
        .route("/", get(index_handler))
        .nest_service("/assets", ServeDir::new("assets"))
        .with_state(state.clone())
}

async fn index_handler() -> impl IntoResponse {
    Html(INDEX_HTML)
}

impl<T: Clone> TokenVerify for AppState<T> {
    type Error = AppError;

    fn verify(&self, token: &str) -> Result<User, Self::Error> {
        Ok(self.dk.verify(token)?)
    }
}

impl<T: Clone> Deref for AppState<T> {
    type Target = AppStateInner<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Listener + Clone> AppState<T> {
    pub fn new(config: AppConfig, listener: T) -> Self {
        let dk = DecodingKey::load(&config.auth.pk).expect("Failed to load public key");
        Self(Arc::new(AppStateInner {
            config,
            dk,
            listener,
        }))
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
