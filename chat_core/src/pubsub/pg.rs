use dashmap::DashMap;
use futures::{Stream, StreamExt};
use sqlx::postgres::PgListener;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn};

use crate::pubsub::{notification::Notification, AppMessage, Subscriber};

#[derive(Clone)]
pub struct PgSubscriber {
    users: Arc<DashMap<u64, broadcast::Sender<Arc<AppMessage>>>>,
}

impl PgSubscriber {
    #[allow(unused)]
    pub async fn new(db_url: impl AsRef<str>) -> anyhow::Result<Self> {
        let users = Arc::new(DashMap::<u64, broadcast::Sender<Arc<AppMessage>>>::new());
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
                                if let Err(err) = tx.send(Arc::new(AppMessage {
                                    user_id: user_id as u64,
                                    event: (*notification.event).clone(),
                                })) {
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

impl Subscriber for PgSubscriber {
    type Stream = impl Stream<Item = AppMessage> + Send + 'static;

    async fn subscribe(&self, user_id: u64) -> anyhow::Result<Self::Stream> {
        let rx = if let Some(tx) = self.users.get(&user_id) {
            tx.subscribe()
        } else {
            let (tx, rx) = broadcast::channel(100);
            self.users.insert(user_id, tx);
            rx
        };

        Ok(tokio_stream::wrappers::BroadcastStream::new(rx)
            .filter_map(move |result| async move { result.ok().map(|e| (*e).clone()) }))
    }
}

#[cfg(test)]
mod tests {
    use futures::pin_mut;
    use sqlx::Executor as _;
    use sqlx_db_tester::TestPg;

    use crate::{AppEvent, ChatType};

    use super::*;

    #[tokio::test]
    async fn test_pg_subscriber() -> anyhow::Result<()> {
        let tdb = get_test_pool(None).await;

        let subscriber = PgSubscriber::new(tdb.url()).await;
        assert!(subscriber.is_ok());

        let stream = subscriber?.subscribe(1).await?;

        let handle = tokio::spawn(async move {
            pin_mut!(stream);
            let message1 = stream.next().await.unwrap();
            let message2 = stream.next().await.unwrap();
            assert_eq!(message1.user_id, 1);
            if let AppEvent::NewChat(chat) = &message1.event {
                assert_eq!(chat.id, 5);
                assert_eq!(chat.members, vec![1, 2]);
                assert_eq!(chat.r#type, ChatType::Single);
            } else {
                panic!("Expected NewChat event");
            }
            assert_eq!(message2.user_id, 1);
            if let AppEvent::NewChat(chat) = &message2.event {
                assert_eq!(chat.id, 6);
                assert_eq!(chat.members, vec![1, 3, 4]);
                assert_eq!(chat.r#type, ChatType::Group);
            } else {
                panic!("Expected NewChat event");
            }
        });

        let pool = tdb.get_pool().await;
        let query = "INSERT INTO chats(ws_id, type, members) VALUES (1, 'single', '{1,2}'), (1, 'group', '{1,3,4}')";
        pool.execute(query)
            .await
            .expect("Failed to insert test chats");

        handle.await?;

        Ok(())
    }

    pub async fn get_test_pool(url: Option<&str>) -> TestPg {
        let url = url
            .map(|x| x.to_string())
            .unwrap_or("postgres://postgres:postgres@localhost:5432".to_string());

        let tdb = TestPg::new(url, std::path::Path::new("../migrations"));
        let pool = tdb.get_pool().await;

        // insert test records
        let sql = include_str!("../../../chat_server/asserts/test.sql").split(';');
        let mut ts = pool.begin().await.expect("begin transaction failed");
        for s in sql {
            if s.trim().is_empty() {
                continue;
            }
            ts.execute(s).await.expect("execute sql failed");
        }
        ts.commit().await.expect("commit transaction failed");

        tdb
    }
}
