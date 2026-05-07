// I need to convert a raw Statement(the type defined in the domain entites) to a parsed statement
// Parsed Statment is the major thing that i'm working with. Its is the product of initial analysis
use crate::domain::entities::{
    LexiconFile, ParsedTransaction, RawStatement, TaxEntity, TransactionRole,
};
use rayon::prelude::*;
use regex::Regex;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::{HashMap, HashSet};
use strsim::levenshtein;
// this library helps me to correct misspelled words (its works with a dictionary) but in our case our json converted to hashmap in keyword container
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

pub struct TransactionCategorizer {
    // what are the fields the transaction categorizer should have?
    // the hashmap that has keywords another that has patterns stored and a charge of type regex
    keywords_container: HashMap<String, TransactionRole>,
    pattern_container: HashMap<String, Vec<PatternRule>>,
    charge_regex: Regex,
    dictionary_words: Vec<String>,
}

impl TransactionCategorizer {
    // need a constructor that fills the containers with the right data
    pub fn new() -> Self {
        let json_to_string = include_str!("../lexicon.json");

        // now use serde to parse it properly, but and arrange it
        // the structure is needed which was defined as LexiconFile in entities
        let arranged_string: LexiconFile =
            serde_json::from_str(&json_to_string).expect("Failed to arrange string");

        // initialize the keyword and pattern containers
        let mut keywords_container = HashMap::new();
        let mut pattern_container: HashMap<String, Vec<PatternRule>> = HashMap::new();

        // now fill the container with a loop based on the arranged string var
        for (category, sub_category) in arranged_string.categories {
            // lets store key words and patterns
            for (sub_cat_name, data) in sub_category {
                // have to give each category a role
                // dynamically checking Sub-Category name first
                let role = match sub_cat_name.as_str() {
                    // sub categories
                    // the subwords are uppercase in the json
                    "RENT" => TransactionRole::Rent,
                    "UTILITIES" => TransactionRole::Utilities,
                    "SALARY" => TransactionRole::Salary,
                    "SCHOOL" | "TUITION" => TransactionRole::Relief, // Education is still generic Relief

                    _ => {
                        //if not in the above categories, check the below too
                        match category.as_str() {
                            // category
                            // the root words in the json is lowercase
                            "income" => TransactionRole::Income,
                            "business_exp" => TransactionRole::BusinessExp,
                            "tax_exempt" => TransactionRole::TaxExempt,
                            "deduction" => TransactionRole::Deduction,
                            "relief" => TransactionRole::Relief,
                            "personal_exp" => TransactionRole::PersonalExp,
                            "pass_through" => TransactionRole::PassThrough,
                            _ => TransactionRole::Unknown,
                        }
                    }
                };

                if role == TransactionRole::Unknown {
                    continue; // we actually do not need any unknown role in this iteration so skip it
                }

                // for keywords
                for items in data.keywords {
                    keywords_container.insert(items.to_uppercase(), role);
                }

                // for patterns. Remember the patternRule struct
                for items in data.patterns {
                    let required_words: HashSet<String> =
                        items.iter().map(|s| s.to_uppercase()).collect();

                    if required_words.is_empty() {
                        continue;
                    }

                    let pattern = PatternRule {
                        required_words: required_words.clone(),
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

        let mut dictionary_words: Vec<String> = keywords_container.keys().cloned().collect();
        dictionary_words.sort();

        return Self {
            keywords_container,
            pattern_container,
            charge_regex: Regex::new(r"(?i)(CHG|COMM|VAT|FEE|DUTY|LEVY|EMTL)[:\s]+([\d,]+\.?\d*)")
                .expect("Invalid regex"),
            dictionary_words,
        };
    }

    // get charges function
    fn extract_charge(&self, narration: &str) -> Decimal {
        // .and_then(...)`: This says "If the previous step succeeded (found a regex match),
        // pass the result to this function.
        // If it failed, just skip this whole block."
        self.charge_regex
            .captures(narration)
            .and_then(|c| c[2].replace(',', "").parse::<Decimal>().ok())
            .unwrap_or(dec!(0))
    }

    // get keyword function
    fn analyze_keywords(&self, words: &HashSet<String>) -> Option<TransactionRole> {
        let mut found_roles = HashSet::new();

        for word in words {
            if let Some(role) = self.keywords_container.get(word) {
                found_roles.insert(*role);
            }
        }

        if found_roles.len() == 1 {
            return Some(found_roles.drain().next().unwrap());
        } else {
            return None;
        }
    }

    // get patterns function
    fn analyze_pattern(&self, words: &HashSet<String>) -> Option<(TransactionRole, u32)> {
        for word_in_pile in words {
            // if the word exists
            if let Some(patterns_list) = self.pattern_container.get(word_in_pile) {
                for pattern in patterns_list {
                    if pattern.required_words.is_subset(words) {
                        return Some((pattern.role, pattern.confidence));
                    }
                }
            }
        }
        None
    }

    fn fuzzy_checker(&self, words: &HashSet<String>) -> HashSet<String> {
        // if the word is already valid no need to stress it
        // if not i find the closest word in my tax dictionary
        words
            .iter()
            .map(|word| {
                if self.keywords_container.contains_key(word) {
                    return word.clone();
                }

                self.dictionary_words
                    .iter()
                    .filter(|candidate| {
                        let length_gap = candidate.len().abs_diff(word.len());
                        length_gap <= 4
                    })
                    .min_by_key(|candidate| levenshtein(word, candidate))
                    .filter(|candidate| levenshtein(word, candidate) <= 2)
                    .cloned()
                    .unwrap_or_else(|| word.clone())
            })
            .collect()
    }

    fn refine_for_pit(&self, role: TransactionRole, amount: Decimal) -> TransactionRole {
        match role {
            // salary outflow is personal expense for individual
            TransactionRole::Salary if amount.is_sign_negative() => TransactionRole::PersonalExp,

            // Rent outflow is kept as Rent (for relief calc)
            TransactionRole::Rent if amount.is_sign_negative() => TransactionRole::Rent,

            // Rent inflow is income (Landlord)
            TransactionRole::Rent if amount.is_sign_positive() => TransactionRole::Income,

            // Income outflow is PersonalExp
            TransactionRole::Income if amount.is_sign_negative() => TransactionRole::PersonalExp,

            // Expense inflow is refund
            TransactionRole::Utilities | TransactionRole::BusinessExp
                if amount.is_sign_positive() =>
            {
                TransactionRole::TaxExempt
            }

            // if a PIT user receives money for something usually bought like groceries, treat as Income (Trading)
            TransactionRole::PersonalExp if amount.is_sign_positive() => TransactionRole::Income,

            _ => role,
        }
    }

    fn refine_for_llc(&self, role: TransactionRole, amount: Decimal) -> TransactionRole {
        match role {
            // Salary/Rent outflow is biz expense
            TransactionRole::Salary | TransactionRole::Rent if amount.is_sign_negative() => {
                TransactionRole::BusinessExp
            }

            // rent inflow is revenue
            TransactionRole::Rent if amount.is_sign_positive() => TransactionRole::Income,

            // utilities inflow is likely a refund
            TransactionRole::Utilities if amount.is_sign_positive() => TransactionRole::TaxExempt,

            // if an LLC receives money for PersonalExp like food items etc or BusinessExp like Fuel, it is Income
            TransactionRole::BusinessExp | TransactionRole::PersonalExp
                if amount.is_sign_positive() =>
            {
                TransactionRole::Income
            }

            _ => role,
        }
    }

    pub fn analyze_batch_parallel(
        &self,
        statements: Vec<RawStatement>,
        user_type: TaxEntity,
    ) -> Vec<ParsedTransaction> {
        statements
            .into_par_iter() // split the work across CPU cores
            .map(|raw| self.analyze_raw_statment(raw, user_type.clone()))
            .collect() // collect results in order
    }

    // ---------------------------
    // now the actual thing
    // ---------------------------
    pub fn analyze_raw_statment(
        &self,
        raw: RawStatement,
        user_type: TaxEntity,
    ) -> ParsedTransaction {
        // check if the transaction is a charge
        let check = self.extract_charge(&raw.narration);

        // break down the narration of the transaction
        let narration_words: HashSet<String> = raw
            .narration
            .to_uppercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        let corrected_words = self.fuzzy_checker(&narration_words);

        // layer by layer we check, first a pattern? no then keyword?
        let (base_role, confidence) = self.layer_processor(&corrected_words);

        // Refine role based on User Type (PIT vs LLC) and Direction
        let role = match user_type {
            TaxEntity::PIT => self.refine_for_pit(base_role, raw.amount),
            TaxEntity::LLC => self.refine_for_llc(base_role, raw.amount),
        };

        ParsedTransaction {
            amount: raw.amount,
            narration: raw.narration,
            role,
            confidence,
            charges: check,
            date: raw.date,
        }
    }

    // layer by layer we check, first a pattern? no then keyword? fuzzy? ai last!
    // you could use coR here but that is overkill
    fn layer_processor(&self, words: &HashSet<String>) -> (TransactionRole, u32) {
        if let Some((process, confidence)) = self.analyze_pattern(words) {
            return (process, confidence);
        }

        if let Some(process) = self.analyze_keywords(words) {
            return (process, 100);
        }

        // gemini can take over from the application layer when deterministic rules fail
        (TransactionRole::Unknown, 0)
    }
}
