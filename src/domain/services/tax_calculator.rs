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
    CalculationResult, ParsedTransaction, TransactionRole, UserTaxState,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

pub struct TaxCalculator;

impl TaxCalculator {
    pub fn new() -> Self {
        Self // even if the code is stateless, just included a constructor
    }

    // this checks the final yearly income against each band
    // only the amount inside a band gets that band's rate
    pub fn calculate_pit_tax(chargeable_income: Decimal) -> Decimal {
        if chargeable_income <= Decimal::ZERO {
            return Decimal::ZERO;
        }

        let bands = [
            (dec!(800000), dec!(0)),
            (dec!(2200000), dec!(0.15)),
            (dec!(9000000), dec!(0.18)),
            (dec!(13000000), dec!(0.21)),
            (dec!(25000000), dec!(0.23)),
        ];

        let mut remaining_income = chargeable_income;
        let mut tax = Decimal::ZERO;

        for (band_size, rate) in bands {
            if remaining_income <= Decimal::ZERO {
                break;
            }

            let taxable_slice = remaining_income.min(band_size);
            tax += taxable_slice * rate;
            remaining_income -= taxable_slice;
        }

        if remaining_income > Decimal::ZERO {
            tax += remaining_income * dec!(0.25);
        }

        tax
    }

    // small companies are not taxed under the current rule in the notes
    pub fn is_small_company(gross_turnover: Decimal, fixed_assets_value: Decimal) -> bool {
        gross_turnover <= dec!(100000000) && fixed_assets_value <= dec!(250000000)
    }

    pub fn calculate_llc_tax(
        taxable_profit: Decimal,
        gross_turnover: Decimal,
        fixed_assets_value: Decimal,
    ) -> Decimal {
        if taxable_profit <= Decimal::ZERO
            || Self::is_small_company(gross_turnover, fixed_assets_value)
        {
            return Decimal::ZERO;
        }

        taxable_profit * dec!(0.30)
    }

    pub fn calculate_development_levy(
        taxable_profit: Decimal,
        gross_turnover: Decimal,
        fixed_assets_value: Decimal,
    ) -> Decimal {
        if taxable_profit <= Decimal::ZERO
            || Self::is_small_company(gross_turnover, fixed_assets_value)
        {
            return Decimal::ZERO;
        }

        taxable_profit * dec!(0.04)
    }

    // identify possible inputs and output
    fn calculate_statement(
        &self,
        statement: Vec<ParsedTransaction>,
        user_state: &UserTaxState,
    ) -> (CalculationResult, UserTaxState) {
        let mut taxable_inc_delta = Decimal::ZERO;
        let mut total_credit_inflow = Decimal::ZERO;
        let mut unknown_trans: Vec<ParsedTransaction> = Vec::new();
        let total_trans_count = statement.len();

        let mut updated_state = user_state.clone();
        let previous_tax_payable = self.tax_payable_for_state(user_state);
        let previous_development_levy = self.development_levy_for_state(user_state);

        match &mut updated_state {
            UserTaxState::PIT(pit_state) => {
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
                            let remaining_relief = max_relief - pit_state.rent_relief_used_ytd;

                            // this makes sure rent relief does not pass the yearly cap
                            if remaining_relief > Decimal::ZERO {
                                let relief_to_apply = applicable_relief.min(remaining_relief);

                                taxable_inc_delta -= relief_to_apply;
                                pit_state.rent_relief_used_ytd += relief_to_apply;
                            }
                        }

                        TransactionRole::Deduction | TransactionRole::Relief => {
                            taxable_inc_delta -= txn.amount.abs();
                        }

                        _ => {}
                    }
                }

                pit_state.taxable_income_ytd += taxable_inc_delta;
            }

            UserTaxState::LLC(llc_state) => {
                for txn in &statement {
                    if txn.amount.is_sign_positive() {
                        total_credit_inflow += txn.amount.abs();
                    }

                    if txn.role == TransactionRole::Unknown {
                        unknown_trans.push(txn.clone());
                        continue;
                    }

                    match txn.role {
                        TransactionRole::Income => {
                            taxable_inc_delta += txn.amount;
                            if txn.amount.is_sign_positive() {
                                llc_state.gross_turnover_ytd += txn.amount.abs();
                            }
                        }

                        TransactionRole::BusinessExp | TransactionRole::Deduction => {
                            let expense = txn.amount.abs();
                            taxable_inc_delta -= expense;
                            llc_state.business_expenses_ytd += expense;
                        }

                        _ => {}
                    }
                }

                llc_state.taxable_income_ytd += taxable_inc_delta;
                llc_state.development_levy_ytd = Self::calculate_development_levy(
                    llc_state.taxable_income_ytd,
                    llc_state.gross_turnover_ytd,
                    llc_state.fixed_assets_value,
                );
            }
        }

        let unknown_count = unknown_trans.len();
        let unknown_percentage = if total_trans_count == 0 {
            0.0
        } else {
            (unknown_count as f64 / total_trans_count as f64) * 100.0
        };
        let is_valid = unknown_percentage < 30.0;
        let tax_payable_ytd = self.tax_payable_for_state(&updated_state);
        let development_levy_ytd = self.development_levy_for_state(&updated_state);
        let annual_taxable_income = self.taxable_income_for_state(&updated_state);

        (
            CalculationResult {
                taxable_income_delta: taxable_inc_delta,
                annual_taxable_income,
                tax_payable_ytd,
                tax_delta: tax_payable_ytd - previous_tax_payable,
                development_levy_ytd,
                development_levy_delta: development_levy_ytd - previous_development_levy,
                total_credit_flow: total_credit_inflow,
                is_valid_for_use: is_valid,
                unknown_transactions: unknown_trans,
            },
            updated_state,
        )
    }

    fn taxable_income_for_state(&self, user_state: &UserTaxState) -> Decimal {
        match user_state {
            UserTaxState::PIT(pit_state) => pit_state.taxable_income_ytd,
            UserTaxState::LLC(llc_state) => llc_state.taxable_income_ytd,
        }
    }

    fn tax_payable_for_state(&self, user_state: &UserTaxState) -> Decimal {
        match user_state {
            UserTaxState::PIT(pit_state) => Self::calculate_pit_tax(pit_state.taxable_income_ytd),
            UserTaxState::LLC(llc_state) => Self::calculate_llc_tax(
                llc_state.taxable_income_ytd,
                llc_state.gross_turnover_ytd,
                llc_state.fixed_assets_value,
            ),
        }
    }

    fn development_levy_for_state(&self, user_state: &UserTaxState) -> Decimal {
        match user_state {
            UserTaxState::PIT(_) => Decimal::ZERO,
            UserTaxState::LLC(llc_state) => Self::calculate_development_levy(
                llc_state.taxable_income_ytd,
                llc_state.gross_turnover_ytd,
                llc_state.fixed_assets_value,
            ),
        }
    }

    // okay so why am i doing this, we use userstate to monitor how to apply relief which is responsible
    // for the how the tax delta will move and the userstate holds other important things that
    // will be applied to the gauge. I make use of the state and update it in the calculate_statement fn
    // but what happens when a statement is rejected? the state it involved get invalid and should have never been added
    // there is a state in the app that the user has to reject or accept statement but we
    // need a mock data to work with at the point of preview i need the deltas to be correct
    // so i know if a user accepts a statement review then i have to recalculate
    // so i create two functions, one that doesn't return a delta at all only the CalculationResult
    // and another that does recalculation based on all accepted statement now we use the delta because
    // at the point of the user clicking on accept all statement is valid and they accept the report making the delta unchangable
    pub fn preview_calculation(
        &self,
        statement: Vec<ParsedTransaction>,
        user_state: &UserTaxState,
    ) -> CalculationResult {
        let (result, _) = self.calculate_statement(statement, user_state);
        result // notice what you're returning and you are throwing the delta away
    }

    pub fn finalize_calculation(
        &self,
        transactions: Vec<ParsedTransaction>,
        current_state: &UserTaxState,
    ) -> (CalculationResult, UserTaxState) {
        self.calculate_statement(transactions, current_state) // notice what you're returning and you are adding the delta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pit_tax_uses_progressive_bands() {
        let tax = TaxCalculator::calculate_pit_tax(dec!(30000000));

        assert_eq!(tax, dec!(5830000));
    }

    #[test]
    fn small_company_does_not_pay_llc_tax_or_development_levy() {
        let tax = TaxCalculator::calculate_llc_tax(dec!(30000000), dec!(90000000), dec!(200000000));
        let levy = TaxCalculator::calculate_development_levy(
            dec!(30000000),
            dec!(90000000),
            dec!(200000000),
        );

        assert_eq!(tax, Decimal::ZERO);
        assert_eq!(levy, Decimal::ZERO);
    }

    #[test]
    fn large_company_pays_tax_and_development_levy_on_profit() {
        let tax =
            TaxCalculator::calculate_llc_tax(dec!(30000000), dec!(120000000), dec!(200000000));
        let levy = TaxCalculator::calculate_development_levy(
            dec!(30000000),
            dec!(120000000),
            dec!(200000000),
        );

        assert_eq!(tax, dec!(9000000));
        assert_eq!(levy, dec!(1200000));
    }
}
