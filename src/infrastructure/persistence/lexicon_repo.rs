use crate::domain::entities::{CategoryRule, LexiconFile, TransactionRole};
use crate::domain::lexicon::{DbError, LexiconRepository};
use async_trait::async_trait;
use serde_json::Value;
use sqlx::{PgPool, Row};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

// this keeps postgres behind one repository so the domain only sees a lexicon snapshot
pub struct PostgresLexiconRepository {
    pool: PgPool,
}

impl PostgresLexiconRepository {
    // i keep the pool here because the repository owns the database access path
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LexiconRepository for PostgresLexiconRepository {
    async fn load_lexicon(&self) -> Result<LexiconFile, DbError> {
        // i load categories first so each entry can be stitched back into the old nested shape
        let category_rows = sqlx::query("SELECT id, name FROM lexicon_categories ORDER BY name")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DbError::QueryError(e.to_string()))?;

        let mut categories_by_id = HashMap::new();
        for row in category_rows {
            let id: Uuid = row
                .try_get("id")
                .map_err(|e| DbError::QueryError(e.to_string()))?;
            let name: String = row
                .try_get("name")
                .map_err(|e| DbError::QueryError(e.to_string()))?;
            categories_by_id.insert(id, name);
        }

        // each entry row becomes one subcategory block in the nested lexicon map
        let entry_rows = sqlx::query(
            r#"
            SELECT category_id, sub_category, role, keywords, patterns
            FROM lexicon_entries
            WHERE is_active = TRUE
            ORDER BY sub_category
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DbError::QueryError(e.to_string()))?;

        let mut categories: HashMap<String, HashMap<String, CategoryRule>> = HashMap::new();

        for row in entry_rows {
            let category_id: Uuid = row
                .try_get("category_id")
                .map_err(|e| DbError::QueryError(e.to_string()))?;
            let category_name = categories_by_id.get(&category_id).cloned().ok_or_else(|| {
                DbError::QueryError("lexicon entry points to a missing category".to_string())
            })?;

            let sub_category: String = row
                .try_get("sub_category")
                .map_err(|e| DbError::QueryError(e.to_string()))?;
            let role_text: String = row
                .try_get("role")
                .map_err(|e| DbError::QueryError(e.to_string()))?;
            let keywords_value: Value = row
                .try_get("keywords")
                .map_err(|e| DbError::QueryError(e.to_string()))?;
            let patterns_value: Value = row
                .try_get("patterns")
                .map_err(|e| DbError::QueryError(e.to_string()))?;

            let keywords: Vec<String> = serde_json::from_value(keywords_value)
                .map_err(|e| DbError::QueryError(e.to_string()))?;
            let patterns: Vec<Vec<String>> = serde_json::from_value(patterns_value)
                .map_err(|e| DbError::QueryError(e.to_string()))?;
            let role = parse_role(&role_text)?;

            categories
                .entry(category_name)
                .or_default()
                .insert(
                    sub_category,
                    CategoryRule {
                        role: Some(role),
                        keywords,
                        patterns,
                    },
                );
        }

        Ok(LexiconFile { categories })
    }

    async fn upsert_rule(
        &self,
        category: &str,
        sub_category: &str,
        role: TransactionRole,
        keywords: Vec<String>,
        patterns: Vec<Vec<String>>,
        source: &str,
        confidence: Option<u32>,
    ) -> Result<(), DbError> {
        // i normalize the incoming rule so the db does not accumulate noisy duplicates
        let cleaned_keywords = normalize_keywords(keywords);
        let cleaned_patterns = normalize_patterns(patterns);
        let category_id = ensure_category(&self.pool, category).await?;

        let existing = sqlx::query(
            r#"
            SELECT keywords, patterns
            FROM lexicon_entries
            WHERE category_id = $1 AND sub_category = $2
            "#,
        )
        .bind(category_id)
        .bind(sub_category)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DbError::QueryError(e.to_string()))?;

        let mut merged_keywords = cleaned_keywords;
        let mut merged_patterns = cleaned_patterns;

        if let Some(row) = existing {
            let existing_keywords: Value = row
                .try_get("keywords")
                .map_err(|e| DbError::QueryError(e.to_string()))?;
            let existing_patterns: Value = row
                .try_get("patterns")
                .map_err(|e| DbError::QueryError(e.to_string()))?;

            let existing_keywords: Vec<String> = serde_json::from_value(existing_keywords)
                .map_err(|e| DbError::QueryError(e.to_string()))?;
            let existing_patterns: Vec<Vec<String>> = serde_json::from_value(existing_patterns)
                .map_err(|e| DbError::QueryError(e.to_string()))?;

            merged_keywords = merge_keywords(existing_keywords, merged_keywords);
            merged_patterns = merge_patterns(existing_patterns, merged_patterns);
        }

        // the row stays as one unit so the cache can refresh from a clean db snapshot later
        sqlx::query(
            r#"
            INSERT INTO lexicon_entries (
                category_id,
                sub_category,
                role,
                keywords,
                patterns,
                source,
                confidence,
                is_active,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, TRUE, NOW())
            ON CONFLICT (category_id, sub_category)
            DO UPDATE SET
                role = EXCLUDED.role,
                keywords = EXCLUDED.keywords,
                patterns = EXCLUDED.patterns,
                source = EXCLUDED.source,
                confidence = EXCLUDED.confidence,
                is_active = TRUE,
                updated_at = NOW()
            "#,
        )
        .bind(category_id)
        .bind(sub_category)
        .bind(role_to_db(role))
        .bind(serde_json::to_value(&merged_keywords).map_err(|e| DbError::QueryError(e.to_string()))?)
        .bind(serde_json::to_value(&merged_patterns).map_err(|e| DbError::QueryError(e.to_string()))?)
        .bind(source)
        .bind(confidence.map(|value| value as i16))
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::QueryError(e.to_string()))?;

        Ok(())
    }
}

// i create the category row lazily so writes do not depend on manual seeding order
async fn ensure_category(pool: &PgPool, category: &str) -> Result<Uuid, DbError> {
    let row = sqlx::query(
        r#"
        INSERT INTO lexicon_categories (name, updated_at)
        VALUES ($1, NOW())
        ON CONFLICT (name)
        DO UPDATE SET updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(category)
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::QueryError(e.to_string()))?;

    row.try_get("id")
        .map_err(|e| DbError::QueryError(e.to_string()))
}

// i keep the incoming words stable so the cache and the table do not drift apart
fn normalize_keywords(input: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut output = Vec::new();

    for keyword in input {
        let cleaned = keyword.trim().to_uppercase();
        if cleaned.is_empty() {
            continue;
        }

        if seen.insert(cleaned.clone()) {
            output.push(cleaned);
        }
    }

    output
}

// i keep the incoming word groups stable for the same reason as keywords
fn normalize_patterns(input: Vec<Vec<String>>) -> Vec<Vec<String>> {
    let mut output = Vec::new();

    for pattern in input {
        let cleaned: Vec<String> = pattern
            .into_iter()
            .map(|word| word.trim().to_uppercase())
            .filter(|word| !word.is_empty())
            .collect();

        if !cleaned.is_empty() {
            output.push(cleaned);
        }
    }

    output
}

// i merge both sides here so new learning extends the existing lexicon instead of replacing it
fn merge_keywords(existing: Vec<String>, incoming: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut output = Vec::new();

    for value in existing.into_iter().chain(incoming.into_iter()) {
        if seen.insert(value.clone()) {
            output.push(value);
        }
    }

    output
}

// i merge pattern groups by string form because the nested vectors need exact dedupe
fn merge_patterns(existing: Vec<Vec<String>>, incoming: Vec<Vec<String>>) -> Vec<Vec<String>> {
    let mut seen = HashSet::new();
    let mut output = Vec::new();

    for pattern in existing.into_iter().chain(incoming.into_iter()) {
        let signature = pattern.join("\u{1f}");
        if seen.insert(signature) {
            output.push(pattern);
        }
    }

    output
}

// this keeps the db role text aligned with the domain enum
fn role_to_db(role: TransactionRole) -> &'static str {
    match role {
        TransactionRole::Income => "Income",
        TransactionRole::BusinessExp => "BusinessExp",
        TransactionRole::Salary => "Salary",
        TransactionRole::TaxExempt => "TaxExempt",
        TransactionRole::Utilities => "Utilities",
        TransactionRole::Rent => "Rent",
        TransactionRole::Relief => "Relief",
        TransactionRole::Deduction => "Deduction",
        TransactionRole::PersonalExp => "PersonalExp",
        TransactionRole::Unknown => "Unknown",
        TransactionRole::PassThrough => "PassThrough",
    }
}

// i parse the stored role back into the domain enum so the cache keeps the exact classification
fn parse_role(raw: &str) -> Result<TransactionRole, DbError> {
    match raw {
        "Income" => Ok(TransactionRole::Income),
        "BusinessExp" => Ok(TransactionRole::BusinessExp),
        "Salary" => Ok(TransactionRole::Salary),
        "TaxExempt" => Ok(TransactionRole::TaxExempt),
        "Utilities" => Ok(TransactionRole::Utilities),
        "Rent" => Ok(TransactionRole::Rent),
        "Relief" => Ok(TransactionRole::Relief),
        "Deduction" => Ok(TransactionRole::Deduction),
        "PersonalExp" => Ok(TransactionRole::PersonalExp),
        "Unknown" => Ok(TransactionRole::Unknown),
        "PassThrough" => Ok(TransactionRole::PassThrough),
        other => Err(DbError::QueryError(format!("unsupported lexicon role: {other}"))),
    }
}
