use axum::response::{sse::Event, Sse};
use axum::Extension;
use axum_extra::{headers, TypedHeader};
use chat_core::User;
use futures::{stream, Stream};
use std::{convert::Infallible, time::Duration};
use tokio_stream::StreamExt;
use tracing::info;

pub(crate) async fn sse_handler(
    Extension(user): Extension<User>,
    TypedHeader(user_agent): TypedHeader<headers::UserAgent>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    info!(
        "`user: {} user agent: {}` connected",
        user.id,
        user_agent.as_str()
    );

    // A `Stream` that repeats an event every second
    //
    // You can also create streams from tokio channels using the wrappers in
    // https://docs.rs/tokio-stream
    let stream = stream::repeat_with(|| Event::default().data("hi!"))
        .map(Ok)
        .throttle(Duration::from_secs(1));

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive-text"),
    )
}
