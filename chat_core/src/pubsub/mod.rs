/// This module is used to publish and subscribe to events in the chat system.
mod amqp;

use serde::{Deserialize, Serialize};

use crate::{Chat, Message};

/// A trait for a subscriber that can subscribe to events.
pub trait Subscriber {
    type Stream: futures::Stream<Item = AppMessage> + Send + 'static;
    #[allow(unused)]
    async fn subscribe(&self, user_id: u64) -> anyhow::Result<Self::Stream>;
}

/// A trait for a publisher that can publish events.
pub trait Publisher {
    #[allow(unused)]
    async fn publish(&self, event: AppMessage) -> anyhow::Result<()>;
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AppMessage {
    pub user_id: u64,
    pub event: AppEvent,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(tag = "event")]
pub enum AppEvent {
    NewChat(Chat),
    AddToChat(Chat),
    RemoveFromChat(Chat),
    NewMessage(Message),
}
