use rust_decimal::Decimal;

pub struct parsed_transaction {
    pub amount: Decimal,
    pub narration: String,
    pub role: transaction_role,
    pub confidence: u32,
    pub charges: Decimal,
}

enum transaction_role {
    Income,      // standard taxable inflow (Salary)
    TaxExempt,   // inflows that is not taxable (Gifts etc)
    Relief,      // specific "tax Credit" categories (Rent relief - 20%)
    Deduction,   // statutory outflows (Pension, NHIS, NHF)
    BusinessExp, // For LLCs, allowable expenses (Bank charges, Salaries paid)
    Unknown,     // Vague narrations requiring user input
}
