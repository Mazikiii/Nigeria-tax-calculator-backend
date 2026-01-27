// i have completed the parsing of statment and
// categorizing of transactions
// the output of the categorizer is what i'll work with here
// what are the structs i need?
// i need the output of the categorizer
// i need calculationResult and possiblely TAXEntity
// for the sake of comparism i need Transaction roles too
// how do i confirm this code whether this code will need a state?
// if the code will need data to be stored so it can refer to in its operations
// and tax calculation doesn't, it just takes input, processes it and gives out something
use crate::domain::entities::{
    CalculationResult, ParsedTransaction, TaxEntity, TransactionRole, UserTaxState,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
pub struct TaxCalculator;

impl TaxCalculator {
    pub fn new() -> Self {
        Self // even if the code is stateless, just included a constructor
    }

    // identify possible inputs and output
    pub fn calculate_statement(
        &self,
        statement: Vec<ParsedTransaction>,
        user_state: &UserTaxState,
    ) -> (CalculationResult, UserTaxState) {
        let mut taxable_inc_delta = dec!(0);
        let mut total_credit_inflow = dec!(0);
        let mut unknown_trans: Vec<ParsedTransaction> = Vec::new();
        let total_trans_count = statement.len();
        let mut unknown_count = unknown_trans.len();
        let mut is_valid = true;

        match user_state {
            UserTaxState::PIT(pit_state) => {
                let mut updated_state = pit_state.clone();

                for txn in &statement {
                    if txn.role == TransactionRole::Unknown {
                        unknown_trans.push(txn.clone());
                        continue;
                    }

                    if txn.amount.is_sign_positive() {
                        total_credit_inflow += txn.amount.abs();
                    }

                    match txn.role {
                        TransactionRole::Income | TransactionRole::Salary => {
                            taxable_inc_delta += txn.amount;
                        }

                        TransactionRole::Rent => {
                            let max_relief = dec!(500000);
                            let rent_paid = txn.amount.abs();
                            let applicable_relief = rent_paid * dec!(0.20);
                            let remaining_relief = max_relief - updated_pit.rent_relief_used_ytd;

                            //  apply if there's room left
                            if remaining_relief > dec!(0) {
                                let relief_to_apply = applicable_relief.min(remaining_relief);

                                taxable_inc_delta -= relief_to_apply;
                                updated_pit.rent_relief_used_ytd += relief_to_apply;
                            }
                        }

                        TransactionRole::Deduction => {
                            taxable_inc_delta -= txn.amount.abs();
                        }

                        TransactionRole::Relief => {
                            taxable_inc_delta -= txn.amount.abs();
                        }

                        _ => {}
                    }
                }

                let unknown_percentage = (unknown_count as f64 / total_trans_count as f64) * 100.0;
                if unknown_percentage >= 30.0 {
                    is_valid = false;
                }

                updated_pit.taxable_income_ytd += taxable_inc_delta;
                return (
                    CalculationResult {
                        taxable_income_delta: taxable_inc_delta,
                        total_credit_flow: total_credit_inflow,
                        is_valid_for_use: is_valid,
                        unknown_transactions: unknown_trans,
                    },
                    UserTaxState::PIT(updated_state),
                );
            }

            UserTaxState::LLC(llc_state) => {
                let mut updated_state = llc_state.clone();
                for txn in &statement {
                    if txn.amount.is_sign_positive() {
                        total_credit_inflow += txn.amount;
                    }

                    if txn.role == TransactionRole::Unknown {
                        unknown_trans.push(txn.clone());
                    }

                    match txn.role {
                        TransactionRole::Income => {
                            taxable_inc_delta += txn.amount;
                        }

                        TransactionRole::BusinessExp => {
                            taxable_inc_delta -= txn.amount.abs();
                        }

                        TransactionRole::Deduction => {
                            taxable_inc_delta -= txn.amount.abs();
                        }

                        _ => {}
                    }
                }

                let unknown_percentage = (unknown_count as f64 / total_trans_count as f64) * 100.0;
                if unknown_percentage >= 30.0 {
                    is_valid = false;
                }
                updated_pit.taxable_income_ytd += taxable_inc_delta;
                return (
                    CalculationResult {
                        taxable_income_delta: taxable_inc_delta,
                        total_credit_flow: total_credit_inflow,
                        is_valid_for_use: is_valid,
                        unknown_transactions: unknown_trans,
                    },
                    UserTaxState::LLC(updated_state),
                );
            }
        }
    }
}
