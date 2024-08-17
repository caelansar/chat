use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::io;
use thiserror::Error;
use utoipa::ToSchema;

#[derive(Debug, ToSchema, Serialize, Deserialize)]
pub struct ErrorOutput {
    pub error: String,
}

impl ErrorOutput {
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
        }
    }
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("sql error: {0}")]
    SqlxError(#[from] sqlx::Error),

    #[error("io error: {0}")]
    IoError(#[from] io::Error),

    #[error("create chat error: {0}")]
    CreateChatError(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("password hash error: {0}")]
    PasswordHashError(#[from] argon2::password_hash::Error),

    #[error("anyhow error: {0}")]
    AnyhowError(#[from] anyhow::Error),

    #[error("email already exists: {0}")]
    EmailAlreadyExists(String),

    #[error("create message error: {0}")]
    CreateMessageError(String),

    #[error("{0}")]
    ChatFileError(String),

    #[error("no man's land")]
    StatusNotFound,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response<axum::body::Body> {
        let status = match &self {
            Self::SqlxError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::PasswordHashError(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::AnyhowError(_) => StatusCode::FORBIDDEN,
            Self::EmailAlreadyExists(_) => StatusCode::CONFLICT,
            Self::CreateChatError(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::IoError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::CreateMessageError(_) => StatusCode::BAD_REQUEST,
            Self::ChatFileError(_) => StatusCode::BAD_REQUEST,
            Self::StatusNotFound => StatusCode::NOT_FOUND,
        };

        (status, Json(ErrorOutput::new(self.to_string()))).into_response()
    }
}
