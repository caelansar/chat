use crate::AppError;

use crate::models::user::ChatUserRepo;
use chat_core::{Chat, ChatType};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateChat {
    pub name: Option<String>,
    pub members: Vec<i64>,
    pub public: bool,
}

#[derive(sqlx::FromRow, Debug)]
struct DeletedRow {
    id: i64,
}

pub struct ChatRepo;

#[allow(dead_code)]
impl ChatRepo {
    pub async fn create(input: CreateChat, ws_id: u64, pool: &PgPool) -> Result<Chat, AppError> {
        let len = input.members.len();
        if len < 2 {
            return Err(AppError::CreateChatError(
                "Chat must have at least 2 members".to_string(),
            ));
        }

        if len > 8 && input.name.is_none() {
            return Err(AppError::CreateChatError(
                "Group chat with more than 8 members must have a name".to_string(),
            ));
        }

        // verify if all members exist
        let users = ChatUserRepo::fetch_by_ids(&input.members, pool).await?;
        if users.len() != len {
            return Err(AppError::CreateChatError(
                "Some members do not exist".to_string(),
            ));
        }

        let chat_type = match (&input.name, len) {
            (None, 2) => ChatType::Single,
            (None, _) => ChatType::Group,
            (Some(_), _) => {
                if input.public {
                    ChatType::PublicChannel
                } else {
                    ChatType::PrivateChannel
                }
            }
        };
        let chat = sqlx::query_as(
            r#"
            INSERT INTO chats (ws_id, name, type, members)
            VALUES ($1, $2, $3, $4)
            RETURNING id, ws_id, name, type, members, created_at
            "#,
        )
        .bind(ws_id as i64)
        .bind(input.name)
        .bind(chat_type)
        .bind(input.members)
        .fetch_one(pool)
        .await?;

        Ok(chat)
    }

    pub async fn fetch_all(ws_id: u64, pool: &PgPool) -> Result<Vec<Chat>, AppError> {
        let chats = sqlx::query_as(
            r#"
            SELECT id, ws_id, name, type, members, created_at
            FROM chats
            WHERE ws_id = $1
            "#,
        )
        .bind(ws_id as i64)
        .fetch_all(pool)
        .await?;

        Ok(chats)
    }

    pub async fn get_by_id(id: u64, pool: &PgPool) -> Result<Option<Chat>, AppError> {
        let chat = sqlx::query_as(
            r#"
            SELECT id, ws_id, name, type, members, created_at
            FROM chats
            WHERE id = $1
            "#,
        )
        .bind(id as i64)
        .fetch_optional(pool)
        .await?;

        Ok(chat)
    }

    pub async fn delete_by_id(id: u64, pool: &PgPool) -> Result<i64, AppError> {
        let res: DeletedRow = sqlx::query_as(
            r#"
            DELETE
            FROM chats
            WHERE id = $1
            RETURNING id
            "#,
        )
        .bind(id as i64)
        .fetch_one(pool)
        .await?;

        Ok(res.id)
    }
}

#[cfg(test)]
impl CreateChat {
    pub fn new(name: &str, members: &[i64], public: bool) -> Self {
        let name = if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        };
        Self {
            name,
            members: members.to_vec(),
            public,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_util::get_test_pool;

    use super::*;

    #[tokio::test]
    async fn create_single_chat_should_work() {
        let (_tdb, pool) = get_test_pool(None).await;
        let input = CreateChat::new("", &[1, 2], false);
        let chat = ChatRepo::create(input, 1, &pool)
            .await
            .expect("create chat failed");
        assert_eq!(chat.ws_id, 1);
        assert_eq!(chat.members.len(), 2);
        assert_eq!(chat.r#type, ChatType::Single);
    }

    #[tokio::test]
    async fn create_public_named_chat_should_work() {
        let (_tdb, pool) = get_test_pool(None).await;
        let input = CreateChat::new("general", &[1, 2, 3], true);
        let chat = ChatRepo::create(input, 1, &pool)
            .await
            .expect("create chat failed");
        assert_eq!(chat.ws_id, 1);
        assert_eq!(chat.members.len(), 3);
        assert_eq!(chat.r#type, ChatType::PublicChannel);
    }

    #[tokio::test]
    async fn chat_get_by_id_should_work() {
        let (_tdb, pool) = get_test_pool(None).await;
        let chat = ChatRepo::get_by_id(1, &pool)
            .await
            .expect("get chat by id failed")
            .unwrap();

        assert_eq!(chat.id, 1);
        assert_eq!(chat.name.unwrap(), "general");
        assert_eq!(chat.r#type, ChatType::PublicChannel);
        assert_eq!(chat.ws_id, 1);
        assert_eq!(chat.members.len(), 5);

        let chat = ChatRepo::get_by_id(2, &pool)
            .await
            .expect("get chat by id failed")
            .unwrap();

        assert_eq!(chat.id, 2);
        assert_eq!(chat.r#type, ChatType::PrivateChannel);
        assert_eq!(chat.name.unwrap(), "private");
        assert_eq!(chat.ws_id, 1);
        assert_eq!(chat.members.len(), 3);
    }

    #[tokio::test]
    async fn chat_fetch_all_should_work() {
        let (_tdb, pool) = get_test_pool(None).await;
        let chats = ChatRepo::fetch_all(1, &pool)
            .await
            .expect("fetch all chats failed");

        assert_eq!(chats.len(), 4);
    }

    #[tokio::test]
    async fn chat_delete_should_work() {
        let (_tdb, pool) = get_test_pool(None).await;

        let id = ChatRepo::delete_by_id(2, &pool).await.unwrap();

        assert_eq!(id, 2);
    }
}
