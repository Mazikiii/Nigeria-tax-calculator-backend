use crate::domain::entities::{ParsedTransaction, RawStatement};
use async_trait::async_trait;

#[async_trait]
trait StatementParser: Send + Sync {
    async fn parse_pdf(&self, data: Vec<u8>) -> Result<Vec<RawStatement>, parse_error>;
}

#[async_trait]
trait ProcessedStatementSaver: Send + Sync {
    // saves the processed statement to db
    async fn save_batch(&self, txs: Vec<ParsedTransaction>) -> Result<(), Err>;
}

#[async_trait]
trait UserRepository {
    async fn create_user_with_password(
        &self,
        email: &str,
        password_hash: &str,
        entity_type: &str,
    ) -> Result<User, DbError>;
    async fn create_user_with_oauth(
        &self,
        auth_provider: &str,
        provider_id: &str,
        entity_type: &str,
    ) -> Result<User, DbError>;
    async fn find_by_id(&self, id: &str) -> Result<Option<User>, DbError>;
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, DbError>;
}

#[async_trait]
trait TaxStateRepository {
    async fn get_taxstate(&self, id: &str, tax_year: u32) -> Result<UserTaxState, DbError>;
    async fn save_taxstate(
        &self,
        id: &str,
        tax_year: u32,
        state: UserTaxState,
    ) -> Result<(), DbError>;
}

// db error
// its important to identify things that could go wrong in the database first
#[derive(Debug)]
pub enum DbError {
    ConnectionError(String),
    NotFound,
    QueryError(String),
    Conflict(String),
}
