use crate::domain::entities::{LexiconFile, TransactionRole};
use async_trait::async_trait;
use thiserror::Error;

// this port keeps the domain blind to postgres while still letting the app load a live lexicon
#[async_trait]
pub trait LexiconRepository: Send + Sync {
    // i load the full nested lexicon once so the categorizer can work in memory
    async fn load_lexicon(&self) -> Result<LexiconFile, DbError>;

    // i write one rule back and the caller can refresh the in-memory cache after that
    async fn upsert_rule(
        &self,
        category: &str,
        sub_category: &str,
        role: TransactionRole,
        keywords: Vec<String>,
        patterns: Vec<Vec<String>>,
        source: &str,
        confidence: Option<u32>,
    ) -> Result<(), DbError>;
}

// lexicon loading can fail for data, query, or connection reasons
#[derive(Debug, Error)]
pub enum DbError {
    #[error("connection error: {0}")]
    ConnectionError(String),
    #[error("not found")]
    NotFound,
    #[error("query error: {0}")]
    QueryError(String),
    #[error("conflict: {0}")]
    Conflict(String),
}
