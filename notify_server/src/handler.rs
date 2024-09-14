use crate::AppState;
use axum::response::Sse;
use axum::Extension;
use axum::{extract::State, response::sse::Event};
use chat_core::{Subscriber, User};
use futures::Stream;
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::StreamExt as _;
use tracing::info;

pub(crate) async fn sse_handler<T: Clone + Subscriber>(
    Extension(user): Extension<User>,
    State(state): State<AppState<T>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    info!("user: {} connected", user.id,);

    let user_id = user.id as u64;

    let stream = state
        .listener
        .subscribe(user_id)
        .await
        .unwrap()
        .map(|event| Event::default().data(serde_json::to_string(&event).unwrap()))
        .map(Ok::<_, Infallible>);

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive-text"),
    )
}
