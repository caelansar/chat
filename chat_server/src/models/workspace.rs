use crate::AppError;
use chat_core::{ChatUser, Workspace};
use sqlx::PgPool;

pub struct WorkspaceRepo;

impl WorkspaceRepo {
    pub async fn create(name: &str, user_id: u64, pool: &PgPool) -> Result<Workspace, AppError> {
        let ws = sqlx::query_as(
            r#"
        INSERT INTO workspaces (name, owner_id)
        VALUES ($1, $2)
        RETURNING id, name, owner_id, created_at
        "#,
        )
        .bind(name)
        .bind(user_id as i64)
        .fetch_one(pool)
        .await?;

        Ok(ws)
    }

    #[allow(dead_code)]
    pub async fn update_owner(
        ws_id: i64,
        owner_id: u64,
        pool: &PgPool,
    ) -> Result<Workspace, AppError> {
        // update owner_id in two cases 1) owner_id = 0 2) owner's ws_id = id
        let ws = sqlx::query_as(
            r#"
        UPDATE workspaces
        SET owner_id = $1
        WHERE id = $2 and (SELECT ws_id FROM users WHERE id = $1) = $2
        RETURNING id, name, owner_id, created_at
        "#,
        )
        .bind(owner_id as i64)
        .bind(ws_id)
        .fetch_one(pool)
        .await?;

        Ok(ws)
    }

    pub async fn find_by_name(name: &str, pool: &PgPool) -> Result<Option<Workspace>, AppError> {
        let ws = sqlx::query_as(
            r#"
        SELECT id, name, owner_id, created_at
        FROM workspaces
        WHERE name = $1
        "#,
        )
        .bind(name)
        .fetch_optional(pool)
        .await?;

        Ok(ws)
    }

    #[allow(dead_code)]
    pub async fn find_by_id(id: u64, pool: &PgPool) -> Result<Option<Workspace>, AppError> {
        let ws = sqlx::query_as(
            r#"
        SELECT id, name, owner_id, created_at
        FROM workspaces
        WHERE id = $1
        "#,
        )
        .bind(id as i64)
        .fetch_optional(pool)
        .await?;

        Ok(ws)
    }

    #[allow(dead_code)]
    pub async fn fetch_all_chat_users(id: u64, pool: &PgPool) -> Result<Vec<ChatUser>, AppError> {
        let users = sqlx::query_as(
            r#"
        SELECT id, fullname, email
        FROM users
        WHERE ws_id = $1 order by id
        "#,
        )
        .bind(id as i64)
        .fetch_all(pool)
        .await?;

        Ok(users)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::user::UserRepo;
    use crate::models::CreateUser;
    use crate::test_util::get_test_pool;
    use anyhow::{Ok, Result};

    #[tokio::test]
    async fn workspace_should_create_and_set_owner() -> Result<()> {
        let (_tdb, pool) = get_test_pool(None).await;

        let ws = WorkspaceRepo::create("test", 0, &pool).await.unwrap();

        let input = CreateUser::new(&ws.name, "Cae Chen", "cae.chen@cae.com", "123456");
        let user = UserRepo::create(&input, &pool).await.unwrap();

        assert_eq!(user.fullname, "Cae Chen");
        assert_eq!(user.email, "cae.chen@cae.com");

        assert_eq!(ws.name, "test");
        assert_eq!(user.ws_id, ws.id);

        let ws = WorkspaceRepo::update_owner(ws.id, user.id as _, &pool)
            .await
            .unwrap();

        assert_eq!(ws.owner_id, user.id);
        Ok(())
    }

    #[tokio::test]
    async fn workspace_should_find_by_name() -> Result<()> {
        let (_tdb, pool) = get_test_pool(None).await;

        let ws = WorkspaceRepo::find_by_name("cae", &pool).await?;

        assert_eq!(ws.unwrap().name, "cae");
        Ok(())
    }

    #[tokio::test]
    async fn workspace_should_fetch_all_chat_users() -> Result<()> {
        let (_tdb, pool) = get_test_pool(None).await;

        let users = WorkspaceRepo::fetch_all_chat_users(1, &pool).await?;
        assert_eq!(users.len(), 5);

        Ok(())
    }
}
