use std::sync::Arc;

use dashmap::DashMap;
use futures::StreamExt as _;
use lapin::{
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicPublishOptions, BasicRejectOptions,
        ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions,
    },
    types::{AMQPValue, FieldTable},
    BasicProperties, Connection,
};
use tokio::sync::mpsc;
use tracing::{error, info};

use super::{AppMessage, Publisher, Subscriber};

/// A publisher & subscriber that publish & subscribe to events in a RabbitMQ queue.
pub struct RabbitMqPubSub {
    connection: Arc<Connection>,
    subscribers: Arc<DashMap<u64, Vec<mpsc::Sender<AppMessage>>>>,
}

impl RabbitMqPubSub {
    /// Create a new RabbitMQ publisher & subscriber.
    #[allow(unused)]
    pub async fn new(connection: Connection) -> anyhow::Result<Self> {
        let ret = Self {
            connection: Arc::new(connection),
            subscribers: Arc::new(DashMap::new()),
        };

        ret.init_queue().await?;
        ret.background_task().await?;
        Ok(ret)
    }

    /// Initialize the RabbitMQ queue.
    async fn init_queue(&self) -> anyhow::Result<()> {
        let declare_channel = self.connection.create_channel().await?;

        declare_channel
            .exchange_declare(
                "chat-dead-letter.exchange",
                lapin::ExchangeKind::Direct,
                ExchangeDeclareOptions::default(),
                FieldTable::default(),
            )
            .await?;
        declare_channel
            .queue_declare(
                "chat-dead-letter.queue",
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await?;
        declare_channel
            .queue_bind(
                "chat-dead-letter.queue",
                "chat-dead-letter.exchange",
                "",
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await?;

        declare_channel
            .exchange_declare(
                "chat.exchange",
                lapin::ExchangeKind::Direct,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await?;

        // The chat.queue is set up with a dead-letter configuration, so any messages that can't be
        // processed will be sent to the dead-letter exchange.
        let mut queue_field = FieldTable::default();
        queue_field.insert(
            "x-dead-letter-exchange".into(),
            AMQPValue::LongString("chat-dead-letter.exchange".into()),
        );
        declare_channel
            .queue_declare(
                "chat.queue",
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                queue_field,
            )
            .await?;
        declare_channel
            .queue_bind(
                "chat.queue",
                "chat.exchange",
                "",
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await?;
        declare_channel.close(0, "declare channel fineshed").await?;

        Ok(())
    }

    /// Background task to consume messages from the RabbitMQ queue.
    async fn background_task(&self) -> anyhow::Result<()> {
        let consumer_channel = self.connection.create_channel().await?;
        let mut consumer = consumer_channel
            .basic_consume(
                "chat.queue",
                "consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        let map = Arc::clone(&self.subscribers);

        tokio::spawn(async move {
            info!("listening to chat.queue");

            while let Some(Ok(delivery)) = consumer.next().await {
                match serde_json::from_slice::<AppMessage>(delivery.data.as_slice()) {
                    Ok(app_message) => {
                        info!("received message");
                        // if subscribers is found by corresponding user_id, send the message to these subscribers
                        let senders = map.get_mut(&app_message.user_id);
                        delivery.ack(BasicAckOptions::default()).await.unwrap();
                        if let Some(senders) = senders {
                            for sender in senders.iter() {
                                if let Err(e) = sender.send(app_message.clone()).await {
                                    error!("Failed to send message: {:?}", e);
                                }
                            }
                        }
                    }
                    Err(err) => {
                        error!("Reject {}", err);
                        delivery
                            .reject(BasicRejectOptions::default())
                            .await
                            .unwrap();
                    }
                }
            }
        });

        Ok(())
    }
}

impl Subscriber for RabbitMqPubSub {
    type Stream = impl futures::Stream<Item = AppMessage>;
    async fn subscribe(&self, user_id: u64) -> anyhow::Result<Self::Stream> {
        let (tx, rx) = mpsc::channel(100);
        self.subscribers.entry(user_id).or_default().push(tx);

        Ok(tokio_stream::wrappers::ReceiverStream::new(rx))
    }
}

impl Publisher for RabbitMqPubSub {
    async fn publish(&self, event: AppMessage) -> anyhow::Result<()> {
        let channel = self.connection.create_channel().await?;
        channel
            .basic_publish(
                "chat.exchange",
                "",
                BasicPublishOptions::default(),
                &serde_json::to_vec(&event)?,
                BasicProperties::default(),
            )
            .await?;
        channel.close(0, "publish channel finished").await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{pubsub::AppEvent, ChatType};

    use futures::pin_mut;
    use lapin::ConnectionProperties;
    use tracing::level_filters::LevelFilter;
    use tracing_subscriber::{
        fmt::Layer, layer::SubscriberExt, util::SubscriberInitExt, Layer as _,
    };

    use super::*;

    #[tokio::test]
    async fn test_publish() -> anyhow::Result<()> {
        let layer = Layer::new().with_filter(LevelFilter::INFO);
        tracing_subscriber::registry().with(layer).init();

        let pubsub = RabbitMqPubSub::new(
            Connection::connect("amqp://localhost:5672", ConnectionProperties::default()).await?,
        )
        .await?;

        let subscriber1 = pubsub.subscribe(1).await?;

        let subscriber2 = pubsub.subscribe(2).await?;

        let handle = tokio::spawn(async move {
            pin_mut!(subscriber1);
            pin_mut!(subscriber2);

            let message = subscriber1.next().await.unwrap();
            info!("user1 received {:?}", message);
            assert_eq!(1, message.user_id);

            let message = subscriber2.next().await.unwrap();
            info!("user2 received {:?}", message);
            assert_eq!(2, message.user_id);
        });

        info!("publishing message1");
        // this message should be routed to user 1
        pubsub
            .publish(AppMessage {
                user_id: 1,
                event: AppEvent::NewChat(crate::Chat {
                    id: 1,
                    ws_id: 1,
                    r#type: ChatType::Group,
                    members: vec![1, 2, 3],
                    created_at: chrono::Utc::now(),
                    name: Some("Test1".to_string()),
                }),
            })
            .await?;

        info!("publishing message2");
        // this message should be routed to user 2
        pubsub
            .publish(AppMessage {
                user_id: 2,
                event: AppEvent::NewChat(crate::Chat {
                    id: 2,
                    ws_id: 2,
                    r#type: ChatType::Group,
                    members: vec![2, 3, 4],
                    created_at: chrono::Utc::now(),
                    name: Some("Test2".to_string()),
                }),
            })
            .await?;

        handle.await?;
        // time::sleep(time::Duration::from_secs(1)).await;

        Ok(())
    }
}
