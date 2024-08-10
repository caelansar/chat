mod config;
mod error;
mod handlers;
mod models;
mod openapi;

use anyhow::Context;

use handlers::*;
use openapi::OpenApiRouter;
use sqlx::{ConnectOptions, PgPool};
use std::time::Duration;
use std::{ops::Deref, sync::Arc};

use axum::http::StatusCode;
use axum::{
    middleware::from_fn_with_state,
    routing::{get, patch, post},
    Router,
};
use chat_core::middlewares::{set_layer, verify_token, TokenVerify};
use chat_core::{DecodingKey, EncodingKey, User};
pub use config::AppConfig;
pub use error::AppError;
pub use error::ErrorOutput;
pub use models::MessageRepo;
use sqlx::pool::PoolOptions;
use sqlx::postgres::PgConnectOptions;

#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

#[allow(unused)]
pub struct AppStateInner {
    pub(crate) config: AppConfig,
    pub(crate) dk: DecodingKey,
    pub(crate) ek: EncodingKey,
    pub(crate) pool: PgPool,
    pub(crate) message: MessageRepo,
}

impl TokenVerify for AppState {
    type Error = AppError;

    fn verify(&self, token: &str) -> Result<User, Self::Error> {
        self.dk.verify(token).map_err(AppError::AnyhowError)
    }
}

pub async fn get_router(state: AppState) -> Result<Router, AppError> {
    let api = Router::new()
        .route("/chats", get(list_chat_handler).post(create_chat_handler))
        .route(
            "/chats/:id",
            patch(update_chat_handler)
                .get(get_chat_handler)
                .delete(delete_chat_handler)
                .post(send_message_handler),
        )
        .route("/chats/:id/messages", get(list_message_handler))
        .route("/upload", post(upload_handler))
        .route("/files/:ws_id/*path", get(file_handler))
        .layer(from_fn_with_state(state.clone(), verify_token::<AppState>))
        .route("/signin", post(signin_handler))
        .route("/signup", post(signup_handler));

    let router = Router::new()
        .openapi()
        .route("/", get(index_handler))
        .nest("/api", api)
        .with_state(state);

    Ok(set_layer(router).fallback(|| async { (StatusCode::NOT_FOUND, "no man's land") }))
}

impl Deref for AppState {
    type Target = AppStateInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl AppState {
    pub async fn try_new(config: AppConfig) -> Result<Self, AppError> {
        let dk = DecodingKey::load(&config.auth.pk).context("load pk failed")?;
        let ek = EncodingKey::load(&config.auth.sk).context("load sk failed")?;

        let opts: PgConnectOptions = config.server.db_url.parse()?;
        let opts = opts.log_statements(log::LevelFilter::Debug);

        let pool = PoolOptions::new();
        let pool = pool.acquire_timeout(Duration::from_secs(5));

        let pool = pool
            .connect_with(opts)
            .await
            .context("connect to db failed")?;

        let base_dir = config.server.base_dir.clone();

        Ok(Self {
            inner: Arc::new(AppStateInner {
                config,
                ek,
                dk,
                pool,
                message: MessageRepo::new(base_dir),
            }),
        })
    }
}

#[cfg(feature = "test-util")]
mod test_util {
    use super::*;
    use sqlx::{Executor, PgPool};
    use sqlx_db_tester::TestPg;

    impl AppState {
        pub async fn new_for_test() -> Result<(TestPg, Self), AppError> {
            let config = AppConfig::load()?;
            let dk = DecodingKey::load(&config.auth.pk).context("load pk failed")?;
            let ek = EncodingKey::load(&config.auth.sk).context("load sk failed")?;
            let server_url = config.server.db_url.rsplit_once('/').unwrap().0;
            let (tdb, pool) = get_test_pool(Some(server_url)).await;

            let base_dir = config.server.base_dir.clone();

            let state = Self {
                inner: Arc::new(AppStateInner {
                    config,
                    ek,
                    dk,
                    pool,
                    message: MessageRepo::new(base_dir),
                }),
            };
            Ok((tdb, state))
        }
    }

    pub async fn get_test_pool(url: Option<&str>) -> (TestPg, PgPool) {
        let url = url
            .map(|x| x.to_string())
            .unwrap_or("postgres://postgres:postgres@localhost:5432".to_string());

        let tdb = TestPg::new(url, std::path::Path::new("../migrations"));
        let pool = tdb.get_pool().await;

        // insert test records
        let sql = include_str!("../asserts/test.sql").split(';');
        let mut ts = pool.begin().await.expect("begin transaction failed");
        for s in sql {
            if s.trim().is_empty() {
                continue;
            }
            ts.execute(s).await.expect("execute sql failed");
        }
        ts.commit().await.expect("commit transaction failed");

        (tdb, pool)
    }
}
