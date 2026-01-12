// I need to convert a raw Statement(the type defined in the domain entites) to a parsed statement
// Parsed Statment is the major thing that i'm working with. Its is the product of initial analysis
use crate::domain::entities::{
    CategoryRule, LexiconFile, ParsedStatment, RawStatment, TransactionRole,
};
use regex::Regex;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::{HashMap, HashSet};
use std::fs;

// First i need to rule out whether the transaaction is a charge VAT or Stamp duties
// there are going to be about four layers in this logic
// first layer is a sentence checker, users narration is checked against
// patterns in json- this will use recursion in some sort of way
// second layer is the keyword layer, which compares a single word to a keyword hashmap
// third layer is the fuzzy/ misspells, eg salry is salary and a check is done
// fourth and final layer is the ai layer, where ai take the narration and tries to infer the category

// Internal struct to hold pattern logic in memory
#[derive(Debug, Clone)]
struct PatternRule {
    required_words: HashSet<String>,
    role: TransactionRole,
    confidence: u32,
}

struct TransactionCategorizer {
    // what are the fields the transaction categorizer should have?
    // the hashmap that has keywords another that has patterns stored and a charge of type regex
    keywords_container: HashMap<String, TransactionRole>,
    pattern_container: HashMap<String, Vec<PatternRule>>,
    charge: Regex,
}

impl TransactionCategorizer {
    // need a constructor that fills the containers with the right data
    fn new() -> Self {
        // to fill the containers we need the data in the json, need serde to parse after
        let json_to_string =
            fs::read_to_String("src/domain/lexicon.json").expect("Failed to read Json");

        // now use serde to parse it properly, but and arrange it
        // the structure is needed which was defined as LexiconFile in entities
        let arranged_string: LexiconFile =
            serde_json::from_str(json_to_string).expect("Failed to arrange string");

        // initialize the keyword and pattern containers
        let mut keywords_container = HashMap::new();
        let mut pattern_container = HashMap::new();

        // now fill the container with a loop based on the arranged string var
        for (category, sub_category) in arranged_string {
            // have to give each category a role
            // dynamically checking Sub-Category name first
            let role = match sub_category_name.as_str() {
                // sub categories
                // the subwords are uppercase in the json
                "RENT" => TransactionRole::Rent,
                "UTILITIES" => TransactionRole::Utilities,
                "SALARY" => TransactionRole::Salary,
                "SCHOOL" | "TUITION" => TransactionRole::Relief, // Education is still generic Relief
                _ => {
                    //if not in the above categories, check the below too
                    match category_key.as_str() {
                        // category
                        // the root words in the json is lowercase
                        "income" => TransactionRole::Income,
                        "business_exp" => TransactionRole::BusinessExp,
                        "tax_exempt" => TransactionRole::TaxExempt,
                        "deduction" => TransactionRole::Deduction,
                        "relief" => TransactionRole::Relief,
                        _ => TransactionRole::Unknown,
                    }
                }
            };

            if role == TransactionRole::Unknown {
                continue; // we actually do not need any unknown role in this iteration so skip it
            }

            // lets store key words and patterns
            for (_sub_cat, data) in sub_category {
                // for keywords
                for items in data.keywords {
                    keywords_container.insert(items.to_uppercase(), role);
                }

                // for patterns. Remember the patternRule struct
                for items in data.patterns {
                    let required_words = HashSet = items.iter().map(|s| s.to_uppercase()).collect();

                    if required_words.is_empty() {
                        continue;
                    }

                    let pattern = PatternRule {
                        required_words: required_words,
                        role: role,
                        confidence: 100,
                    };

                    // come back to this
                    // every single word in the patterns vec has to be allocated the pattern rule
                    // a single word can have like four patterns
                    for word in required_words {
                        pattern_container
                            .entry(word.clone())
                            .or_default()
                            .push(pattern.clone());
                    }
                }
            }
        }

        return Self {
            keywords_container,
            pattern_container,
            charge: Regex::new(r"(?i)(CHG|COMM|VAT|FEE|DUTY|LEVY|EMTL)[:\s]+([\d,]+\.?\d*)")
                .expect("Invalid regex"),
        };
    }

    // get charges function
    fn extract_charge(&self, narration: &str) -> Decimal {
        // .and_then(...)`: This says "If the previous step succeeded (found a regex match),
        // pass the result to this function.
        // If it failed, just skip this whole block."
        let mut charge_regex = self.charge;
        charge_regex
            .captures(narration)
            .and_then(|c| c[2].replace(',', "").parse::<Decimal>().ok())
            .unwrap_or(dec!(0))
    }

    // get keyword function
    fn analyze_keywords(&self, words: &HashSet<String>) -> Option<TransactionRole> {
        for word in words {
            if self.keywords_container.contains_key(word) {
                return Some(*self.keywords_container.get(word).unwrap());
            }
        }
        None
    }

    // get patterns function
    fn analyze_pattern(&self, words: &HashSet<String>) -> Option<TransactionRole> {
        for word_in_pile in words {
            // if the word exists
            if let Some(word) = self.pattern_container.get(word_in_pile) {
                for pattern in word {
                    if pattern.required_words.is_subset(words) {
                        Some(*pattern.role)
                    }
                }
            }
            None
        }
    }

    //fuzzy or misspell checker
    fn fuzzy_checker(&self, narration: &str) -> Option<TransactionRole> {
        let uppercase_narration = narration.to_uppercase();
        let size_of_narration = uppercase_narration.len();

        if size_of_narration < 2 {
            return None;
        }

        // If length difference is > 2, the edit distance is definitely > 4.
        // This skips 90% of comparisons immediately.
        for (keyword, role) in self.keyword_container {
            let length_of_keyword = keyword.len();
            //if the word difference is very large then its not something to consider
            // the misspell(input) and the actual word
            if (size_of_narration as i32 - length_of_keyword as i32).abs() > 4 {
                continue;
            }

            // "Salary" vs "Pallery"
            if size_of_narration == length_of_keyword {
                continue;
            }

            // the actual processing, compare the the misspell with an actual word
            let ratio = fuzzywuzzy::fuzz::ratio(&uppercase_narration, keyword);
            if ratio > 82 {
                // if comparism is pretty high return the role
                return Some(*role);
            }
        }
    }

    // Major logic, Based on all the defined functions
    pub fn analyze_raw_statment(
        &self,
        raw: RawStatement,
        user_type: TaxEntity,
    ) -> ParsedTransaction {
        // check if the transaction is a charge
        let check = self.charge(&raw.narration);

        // break down the narration of the transaction
        let narration_words: HashSet<String> = &raw
            .narration
            .to_uppercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        // layer by layer we check, first a pattern? no then keyword?
        let (mut role, confidence) = self.layer_processor(&narration_words, &raw.narration);

        // A lot of checks has to happen here

        match role {
            TransactionRole::Salary => {
                if raw.amount.is_sign_negative() {
                    match user_entity {
                        PIT => role = TransacationRole::PersonalExp,
                        LLC => role = TransactionRole::BusinessExp,
                    }
                }
            }

            TransactionRole::Utilities => {
                // Utilities are always Debits (Expenses).
                if raw.amount.is_sign_positive() {
                    role = TransactionRole::TaxExempt; // Refund
                }
            }

            TransactionRole::Rent => {
                if raw.amount.is_sign_negative() {
                    // Paying Rent
                    match user_entity {
                        // PIT: It needs to apply the special 20% Relief logic
                        TaxEntity::PIT => role = TransactionRole::Rent,

                        // LLC: It is just a standard operating expense
                        TaxEntity::LLC => role = TransactionRole::BusinessExp,
                    }
                } else {
                    // Receiving Rent (Positive)
                    role = TransactionRole::Income;
                }
            }

            _ => {} // any other roles behave normally
        }

        // if lets say the person pays salary from their personal account to driver or cleaner
        // it is has to be a debit(negative) and it is a personal expense, not under taxExempt
        if role == TransactionRole::Income && raw.amount.is_sign_negative() {
            role = TransactionRole::PersonalExp;
        }

        // this is for businesses."Reversal of PHCN Bill" (Credit) is not an expense
        if role == TransactionRole::BusinessExp && raw.amount.is_sign_positive() {
            role = TransactionRole::TaxExempt;
        }

        let parsed = ParsedTransaction {
            amount: raw.amount,
            narration: raw.narration,
            role: role,
            confidence: confidence,
            charges: check,
            date: raw.date,
        };
    }

    // layer by layer we check, first a pattern? no then keyword? fuzzy? ai last!
    // you could use coR here but that is overkill
    fn layer_processor(&self, words: &HashSet<String>, narration: &str) -> (TransactionRole, u32) {
        if let Some(process) = analyze_pattern(words) {
            return (process, 100);
        }

        if let Some(process) = analyze_keywords(words) {
            return (process, 100);
        }

        if let Some(process) = fuzzy_checker(narration) {
            return (process, 100);
        }

        // if all failes then transaction unknown, ai handle it
        (TransactionRole::unknown, 0)
    }
}
