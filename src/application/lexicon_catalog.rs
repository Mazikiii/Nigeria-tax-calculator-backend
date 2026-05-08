use crate::domain::entities::{LexiconFile, TransactionRole};
use crate::domain::lexicon::{DbError, LexiconRepository};
use crate::domain::services::categorizer::TransactionCategorizer;
use std::sync::Arc;
use tokio::sync::RwLock;

// this keeps the live lexicon in memory and lets db writes refresh the cache right away
pub struct LexiconCatalog<R> {
    repo: R,
    cache: Arc<RwLock<LexiconFile>>,
}

impl<R: LexiconRepository> LexiconCatalog<R> {
    // i load once here because the categorizer should read memory, not query postgres per statement
    pub async fn load(repo: R) -> Result<Self, DbError> {
        let lexicon = repo.load_lexicon().await?;
        Ok(Self {
            repo,
            cache: Arc::new(RwLock::new(lexicon)),
        })
    }

    // this gives callers a clean snapshot they can hand to the categorizer
    pub async fn snapshot(&self) -> LexiconFile {
        self.cache.read().await.clone()
    }

    // when a new rule is taught, i persist it first and then refresh the memory copy
    pub async fn upsert_rule(
        &self,
        category: &str,
        sub_category: &str,
        role: TransactionRole,
        keywords: Vec<String>,
        patterns: Vec<Vec<String>>,
        source: &str,
        confidence: Option<u32>,
    ) -> Result<(), DbError> {
        self.repo
            .upsert_rule(
                category,
                sub_category,
                role,
                keywords,
                patterns,
                source,
                confidence,
            )
            .await?;
        self.refresh().await
    }

    // i reload after writes so the next classification sees the updated words
    pub async fn refresh(&self) -> Result<(), DbError> {
        let lexicon = self.repo.load_lexicon().await?;
        *self.cache.write().await = lexicon;
        Ok(())
    }

    // this is the handoff point into the deterministic categorizer
    pub async fn categorizer(&self) -> TransactionCategorizer {
        TransactionCategorizer::from_lexicon(self.snapshot().await)
    }
}
