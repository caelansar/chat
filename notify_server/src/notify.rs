use crate::UserMap;
use axum::response::sse::Event;
use axum::BoxError;
use chat_core::{Chat, Message};
use dashmap::DashMap;
use futures::TryStream;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgListener;
use std::collections::HashSet;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};
use tracing::{info, instrument, warn};

const CHANNEL_CAPACITY: usize = 100;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum AppEvent {
    NewChat(Chat),
    AddToChat(Chat),
    RemoveFromChat(Chat),
    NewMessage(Message),
}

#[derive(Debug)]
pub(crate) struct Notification {
    // users are affected, we should send notification
    pub(crate) user_ids: HashSet<i64>,
    pub(crate) event: Arc<AppEvent>,
}

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

#[derive(Clone, Debug)]
pub struct PgNotify {
    users: UserMap,
}

pub trait Listener {
    type Stream: TryStream<Item = Result<Event, Self::Error>, Ok = Event, Error = Self::Error>
        + Send
        + 'static;
    type Error: Into<BoxError>;
    fn subscribe(&self, user_id: u64) -> Self::Stream;
}

impl PgNotify {
    pub async fn new(db_url: impl AsRef<str>) -> anyhow::Result<Self> {
        let users = Arc::new(DashMap::<u64, broadcast::Sender<Arc<AppEvent>>>::new());

        let mut listener = PgListener::connect(db_url.as_ref()).await?;
        listener.listen("chat_updated").await?;
        listener.listen("chat_message_created").await?;

        let mut stream = listener.into_stream();
        let cloned_users = users.clone();

        tokio::spawn(async move {
            while let Some(Ok(notification)) = stream.next().await {
                info!("Received notification: {:?}", notification);
                match Notification::load(notification.channel(), notification.payload()) {
                    Ok(notification) => {
                        info!("user_ids: {:?}", notification.user_ids);
                        for user_id in notification.user_ids {
                            if let Some(tx) = cloned_users.get(&(user_id as u64)) {
                                info!("sending notification to user: {}", user_id);
                                if let Err(err) = tx.send(notification.event.clone()) {
                                    warn!(
                                        "failed to send notification to user: {}, err: {:?}",
                                        user_id, err
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("failed to load notification: {:?}", e);
                    }
                }
            }
        });

        Ok(Self { users })
    }
}

impl Listener for PgNotify {
    type Stream = impl TryStream<Item = Result<Event, Self::Error>, Ok = Event, Error = Self::Error>
        + Send
        + 'static;
    type Error = Infallible;

    #[instrument(skip(self))]
    fn subscribe(&self, user_id: u64) -> Self::Stream {
        let rx = if let Some(tx) = self.users.get(&user_id) {
            tx.subscribe()
        } else {
            let (tx, rx) = broadcast::channel(CHANNEL_CAPACITY);
            info!("insert user: {user_id}");
            self.users.insert(user_id, tx);
            rx
        };
        info!("user {user_id} subscribed");

        BroadcastStream::new(rx).filter_map(|e| {
            e.ok().map(|e| {
                let name = match e.as_ref() {
                    AppEvent::NewChat(_) => "NewChat",
                    AppEvent::AddToChat(_) => "AddToChat",
                    AppEvent::RemoveFromChat(_) => "RemoveFromChat",
                    AppEvent::NewMessage(_) => "NewMessage",
                };
                let v = serde_json::to_string(&e).expect("Failed to serialize event");
                Ok::<_, Infallible>(Event::default().data(v).event(name))
            })
        })
    }
}

impl Notification {
    pub(crate) fn load(channel: &str, payload: &str) -> anyhow::Result<Self> {
        match channel {
            "chat_updated" => {
                let payload: ChatUpdated = serde_json::from_str(payload)?;
                let user_ids =
                    get_affected_chat_user_ids(payload.old.as_ref(), payload.new.as_ref());
                let event = match payload.op.as_str() {
                    "INSERT" => AppEvent::NewChat(payload.new.expect("new should exist")),
                    "UPDATE" => AppEvent::AddToChat(payload.new.expect("new should exist")),
                    "DELETE" => AppEvent::RemoveFromChat(payload.old.expect("old should exist")),
                    _ => return Err(anyhow::anyhow!("Invalid operation")),
                };
                Ok(Self {
                    user_ids,
                    event: Arc::new(event),
                })
            }
            "chat_message_created" => {
                let payload: ChatMessageCreated = serde_json::from_str(payload)?;
                let user_ids = payload.members.iter().cloned().collect();
                Ok(Self {
                    user_ids,
                    event: Arc::new(AppEvent::NewMessage(payload.message)),
                })
            }
            _ => Err(anyhow::anyhow!("Invalid notification type")),
        }
    }
}

fn get_affected_chat_user_ids(old: Option<&Chat>, new: Option<&Chat>) -> HashSet<i64> {
    match (old, new) {
        (Some(old), Some(new)) => {
            // diff old/new members, if identical, no need to notify, otherwise notify the union of both
            let old_user_ids: HashSet<_> = old.members.iter().cloned().collect();
            let new_user_ids: HashSet<_> = new.members.iter().cloned().collect();
            if old_user_ids == new_user_ids {
                HashSet::new()
            } else {
                old_user_ids.union(&new_user_ids).copied().collect()
            }
        }
        (Some(old), None) => old.members.iter().cloned().collect(),
        (None, Some(new)) => new.members.iter().cloned().collect(),
        _ => HashSet::new(),
    }
}
