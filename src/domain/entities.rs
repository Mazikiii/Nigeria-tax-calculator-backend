use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Deserialize)]
pub struct LexiconFile {
    #[serde(flatten)]
    pub categories: HashMap<String, HashMap<String, CategoryRule>>,
}

#[derive(Debug, Deserialize)]
pub struct CategoryRule {
    pub keywords: Vec<String>,
    pub patterns: Vec<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct RawStatement {
    // after the pdf is parsed this are the field we analyze, it is raw
    pub date: String,
    pub narration: String,
    pub amount: Decimal,
    pub counterparty: Option<String>,
}

pub struct ParsedTransaction {
    pub amount: Decimal,
    pub narration: String,
    pub role: TransactionRole,
    pub confidence: u32,
    pub charges: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionRole {
    Income,      // standard taxable inflow (Salary)
    TaxExempt,   // inflows that is not taxable (Gifts etc)
    Relief,      // specific "tax Credit" categories (Rent relief - 20%)
    Deduction,   // statutory outflows (Pension, NHIS, NHF)
    BusinessExp, // For LLCs, allowable expenses (Bank charges, Salaries paid)
    Unknown,     // Vague narrations requiring user input
}

#[derive(Debug, Error)]
pub enum ParserError {
    #[error("failed ot read the pdf ")]
    FileError,
    #[error("this pdf formart is not recognized")]
    UnknownFormat,
}
