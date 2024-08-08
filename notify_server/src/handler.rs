use crate::notify::AppEvent;
use crate::AppState;
use axum::extract::State;
use axum::response::{sse::Event, Sse};
use axum::Extension;
use chat_core::User;
use futures::Stream;
use std::{convert::Infallible, time::Duration};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tracing::info;

const CHANNEL_CAPACITY: usize = 100;

pub(crate) async fn sse_handler(
    Extension(user): Extension<User>,
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    info!("user: {} connected", user.id,);

    let user_id = user.id as u64;
    let users = &state.users;

    let rx = if let Some(tx) = users.get(&user_id) {
        tx.subscribe()
    } else {
        let (tx, rx) = broadcast::channel(CHANNEL_CAPACITY);
        info!("insert user: {user_id}");
        users.insert(user_id, tx);
        rx
    };
    info!("user {user_id} subscribed");

    let stream = BroadcastStream::new(rx).filter_map(|e| {
        e.ok().map(|e| {
            let name = match e.as_ref() {
                AppEvent::NewChat(_) => "NewChat",
                AppEvent::AddToChat(_) => "AddToChat",
                AppEvent::RemoveFromChat(_) => "RemoveFromChat",
                AppEvent::NewMessage(_) => "NewMessage",
            };
            let v = serde_json::to_string(&e).expect("Failed to serialize event");
            Ok(Event::default().data(v).event(name))
        })
    });

    // A `Stream` that repeats an event every second
    //
    // You can also create streams from tokio channels using the wrappers in
    // https://docs.rs/tokio-stream
    // let stream = stream::repeat_with(|| Event::default().data("hi!"))
    //     .map(Ok)
    //     .throttle(Duration::from_secs(1));

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive-text"),
    )
}
