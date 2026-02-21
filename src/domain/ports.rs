use crate::domain::entities::{ParsedTransaction, RawStatement, User};
use async_trait::async_trait;

// whatever is going to be used to parse the pdf must use this interface
#[async_trait]
trait StatementParser: Send + Sync {
    async fn parse_pdf(&self, data: Vec<u8>) -> Result<Vec<RawStatement>, parse_error>;
}

#[async_trait]
trait ProcessedStatementSaver: Send + Sync {
    // saves the processed statement to db
    async fn save_batch(&self, txs: Vec<ParsedTransaction>) -> Result<(), Err>;
}

// whatever service is going to be used as database must use this interface
// the interface defines the operations that is needed for the app
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

    async fn hash_password(&self, pass: &str) -> Result<String, PasswordError>;
    async fn verify_password(&self, user: &User, pass: &str) -> Result<bool, PasswordError>;
}

// this is also some operations relating to the database that the app needs
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

// this will be the interface used for Auth services like jwt
// this interface includes the important things the app needs generated. access and refresh tokens and a fn that validates generated tokens
// secrets are passed but secrets and JWT tokens are not the same thing
// the secret is used for siging
#[aysnc_trait]
pub trait TokenService: Send + Sync {
    // needs to create and encode jwt tokens to know who the user is
    fn generate_access_token(&self, user: &User) -> Result<String, TokenError>;
    fn generate_refresh_token(&self, user: &User) -> Result<String, TokenError>;
    // needs to be decoded for middleware to actually do its work
    fn validate_access_token(&self, token: &str) -> Result<AccessTokenPayload, TokenError>;
    fn validate_refresh_token(&self, token: &str) -> Result<RefreshTokenPayload, TokenError>;
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

// token generation and validation token errors
#[derive(Debug)]
pub enum TokenError {
    InvalidToken,
    Expired,
    GenerationError(String),
}

// this error is for password verification and hashing
#[derive(Debug)]
pub enum PasswordError {
    VerificationFailed,
    HashingFailed(String),
}
