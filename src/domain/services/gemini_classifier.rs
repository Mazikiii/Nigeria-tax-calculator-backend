use crate::domain::entities::{TaxEntity, TransactionRole};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::time::Duration;
use thiserror::Error;

// the learned rule that can be written back to lexicon.json later
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiLexiconSuggestion {
    // this is the folder name in the lexicon json
    pub category: String,
    // these are the actual words gemini thinks we should remember
    pub keywords: Vec<String>,
    // these are the multi word shapes i can match against later
    pub patterns: Vec<Vec<String>>,
}

// this is the final answer i want back from gemini after validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiClassification {
    pub role: TransactionRole,
    pub confidence: u32,
    pub reason: String,
    // this is the suggestion i can use to teach the lexicon later
    pub lexicon_suggestion: AiLexiconSuggestion,
}

#[derive(Debug, Error)]
pub enum AiClassifierError {
    #[error("missing gemini api key")]
    MissingApiKey,
    #[error("failed to read tax law context: {0}")]
    TaxLawReadFailed(String),
    #[error("gemini request failed: {0}")]
    RequestFailed(String),
    #[error("gemini returned invalid json: {0}")]
    InvalidJson(String),
    #[error("gemini returned unsupported role: {0}")]
    UnsupportedRole(String),
}

// gemini wraps the response deeply, so i model the envelope first
#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

// one response can have many candidates, but i only need the first usable one
#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiContent>,
}

// candidate content is still split into parts, so this keeps the path readable
#[derive(Debug, Deserialize)]
struct GeminiContent {
    parts: Option<Vec<GeminiPart>>,
}

// each part may carry text or other content types later
#[derive(Debug, Deserialize)]
struct GeminiPart {
    text: Option<String>,
}

// this is the json shape i ask gemini to return for a single classification
#[derive(Debug, Deserialize)]
struct GeminiOutput {
    role: String,
    confidence: Option<u32>,
    reason: Option<String>,
    keywords: Option<Vec<String>>,
    patterns: Option<Vec<Vec<String>>>,
}

// this holds the api state so i do not rebuild clients for every call
pub struct GeminiTransactionClassifier {
    // http client for the real api call
    client: Client,
    model: String,
    // api key comes from env and stays out of the prompt
    api_key: String,
    // this is the tax law context that guides the model
    tax_law_context: String,
}

impl GeminiTransactionClassifier {
    // i read everything once here so classify calls stay simple
    pub fn from_env() -> Result<Self, AiClassifierError> {
        let api_key =
            std::env::var("GEMINI_API_KEY").map_err(|_| AiClassifierError::MissingApiKey)?;
        let model =
            std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.5-flash-lite".to_string());
        let law_path = std::env::var("TAX_LAW_CONTEXT_PATH")
            .unwrap_or_else(|_| "../Nigerias_2026_Tax_Laws_Information.md".to_string());
        // the law document is what keeps the model aligned with this tax engine
        let tax_law_context = fs::read_to_string(law_path)
            .map_err(|e| AiClassifierError::TaxLawReadFailed(e.to_string()))?;

        let client = Client::builder()
            .timeout(Duration::from_secs(25))
            .build()
            .map_err(|e| AiClassifierError::RequestFailed(e.to_string()))?;

        Ok(Self {
            client,
            model,
            api_key,
            tax_law_context,
        })
    }

    // this is the actual ai fallback path when the rule engine gives up
    pub async fn classify_unknown(
        &self,
        narration: &str,
        amount: &str,
        user_type: TaxEntity,
    ) -> Result<AiClassification, AiClassifierError> {
        // i build one strict prompt so the model does not wander off format
        let prompt = self.build_prompt(narration, amount, user_type);
        let body = serde_json::json!({
            "contents": [
                {
                    "parts": [{"text": prompt}]
                }
            ],
            "generationConfig": {
                "temperature": 0.1,
                "responseMimeType": "application/json"
            }
        });

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );

        // i send a plain request because the gemini api shape is stable enough for this layer
        let response = self
            .client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| AiClassifierError::RequestFailed(e.to_string()))?;

        // the raw payload is useful when the api rejects the request
        let status = response.status();
        let payload = response
            .text()
            .await
            .map_err(|e| AiClassifierError::RequestFailed(e.to_string()))?;

        if !status.is_success() {
            return Err(AiClassifierError::RequestFailed(payload));
        }

        // this is just the outer envelope, not the actual classification yet
        let parsed_response: GeminiResponse = serde_json::from_str(&payload)
            .map_err(|e| AiClassifierError::InvalidJson(e.to_string()))?;

        // gemini wraps the actual text inside candidates and parts, so i unwrap carefully
        let text = parsed_response
            .candidates
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.content)
            .and_then(|c| c.parts)
            .and_then(|p| p.into_iter().next())
            .and_then(|p| p.text)
            .ok_or_else(|| AiClassifierError::InvalidJson("missing response text".to_string()))?;

        // the model sometimes returns fenced json, so i strip that first
        let cleaned = cleanup_json_text(&text);
        let output: GeminiOutput = serde_json::from_str(&cleaned)
            .map_err(|e| AiClassifierError::InvalidJson(e.to_string()))?;

        // this keeps the ai answer inside the same role system the rest of the app uses
        let role = parse_role(&output.role)?;
        let confidence = output.confidence.unwrap_or(60).min(100);
        let reason = output
            .reason
            .unwrap_or_else(|| "gemini classification".to_string());

        // if gemini gives me weak suggestions i still want something reusable for learning
        let mut keywords = normalize_keywords(output.keywords.unwrap_or_default());
        let mut patterns = normalize_patterns(output.patterns.unwrap_or_default());

        if keywords.is_empty() {
            keywords.push(narration.to_uppercase());
        }
        if patterns.is_empty() {
            patterns.push(
                narration
                    .to_uppercase()
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|s| !s.is_empty())
                    .take(3)
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>(),
            );
        }

        Ok(AiClassification {
            role,
            confidence,
            reason,
            lexicon_suggestion: AiLexiconSuggestion {
                category: role_to_lexicon_category(role).to_string(),
                keywords,
                patterns,
            },
        })
    }

    // this prompt is where i force gemini to think like the same tax engine
    fn build_prompt(&self, narration: &str, amount: &str, user_type: TaxEntity) -> String {
        let entity = match user_type {
            TaxEntity::PIT => "PIT",
            TaxEntity::LLC => "LLC",
        };

        // the tax law context goes in the prompt so the model reasons from the same rules
        format!(
            "you are classifying nigerian bank transaction narrations for a tax engine.

your output must be strict json with this shape:
{{
  \"role\": \"Income|BusinessExp|Salary|TaxExempt|Utilities|Rent|Relief|Deduction|PersonalExp|PassThrough|Unknown\",
  \"confidence\": 0..100,
  \"reason\": \"short reason\",
  \"keywords\": [\"UPPERCASE WORD\"],
  \"patterns\": [[\"UPPER\",\"WORDS\"]]
}}

rules:
- use the tax law context below before deciding
- keep SCHOOL and TUITION under Relief when that fits the narration
- choose PassThrough only when funds are clearly received/held/remitted for a third party
- avoid Unknown unless genuinely ambiguous
- keywords and pattern tokens must be uppercase
- patterns should be short and reusable, not full sentences

entity_type: {entity}
amount_sign: {amount}
narration: {narration}

tax law context:
{laws}",
            laws = self.tax_law_context
        )
    }
}

// this strips code fences and stray whitespace from the model output
fn cleanup_json_text(raw: &str) -> String {
    raw.trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string()
}

// this keeps the ai output inside the exact role enum the engine already knows
fn parse_role(raw: &str) -> Result<TransactionRole, AiClassifierError> {
    // i only accept roles the engine already understands
    match raw.trim().to_uppercase().as_str() {
        "INCOME" => Ok(TransactionRole::Income),
        "BUSINESSEXP" | "BUSINESS_EXP" | "BUSINESS_EXPENSE" => Ok(TransactionRole::BusinessExp),
        "SALARY" => Ok(TransactionRole::Salary),
        "TAXEXEMPT" | "TAX_EXEMPT" => Ok(TransactionRole::TaxExempt),
        "UTILITIES" => Ok(TransactionRole::Utilities),
        "RENT" => Ok(TransactionRole::Rent),
        "RELIEF" => Ok(TransactionRole::Relief),
        "DEDUCTION" => Ok(TransactionRole::Deduction),
        "PERSONALEXP" | "PERSONAL_EXP" => Ok(TransactionRole::PersonalExp),
        "PASSTHROUGH" | "PASS_THROUGH" | "PASS-THROUGH" => Ok(TransactionRole::PassThrough),
        "UNKNOWN" => Ok(TransactionRole::Unknown),
        other => Err(AiClassifierError::UnsupportedRole(other.to_string())),
    }
}

// this tells me which top level lexicon bucket the new rule belongs to
fn role_to_lexicon_category(role: TransactionRole) -> &'static str {
    match role {
        TransactionRole::Income => "income",
        TransactionRole::BusinessExp => "business_exp",
        TransactionRole::Salary => "income",
        TransactionRole::TaxExempt => "tax_exempt",
        TransactionRole::Utilities => "business_exp",
        TransactionRole::Rent => "relief",
        TransactionRole::Relief => "relief",
        TransactionRole::Deduction => "deduction",
        TransactionRole::PersonalExp => "personal_exp",
        TransactionRole::PassThrough => "pass_through",
        TransactionRole::Unknown => "unknown",
    }
}

// i clean keywords so a future lexicon write does not store noisy duplicates
fn normalize_keywords(input: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut output = Vec::new();
    for keyword in input {
        let clean = keyword.trim().to_uppercase();
        if clean.is_empty() {
            continue;
        }
        if seen.insert(clean.clone()) {
            output.push(clean);
        }
    }
    output
}

// i clean patterns the same way so they stay reusable in the matcher
fn normalize_patterns(input: Vec<Vec<String>>) -> Vec<Vec<String>> {
    let mut output = Vec::new();
    for pattern in input {
        let normalized: Vec<String> = pattern
            .into_iter()
            .map(|w| w.trim().to_uppercase())
            .filter(|w| !w.is_empty())
            .collect();
        if !normalized.is_empty() {
            output.push(normalized);
        }
    }
    output
}
