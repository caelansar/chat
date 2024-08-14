use crate::models::ChatRepo;
use crate::{models::CreateChat, AppError, AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use chat_core::User;
use tracing::info;

#[utoipa::path(
    get,
    path = "/api/chats",
    responses(
        (status = 200, description = "List of chats", body = Vec<Chat>),
    ),
    security(
        ("token" = [])
    ),
    tag = "chat",
)]
pub(crate) async fn list_chat_handler(
    Extension(user): Extension<User>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let chat = ChatRepo::fetch_all(user.ws_id as _, &state.pool).await?;
    Ok((StatusCode::OK, Json(chat)))
}

#[utoipa::path(
    post,
    path = "/api/chats",
    responses(
        (status = 201, description = "Chat created", body = Chat),
    ),
    security(
        ("token" = [])
    ),
    tag = "chat",
)]
pub(crate) async fn create_chat_handler(
    Extension(user): Extension<User>,
    State(state): State<AppState>,
    Json(input): Json<CreateChat>,
) -> Result<impl IntoResponse, AppError> {
    // TODO: validate whether all members are existed
    let chat = ChatRepo::create(input, user.ws_id as _, &state.pool).await?;
    Ok((StatusCode::CREATED, Json(chat)))
}

#[utoipa::path(
    get,
    path = "/api/chats/{id}",
    params(
        ("id" = u64, Path, description = "Chat id")
    ),
    responses(
        (status = 200, description = "Chat found", body = Chat),
        (status = 404, description = "Chat not found", body = ErrorOutput),
    ),
    security(
        ("token" = [])
    ),
    tag = "chat",
)]
pub(crate) async fn get_chat_handler(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    let chat = ChatRepo::get_by_id(id as _, &state.pool).await?;
    match chat {
        Some(chat) => Ok(Json(chat)),
        None => Err(AppError::NotFound(format!("chat id {id}"))),
    }
}

pub(crate) async fn update_chat_handler(Path(_id): Path<u64>) -> impl IntoResponse {
    "update chat"
}

pub(crate) async fn delete_chat_handler(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    let chat = ChatRepo::get_by_id(id as _, &state.pool).await?;
    if chat.is_none() {
        return Err(AppError::NotFound(format!("chat id {id}")));
    }

    let id = ChatRepo::delete_by_id(id as _, &state.pool).await?;
    info!("delete chat: {id}");

    Ok(StatusCode::NO_CONTENT)
}
