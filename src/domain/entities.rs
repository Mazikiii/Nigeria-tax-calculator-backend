use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;
use thiserror::Error;

// this is for storing the processed json. Defined the hashmap according to the
// structure i want
#[derive(Debug, Deserialize)]
pub struct LexiconFile {
    #[serde(flatten)]
    pub categories: HashMap<String, HashMap<String, CategoryRule>>,
}

// this is for the arranged data from json, the subcatigory is either a keyword or pattern
#[derive(Debug, Deserialize)]
pub struct CategoryRule {
    pub keywords: Vec<String>,
    pub patterns: Vec<Vec<String>>,
}

// data gotten after the actual statement is parsed
#[derive(Debug, Clone)]
pub struct RawStatement {
    // after the pdf is parsed this are the field we analyze, it is raw
    pub date: String,
    pub narration: String,
    pub amount: Decimal,
    pub counterparty: Option<String>,
}

// the logic for catigorizer processes the raw statement to parsed, majorly to identify
// role, charges, and confidence
pub struct ParsedTransaction {
    pub amount: Decimal,
    pub narration: String,
    pub role: TransactionRole,
    pub confidence: u32,
    pub charges: Decimal,
    pub date: String,
}

// there are two major entities that have seperate logic
#[derive(Debug, Clone)]
pub enum TaxEntity {
    PIT,
    LLC,
}

// based on the tax law, users narration can fall under about 6 categories
// for both PIT and LLC
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionRole {
    Income,      // standard taxable inflow (Salary)
    TaxExempt,   // inflows that is not taxable (Gifts etc)
    Relief,      // specific "tax Credit" categories (Rent relief - 20%)
    Deduction,   // statutory outflows (Pension, NHIS, NHF)
    BusinessExp, // For LLCs, allowable expenses (Bank charges, Salaries paid)
    Unknown,     // Vague narrations requiring user input
}

// this is error handling for the pdf parser
#[derive(Debug, Error)]
pub enum ParserError {
    #[error("failed ot read the pdf ")]
    FileError,
    #[error("this pdf formart is not recognized")]
    UnknownFormat,
}
