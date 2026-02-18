use crate::domain::entities::{User, UserTaxState};
use crate::domain::port::{DbError, TaxStateRepository};
use sqlx::PgPool;

struct TaxState {
    pool: PgPool,
}

impl TaxState {
    async fn new(pool: PgPool) -> Self {
        Self(pool)
    }
}

impl TaxStateRepository for TaxState {
    async fn get_taxstate(&self, id: &str, tax_year: u32) -> Result<UserTaxState, DbError> {
        let retrieve_state = sqlx::query_as!(
            UserTaxState,
            r#"SELECT * FROM user_tax_states WHERE user_id = $1 AND tax_year = $2"#,
            id,
            tax_year
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::QueryError(e.to_string()))?;

        Ok(retrieve_state)
    }

    async fn save_taxstate(&self, id: &str, year: u32, state: UserTaxState) -> Result<(), DbError> {
        sqlx::query!(
            r#"INSERT INTO user_tax_states (user_id, tax_year, state_data) VALUES ($1,$2,$3) ON CONFLICT (user_id, tax_year) DO UPDATE SET state_data = $3, update_at = NOW()"#,
            id,
            tax_year,
            serde_json::to_value(&state)? // need to turn the value of this state to json, so serialize it with serde
        ).execute(&self.pool).await.map_err(|e| DbError::QueryError(e.to_string()))?;
    }
}
