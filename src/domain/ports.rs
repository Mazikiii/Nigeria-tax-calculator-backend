use crate::domain::entities::{parsed_transaction, raw_statement};
use async_trait::async_trait;

#[async_trait]
trait statement_parser: send + sync {
    async fn parse_pdf(&self, data: Vec<u8>) -> Result<Vec<raw_statement>, parse_error>;
}

#[async_trait]
trait processed_statement_saver: send + sync {
    // saves the processed statement to db
    async fn save_batch(&self, txs: Vec<parsed_transaction>) -> Result<(), save_error>;
}
