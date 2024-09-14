use axum::response::sse::Event;
use axum::BoxError;
use chat_core::{Chat, Message};
use futures::TryStream;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct ChatUpdated {
    op: String,
    old: Option<Chat>,
    new: Option<Chat>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessageCreated {
    message: Message,
    members: Vec<i64>,
}

#[deprecated(note = "use Subscriber instead")]
pub trait Listener {
    type Stream: TryStream<Item = Result<Event, Self::Error>, Ok = Event, Error = Self::Error>
        + Send
        + 'static;
    type Error: Into<BoxError>;
    #[allow(dead_code)]
    fn subscribe(&self, user_id: u64) -> Self::Stream;
}
