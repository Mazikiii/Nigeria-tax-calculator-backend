// I need to convert a raw Statement(the type defined in the domain entites) to a parsed statement
// Parsed Statment is the major thing that i'm working with. Its is the product of initial analysis
use entities::{CategoryRule, LexiconFile, ParsedStatment, RawStatment, TransactionRole};
use regex::Regex;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collection::{Hashmap, Hashset};
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
    keywords_container: Hashmap<String, TransactionRole>,
    pattern_container: Hashmap<String, Vec<PatternRule>>,
    charge: Regex,
}

impl TransactionCategorizer {
    // need a constructor that fills the containers with the right data
    fn new() -> self {
        // to fill the containers we need the data in the json, need serde to parse after
        let json_to_string =
            fs::read_to_String(src / domain / lexicon.json).expect("Failed to read Json");

        // now use serde to parse it properly, but and arrange it
        // the structure is needed which was defined as LexiconFile in entities
        let arranged_string: LexiconFile =
            serde_json::from_str(json_to_string).expect("Failed to arrange string");

        // initialize the keyword and pattern containers
        let mut keywords_container = Hashmap::new();
        let mut pattern_container = Hashmap::new();

        // now fill the container with a loop based on the arranged string var
        for (category, sub_category) in arranged_string {
            // have to give each category a role
            let role = match category.to_str() {
                "income" => TransactionRole::Income,
                "tax_exempt" => TransactionRole::TaxExempt,
                "relief" => TransactionRole::Relief,
                "deduction" => TransactionRole::Deduction,
                "business_exp" => TransactionRole::BusinessExp,
                _ => TransactionRole::Unknown, // this is for defensive programming
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
                    let required_words = Hashset = items.iter().map(|s| s.to_uppercase()).collect();

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

    fn extract_charge(&self, narration: &str) -> Decimal {
        // .and_then(...)`: This says "If the previous step succeeded (found a regex match),
        // pass the result to this function.
        // If it failed, just skip this whole block."
        self.charge
            .captures(narration)
            .and_then(|c| c[2].replace(',', "").parse::<Decimal>().ok())
            .unwrap_or(Dec!(0));
    }

    fn analyze_keywords(&self, words: &HashSet<String>) -> Option<TransactionRole> {
        for word in words {
            if self.keywords_container.contains_key(word) {
                return Some(self.keywords_container.get(word));
            }
        }
        None
    }

    // so first what does the self.pattern_container look like? <string, pattern rule>
    // a word and its various patterns
    // we check if the word exists in the hashmap then open a loop if it does
    // we chec
    fn analyze_pattern(&self, words: &HashSet<String>) -> Option<TransactionRole> {
        for word_in_pile in words {
            // if the word exists
            if let Some(word) = self.pattern_container.get(word_in_pile) {
                for pattern in word {
                    if pattern.required_words.is_subset(words) {
                        Some(pattern.role)
                    }
                }
            }
            None
        }
    }

    // now the actual thing
    pub fn analyze_raw_statment(&self, raw: RawStatement) -> ParsedTransaction {
        // check if the transaction is a charge
        let check = self.charge(&raw.narration);

        // break down the narration of the transaction
        let narration_words: Hashset<String> = &raw
            .narration
            .to_uppercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
    }
}
