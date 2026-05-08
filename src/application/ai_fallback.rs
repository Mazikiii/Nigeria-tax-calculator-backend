use crate::domain::entities::{ParsedTransaction, RawStatement, TaxEntity, TransactionRole};
use crate::domain::lexicon::LexiconRepository;
use crate::domain::services::categorizer::TransactionCategorizer;
use crate::domain::services::gemini_classifier::{
    AiClassification, AiClassifierError, AiLexiconSuggestion, GeminiTransactionClassifier,
};
use crate::application::lexicon_catalog::LexiconCatalog;

// this carries the final parsed transaction and the ai note that produced it
pub struct AiFallbackResult {
    pub transaction: ParsedTransaction,
    pub ai: Option<AiClassification>,
}

// this keeps the deterministic engine and the ai engine in one place
pub struct TransactionAiPipeline {
    categorizer: TransactionCategorizer,
    gemini: GeminiTransactionClassifier,
}

impl TransactionAiPipeline {
    // i inject both engines here so the caller does not juggle them separately
    pub fn new(categorizer: TransactionCategorizer, gemini: GeminiTransactionClassifier) -> Self {
        Self {
            categorizer,
            gemini,
        }
    }

    pub async fn classify_raw(
        &self,
        raw: RawStatement,
        user_type: TaxEntity,
    ) -> Result<AiFallbackResult, AiClassifierError> {
        // first i let the deterministic rules try their best
        let parsed = self
            .categorizer
            .analyze_raw_statment(raw, user_type.clone());

        // if the rules already know the role i stop here
        if parsed.role != TransactionRole::Unknown {
            return Ok(AiFallbackResult {
                transaction: parsed,
                ai: None,
            });
        }

        // if the rules do not know it yet, gemini gets the last word
        let ai = self
            .gemini
            .classify_unknown(&parsed.narration, &parsed.amount.to_string(), user_type)
            .await?;

        // i keep the old narration and just swap in the ai role
        let transaction = ParsedTransaction {
            role: ai.role,
            confidence: ai.confidence,
            ..parsed
        };

        Ok(AiFallbackResult {
            transaction,
            ai: Some(ai),
        })
    }

    pub async fn classify_batch(
        &self,
        raws: Vec<RawStatement>,
        user_type: TaxEntity,
    ) -> Result<Vec<AiFallbackResult>, AiClassifierError> {
        // i keep this simple so each statement gets the same fallback path
        let mut output = Vec::with_capacity(raws.len());

        for raw in raws {
            output.push(self.classify_raw(raw, user_type.clone()).await?);
        }

        Ok(output)
    }

    // this path teaches the lexicon only when gemini gives a concrete answer
    pub async fn classify_raw_and_teach<R: LexiconRepository>(
        &self,
        raw: RawStatement,
        user_type: TaxEntity,
        catalog: &LexiconCatalog<R>,
    ) -> Result<AiFallbackResult, AiClassifierError> {
        // i run the same deterministic path first because the model should stay the last line of defence
        let parsed = self
            .categorizer
            .analyze_raw_statment(raw, user_type.clone());

        if parsed.role != TransactionRole::Unknown {
            return Ok(AiFallbackResult {
                transaction: parsed,
                ai: None,
            });
        }

        let ai = self
            .gemini
            .classify_unknown(&parsed.narration, &parsed.amount.to_string(), user_type)
            .await?;

        let transaction = ParsedTransaction {
            role: ai.role,
            confidence: ai.confidence,
            ..parsed
        };

        // unknown stays unknown, because there is nothing trustworthy to learn from
        if ai.role != TransactionRole::Unknown {
            let (category, keywords, patterns) = suggestion_to_lexicon_entry(&ai.lexicon_suggestion);
            let sub_category = learned_sub_category(ai.role);

            // i write the learned rule back here so the next run stops calling this unknown
            catalog
                .upsert_rule(
                    &category,
                    &sub_category,
                    ai.role,
                    keywords,
                    patterns,
                    "ai",
                    Some(ai.confidence),
                )
                .await
                .map_err(|e| AiClassifierError::RequestFailed(e.to_string()))?;
        }

        Ok(AiFallbackResult {
            transaction,
            ai: Some(ai),
        })
    }
}

pub fn suggestion_to_lexicon_entry(
    suggestion: &AiLexiconSuggestion,
) -> (String, Vec<String>, Vec<Vec<String>>) {
    // this keeps the lexicon write step separate from the ai call itself
    (
        suggestion.category.clone(),
        suggestion.keywords.clone(),
        suggestion.patterns.clone(),
    )
}

// i keep learned rules grouped by role so repeated ai discoveries merge instead of splitting apart
fn learned_sub_category(role: TransactionRole) -> String {
    match role {
        TransactionRole::Income => "AI_INCOME".to_string(),
        TransactionRole::BusinessExp => "AI_BUSINESS_EXP".to_string(),
        TransactionRole::Salary => "AI_SALARY".to_string(),
        TransactionRole::TaxExempt => "AI_TAX_EXEMPT".to_string(),
        TransactionRole::Utilities => "AI_UTILITIES".to_string(),
        TransactionRole::Rent => "AI_RENT".to_string(),
        TransactionRole::Relief => "AI_RELIEF".to_string(),
        TransactionRole::Deduction => "AI_DEDUCTION".to_string(),
        TransactionRole::PersonalExp => "AI_PERSONAL_EXP".to_string(),
        TransactionRole::Unknown => "AI_UNKNOWN".to_string(),
        TransactionRole::PassThrough => "AI_PASS_THROUGH".to_string(),
    }
}
