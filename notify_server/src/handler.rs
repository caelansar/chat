use crate::notify::Listener;
use crate::AppState;
use axum::extract::State;
use axum::response::Sse;
use axum::Extension;
use chat_core::User;
use futures::Stream;
use std::time::Duration;
use tracing::info;

pub(crate) async fn sse_handler<T: Clone + Listener>(
    Extension(user): Extension<User>,
    State(state): State<AppState<T>>,
) -> Sse<impl Stream<Item = <<T as Listener>::Stream as Stream>::Item>> {
    info!("user: {} connected", user.id,);

    let user_id = user.id as u64;

    let stream = state.listener.subscribe(user_id);

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive-text"),
    )
}
