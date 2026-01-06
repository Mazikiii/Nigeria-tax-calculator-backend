// I need to convert a raw Statement(the type defined in the domain entites) to a parsed statement
// Parsed Statment is the major thing that i'm working with. Its is the product of initial analysis
use entities::{ParsedStatment, RawStatment, TransactionRole};
use rust_decimal::Decimal;

struct TransactionCategorizer {
    // we need to store the narration and the role
    lexicon: Hashmap<String, TransactionRole>,
    charge: Regex, // of type regex
}

impl TransactionCategorizer {
    fn analyze(&self, raw_statement: RawStatement) -> ParsedStatment {
        let mut role = TransactionRole::Unknown;
        let mut confidence = 0;
        let mut charges = dec!(0);

        if let some(get_charge) = stip_charge(&raw_statment) {
            charges = get_charge;
        }
    }

    fn get_role(&raw_statement: RawStatement) -> TransactionRole {
        match raw = raw_statment.narration {}
    }
}
