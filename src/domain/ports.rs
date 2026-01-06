use crate::domain::entities::{ParsedTransaction, RawStatement};
use async_trait::async_trait;

#[async_trait]
trait StatementParser: Send + Sync {
    async fn parse_pdf(&self, data: Vec<u8>) -> Result<Vec<RawStatement>, parse_error>;
}

#[async_trait]
trait ProcessedStatementSaver: Send + Sync {
    // saves the processed statement to db
    async fn save_batch(&self, txs: Vec<ParsedTransaction>) -> Result<(), save_error>;
}
