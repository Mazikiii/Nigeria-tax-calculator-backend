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

// after the calculator has gone through a collection of transaction its final result should be
pub struct CalculationResult {
    // Delta (means change) a statement causes
    pub taxable_income_delta: Decimal, // (Income - Expenses/Reliefs)
    pub total_credit_flow: Decimal,    // keeping track of the raw inflows
    pub is_valid_for_use: bool, // if the statement unknown transaction is greater than 30% it isn't reliable
    pub unknown_transactions: Vec<ParsedTransaction>,
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
    BusinessExp, // For LLCs, allowable expenses (Bank charges, Salaries paid)
    Salary,      // Income (if Credit), Expense (if Debit for LLC)
    TaxExempt,   // inflows and outflows that is not taxable (Gifts,loan repay etc)
    Utilities,   // Can be Expense (LLC) or Personal (PIT)
    Rent,        // Can be Relief (PIT) or Expense (LLC)
    Relief,      // specific "tax Credit" categories (Rent relief - 20%)
    Deduction,   // statutory outflows (Pension, NHIS, NHF)
    PersonalExp, // the tax man does not care about this
    Unknown,     // vague narrations requiring user input
}

// more like an uptodate tracker for each PIT users
pub struct PitTaxState {
    pub user_id: String,
    pub tax_year: u32, // this is to keep track of what year i am working with
    pub taxable_income_ytd: Decimal,
    pub rent_relief_used_ytd: Decimal, // i need to keep track of the rent relief applied per year
}

pub struct LLCTaxState {
    pub user_id: String,
    pub tax_year: u32,
    pub taxable_income_ytd: Decimal,
    pub business_expenses_ytd: Decimal,
    pub development_levy_ytd: Decimal,
}

pub enum UserTaxState {
    PIT(PitTaxState),
    LLC(LLCTaxState),
}

// need a constructor for the various tax statess,(this is allowed in entites)
impl LLCTaxState {
    fn new(user_id: String, tax_year: u32) -> Self {
        Self {
            user_id,
            tax_year,
            taxable_income_ytd: Decimal::ZERO,
            business_expenses_ytd: Decimal::ZERO,
            development_levy_ytd: Decimal::ZERO,
        }
    }
}

impl PitTaxState {
    pub fn new(user_id: String, tax_year: u32) -> Self {
        Self {
            user_id,
            tax_year,
            taxable_income_ytd: Decimal::ZERO,
            rent_relief_used_ytd: Decimal::ZERO,
            pension_deduction_ytd: Decimal::ZERO,
            nhis_deduction_ytd: Decimal::ZERO,
            nhf_deduction_ytd: Decimal::ZERO,
            life_insurance_ytd: Decimal::ZERO,
        }
    }
}

// this is error handling for the pdf parser
#[derive(Debug, Error)]
pub enum ParserError {
    #[error("failed ot read the pdf ")]
    FileError,
    #[error("this pdf formart is not recognized")]
    UnknownFormat,
}
