use crate::domain::entities::{ParserError, RawStatement};
use crate::domain::ports::StatementParser;
use async_trait::async_trait;
use regex::Regex;
use rust_decimal::Decimal;
use std::str::FromStr;

// the earlier lopdf version tried to become its own pdf engine
// that was useful for my learning, but it was the wrong production attempt
// the better approach is to let the extractor do pdf decoding and keep this file focused on statement shaping

// this is the smallest useful unit after extraction
// the old parser tried to work directly on pdf operators, but that made row handling too brittle
// this draft keeps the parser in a row-like mindset without pretending the pdf itself is already structured
#[derive(Debug, Clone)]
struct StatementDraft {
    date: String,
    narration_parts: Vec<String>,
    amount: Option<Decimal>,
    counterparty: Option<String>,
}

impl StatementDraft {
    // the old approach assumed one pdf line was one transaction
    // that breaks quickly, so the better approach is to keep a draft open until enough fields are collected
    fn new(date: String) -> Self {
        Self {
            date,
            narration_parts: Vec::new(),
            amount: None,
            counterparty: None,
        }
    }

    // the old code i wrote tried to finalize too early
    // this collects narration fragments so wrapped rows can still become one transaction
    fn push_narration(&mut self, fragment: &str) {
        let cleaned = fragment.trim();
        if cleaned.is_empty() {
            return;
        }

        self.narration_parts.push(cleaned.to_string());

        if self.counterparty.is_none() {
            self.counterparty = detect_counterparty(cleaned);
        }
    }

    // this only returns a statement when the row has enough usable data for the next layer
    fn finish(self) -> Option<RawStatement> {
        let amount = self.amount?;
        let narration = self.narration_parts.join(" ").trim().to_string();

        if narration.is_empty() {
            return None;
        }

        Some(RawStatement {
            date: self.date,
            narration,
            amount,
            counterparty: self.counterparty,
        })
    }
}

// this parser is intentionally plain
// the previous parser spent effort decoding raw pdf operators, which was the wrong place to spend complexity
// the production move is to use an extractor crate and reserve this file for transaction reconstruction
pub struct PdfParserAdapter;

impl PdfParserAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatementParser for PdfParserAdapter {
    async fn parse_pdf(&self, data: Vec<u8>) -> Result<Vec<RawStatement>, ParserError> {
        // the old approach walked pdf operators manually and tried to rebuild layout from scratch
        // this version asks the extractor for text pages first, which gives us a smaller and more stable problem
        let pages = pdf_extract::extract_text_from_mem_by_pages(&data)
            .map_err(|_| ParserError::FileError)?;

        let mut statements = Vec::new();

        for page_text in pages {
            let lines = normalize_page_text(&page_text);
            let page_statements = parse_page_lines(&lines);
            statements.extend(page_statements);
        }

        if statements.is_empty() {
            return Err(ParserError::UnknownFormat);
        }

        Ok(statements)
    }
}

// this is where the parser becomes opinionated about normal bank statement text
// the old parser code i wrote tried to infer structure from coordinates alone
// the better approach is to normalize text first and only then decide which lines are real transactions
fn parse_page_lines(lines: &[String]) -> Vec<RawStatement> {
    let mut results = Vec::new();
    let mut current: Option<StatementDraft> = None;

    for line in lines {
        if is_noise_line(line) {
            continue;
        }

        if let Some((date, body)) = extract_date_and_body(line) {
            // the old logic would keep appending into a single giant buffer
            // the better approach is to close the previous draft when a new dated row starts
            if let Some(previous) = current.take().and_then(StatementDraft::finish) {
                results.push(previous);
            }

            let mut draft = StatementDraft::new(date);
            apply_body_to_draft(&mut draft, &body);
            current = Some(draft);
            continue;
        }

        // the old logic would treat every non-date line as noise
        // that loses wrapped narrations, so the better approach is to keep the current draft open
        if let Some(draft) = current.as_mut() {
            apply_continuation_line(draft, line);
        }
    }

    if let Some(last) = current.and_then(StatementDraft::finish) {
        results.push(last);
    }

    results
}

// this is the part that turns raw pdf text into tidy lines
// the extractor gives us structure, but it still needs normalization because pdf spacing is often messy
fn normalize_page_text(page_text: &str) -> Vec<String> {
    page_text
        .lines()
        .map(|line| line.replace('\u{00a0}', " "))
        .map(|line| collapse_whitespace(&line))
        .filter(|line| !line.is_empty())
        .collect()
}

// this strips repeated spaces so the parser can make line-based decisions without fighting layout noise
fn collapse_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

// a lot of extracted lines are page furniture, headers, or table labels
// the old parser would overfit to these and misread them as transactions
fn is_noise_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();

    if lower.starts_with("page ") || lower.contains("page ") && lower.contains("of") {
        return true;
    }

    let exact_hits = [
        "date",
        "details",
        "particulars",
        "narration",
        "description",
        "debit",
        "credit",
        "amount",
        "balance",
        "running balance",
        "available balance",
    ];

    // this checks whether the current line is exactly one of the known header or noise words
    // iterate the list then .any stops as soon as one match is found
    exact_hits.iter().any(|needle| lower == *needle)
}

// this pulls the date from the front of a line and gives back the rest of the text
// the old parser guessed positionally from raw coordinates, which was fragile across statement formats
// this version makes the date explicit first so the rest of the line can be treated as narration and amount data
fn extract_date_and_body(line: &str) -> Option<(String, String)> {
    let date_patterns = [
        r"^(\d{2}/\d{2}/\d{2,4})\s*(.*)$",
        r"^(\d{2}-\d{2}-\d{2,4})\s*(.*)$",
        r"^(\d{4}-\d{2}-\d{2})\s*(.*)$",
        r"^(\d{1,2}\s+[A-Za-z]{3,9}\s+\d{4})\s*(.*)$",
        r"^([A-Za-z]{3,9}\s+\d{1,2},\s+\d{4})\s*(.*)$",
    ];

    for pattern in date_patterns {
        let re = Regex::new(pattern).expect("valid date regex");
        if let Some(captures) = re.captures(line) {
            let date = captures.get(1)?.as_str().trim().to_string();
            let body = captures
                .get(2)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();
            return Some((date, body));
        }
    }

    None
}

// this line-level helper is where the actual transaction shape is extracted
// it strips the amount out last because many real statement lines mix narration and money in one row
fn apply_body_to_draft(draft: &mut StatementDraft, body: &str) {
    if body.is_empty() {
        return;
    }

    let (narration, amount, sign_hint) = split_amount_from_text(body);

    if !narration.is_empty() {
        draft.push_narration(&narration);
    }

    if let Some(amount) = amount {
        draft.amount = Some(apply_sign(amount, sign_hint, &narration));
    }
}

// continuation lines are common when the narration is long
// the old parser tried to solve this by reading raw pdf commands, which was too low level for production
fn apply_continuation_line(draft: &mut StatementDraft, line: &str) {
    let (narration, amount, sign_hint) = split_amount_from_text(line);

    if !narration.is_empty() {
        draft.push_narration(&narration);
    }

    if draft.amount.is_none() {
        if let Some(amount) = amount {
            draft.amount = Some(apply_sign(amount, sign_hint, &narration));
        }
    }
}

// this function is a tradeoff point
// the old parser i wrote assumed the amount had a stable x position, which is not true across all banks
// the better approach is to strip the last money-like token and keep the rest as narration
fn split_amount_from_text(text: &str) -> (String, Option<Decimal>, SignHint) {
    let amount_re =
        Regex::new(r"(?i)(\(?-?\d[\d,]*(?:\.\d{1,2})?\)?)(?:\s*(dr|cr|debit|credit))?\s*$")
            .expect("valid amount regex");

    if let Some(captures) = amount_re.captures(text) {
        let raw_amount = captures.get(1).map(|m| m.as_str()).unwrap_or("");
        let tail = captures.get(2).map(|m| m.as_str()).unwrap_or("");
        let amount = parse_decimal_token(raw_amount);
        let sign_hint = infer_sign_hint(raw_amount, tail, text);

        let narration = text
            .get(..captures.get(1).unwrap().start())
            .unwrap_or("")
            .trim()
            .trim_matches(|c: char| matches!(c, '-' | ':' | ',' | ';'))
            .to_string();

        return (narration, amount, sign_hint);
    }

    (text.trim().to_string(), None, SignHint::Unknown)
}

// this does the mechanical number cleanup
// i now clean the token and let explicit debit or credit hints decide the sign later
fn parse_decimal_token(raw: &str) -> Option<Decimal> {
    let cleaned = raw
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim_start_matches('-')
        .replace(',', "");

    if cleaned.is_empty() {
        return None;
    }

    Decimal::from_str(&cleaned).ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SignHint {
    Unknown,
    Negative,
    Positive,
}

// the extractor does not know if a token means debit or credit, but the row context can still hint at it
fn infer_sign_hint(raw_amount: &str, tail: &str, full_text: &str) -> SignHint {
    let lowered = full_text.to_ascii_lowercase();
    let tail_lower = tail.to_ascii_lowercase();

    if raw_amount.starts_with('(')
        || raw_amount.starts_with('-')
        || lowered.contains("debit")
        || tail_lower.contains("dr")
    {
        return SignHint::Negative;
    }

    if lowered.contains("credit") || tail_lower.contains("cr") {
        return SignHint::Positive;
    }

    SignHint::Unknown
}

// this applies the sign after parsing the amount token
// the old version mixed sign handling into the coordinate walk itself
// my new approach is to decide the sign after the token is isolated
fn apply_sign(amount: Decimal, sign_hint: SignHint, narration: &str) -> Decimal {
    match sign_hint {
        SignHint::Negative => -amount,
        SignHint::Positive => amount,
        SignHint::Unknown => {
            if narration.to_ascii_lowercase().contains("dr") {
                -amount
            } else {
                amount
            }
        }
    }
}

// a tiny counterparty heuristic is better than leaving the field empty when the line clearly gives us a recipient or sender
fn detect_counterparty(fragment: &str) -> Option<String> {
    let lowered = fragment.to_ascii_lowercase();

    for keyword in [" to ", " from ", " beneficiary ", " sender ", " receiver "] {
        if let Some(index) = lowered.find(keyword) {
            let start = index + keyword.len();
            let candidate = fragment[start..].trim();
            if !candidate.is_empty() {
                return Some(candidate.to_string());
            }
        }
    }

    None
}
