use crate::models::UserRepo;
use crate::{
    models::{CreateUser, SigninUser},
    AppError, AppState, ErrorOutput,
};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};
use utoipa::ToSchema;

#[derive(Debug, ToSchema, Serialize, Deserialize)]
pub struct AuthOutput {
    token: String,
}

#[utoipa::path(
    post,
    path = "/api/signup",
    responses(
        (status = 200, description = "User created", body = AuthOutput),
    ),
    tag = "auth",
)]
#[instrument(level = "debug", skip(state))]
pub(crate) async fn signup_handler(
    State(state): State<AppState>,
    Json(input): Json<CreateUser>,
) -> Result<impl IntoResponse, AppError> {
    info!("create user");
    let user = UserRepo::create(&input, &state.pool).await?;
    let token = state.ek.sign(user)?;
    let body = Json(AuthOutput { token });
    Ok((StatusCode::CREATED, body))
}

#[utoipa::path(
    post,
    path = "/api/signin",
    responses(
        (status = 200, description = "User signed in", body = AuthOutput),
    ),
    tag = "auth",
)]
#[instrument(level = "debug", skip(state))]
pub(crate) async fn signin_handler(
    State(state): State<AppState>,
    Json(input): Json<SigninUser>,
) -> Result<impl IntoResponse, AppError> {
    info!("verify user");
    let user = UserRepo::verify(&input, &state.pool).await?;

    match user {
        Some(user) => {
            let token = state.ek.sign(user)?;
            Ok((StatusCode::OK, Json(AuthOutput { token })).into_response())
        }
        None => {
            let body = Json(ErrorOutput::new("Invalid email or password"));
            Ok((StatusCode::FORBIDDEN, body).into_response())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use http_body_util::BodyExt;

    #[tokio::test]
    async fn signup_should_work() -> Result<()> {
        let (_tdb, state) = AppState::new_for_test().await?;
        let input = CreateUser::new("none", "cae", "cae@cae.com", "123456");
        let ret = signup_handler(State(state), Json(input))
            .await?
            .into_response();
        assert_eq!(ret.status(), StatusCode::CREATED);
        let body = ret.into_body().collect().await?.to_bytes();
        let ret: AuthOutput = serde_json::from_slice(&body)?;
        assert!(!ret.token.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn signup_duplicate_user_should_409() -> Result<()> {
        let (_tdb, state) = AppState::new_for_test().await?;
        let input = CreateUser::new("none", "cae", "cae@cae.com", "123456");
        signup_handler(State(state.clone()), Json(input.clone())).await?;
        let ret = signup_handler(State(state.clone()), Json(input.clone()))
            .await
            .into_response();
        assert_eq!(ret.status(), StatusCode::CONFLICT);
        let body = ret.into_body().collect().await?.to_bytes();
        let ret: ErrorOutput = serde_json::from_slice(&body)?;

        assert_eq!(ret.error, "email already exists: cae@cae.com");
        Ok(())
    }

    #[tokio::test]
    async fn signin_should_work() -> Result<()> {
        let (_tdb, state) = AppState::new_for_test().await?;
        let email = "alice@cae.org";
        let password = "123456";
        let input = SigninUser::new(email, password);
        let ret = signin_handler(State(state), Json(input))
            .await?
            .into_response();
        assert_eq!(ret.status(), StatusCode::OK);
        let body = ret.into_body().collect().await?.to_bytes();
        let ret: AuthOutput = serde_json::from_slice(&body)?;
        assert!(!ret.token.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn signin_with_non_exist_user_should_403() -> Result<()> {
        let (_tdb, state) = AppState::new_for_test().await?;
        let email = "cae1@cae.org";
        let password = "123456";
        let input = SigninUser::new(email, password);
        let ret = signin_handler(State(state), Json(input))
            .await
            .into_response();
        assert_eq!(ret.status(), StatusCode::FORBIDDEN);
        let body = ret.into_body().collect().await?.to_bytes();
        let ret: ErrorOutput = serde_json::from_slice(&body)?;
        assert_eq!(ret.error, "Invalid email or password");

        Ok(())
    }
}
