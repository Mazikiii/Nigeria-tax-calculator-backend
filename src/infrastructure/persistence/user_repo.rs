use crate::domain::entities::User;
use crate::domain::ports::{DbError, UserRepository};

use sqlx::PgPool;

struct PostgresDb {
    pool: PgPool,
}

impl PostgresDb {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl UserRespository for PostgresDb {
    async fn create_user_with_password(
        &self,
        email: &str,
        password_hash: &str,
        entity_type: &str,
    ) -> Result<User, DbError> {
        // inset a new row in the database, for the first time
        let create_row = sqlx::query_as!(
            User,
            r#"INSERT INTO users (email, password_hash, auth_provider, entity_type)
                    VALUES ($1, $2, 'email', $3)
                    RETURNING *"#,
            email,
            password_hash,
            entity_type
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DbError::QueryError(e.to_string()))?;

        Ok(create_row)
    }

    async fn create_user_with_oauth(
        &self,
        email: &str,
        auth_provider: &str,
        provider_id: &str,
        entity_type: &str,
    ) -> Result<User, DbError> {
        let create_row = sqlx::query_as!(User, r#" INSERT INTO users (email, auth_provider, provider_id, entity_type) VALUES ($1,$2,$3,$4) RETURNING * "#, email, auth_provider, provider_id, entity_type)
            .fetch_one(&self.pool) // the query needs to go to a pool, and fetch exactly one row
            .await
            .map_err(|e| DbError::QueryError(e.to_string()))?;
        Ok(create_row)
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>, DbError> {
        let get_row = sqlx::query_as!(User, r#"SELECT * FROM users WHERE email = $1"#, email)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| DbError::QueryError(e.to_string()))?;

        Ok(get_row)
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<User>, DbError> {
        let get_row = sqlx::query_as!(User, r#"SELECT * FROM users WHERE id = $1"#, id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| DbError::QueryError(e.to_string()))?;

        Ok(get_row)
    }
}
