// here is the orchestrator that connects the dot
// i need a function that processes a bunch of statements
// lets start with one function that handles the most base case
use crate::domain::entities::{
    CheckError, CheckerResult, LLCTaxState, PITaxState, ParsedTransaction, PdfFile, PdfStates,
    TaxEntity, UserTaxState,
};

use crate::domain::services::categorizer::TransactionCategorizer;
use crate::domain::tax_calculator::TaxCalculator;
use crate::infrastructure::pdf_decrypter::PdfEncryptionChecker;
use crate::infrastructure::pdf_parser::PdfParserAdapter;
use rust_decimal::Decimal;
use std::collections::HashMap;
// what entites would i need for the connections
// TaxEntity
// since this is a point of access for other parts look at what type of data a
// user is passing from the early stage
// and thats the user id, are they pit or llc and statment
// can you think of a creational or structural or behavioural pattern for this?
// facade, builder are the best fits

// this is for valid statments
#[derive(Debug, Clone)]
pub struct ValidStatement {
    // i need a the id for this
    // i need to know the tax year
    // i need to know the inflow and data for the delta
    // i need to know if there were unknown transactions, the count and the transactions
    pub statement_id: String,
    pub taxable_income_delta: Decimal,
    pub total_credit_flow: Decimal,
    pub all_transactions: Vec<ParsedTransaction>, // need this to recalculate later
    pub unknown_transactions: Vec<ParsedTransaction>,
    pub unknown_count: usize,
}

// so some transactions may possible have > 30% in unknowns
#[derive(Debug)]
pub struct InvalidStatement {
    pub statement_id: String,
    pub taxable_income_delta: Decimal,
    pub total_credit_flow: Decimal,
    pub total_transaction_count: usize, // need this so i calculate for against user skipped txns
    pub all_transactions: Vec<ParsedTransaction>, // need this to recalculate later
    pub unknown_transactions: Vec<ParsedTransaction>,
    pub unknown_count: usize,
}

// helper to get all invalid txns with their source statement info
pub struct IndexedTransaction {
    pub statement_id: String,
    pub transaction: ParsedTransaction,
    pub global_index: usize, // the index of that txn in the collection that stores alll invalid txn
}

pub enum SingleStatmentResult {
    Valid(ValidStatement),
    Invalid(InvalidStatement),
}

// i need a struct for BatchProcessing
// what should be contained?
// batch id, the batch should contain invalid or/and valid statments
// when a batch of pdfs are uploaded there a possiblility that some pdfs had alot
// of unknowns and some are okay so i need to identify them
pub struct BatchProcessed {
    // need an id for the batch
    // i need to store the list of okay statements
    // i need to store the list of not okay statemnts
    pub batch_id: String,
    pub valid_statments: Vec<ValidStatement>,
    pub invalid_statements: Vec<InvalidStatement>,
    pub updated_user_state: Option<UserTaxState>,
}

// i need some sort of error type incase the statement processing fails
#[derive(Debug)]
pub enum ProcessorError {
    pdf_parsing_error(String),
}

// because of what i did in tax calculator(preview and finalize calculator), i want an enum to specify mode
#[derive(Clone, Copy)]
pub enum ProcessorMode {
    Preview,
    Final,
}

pub struct TxnClarification {
    pub index: usize, // index from get_all_invalid_transactions output
    pub role: TransactionRole,
}

//learn to construct structs
// since i'm doing something like a facade i need to think of a structure
// that embodies the processes happening, i want to Process a statement,
// that alone is a struct, i just define it values as the operations i want to perform
pub struct StatementProcessor {
    statment_parser: PdfParserAdapter,
    categorizer: TransactionCategorizer,
    statement_calculator: TaxCalculator,
    pdf_decrypter: PdfEncryptionChecker,
}

impl StatementProcessor {
    //what are the inputs and outputs?
    // inputs : the data:statement, are they pit or llc and the userid and tax year
    // output :  i give the user a report to accept is the final thing before updating the bar
    // that is an option

    // constructor for my various StatementProcessor attributes, client passes them in
    pub fn new(
        pdf_parser: PdfParserAdapter,
        categorize: TransactionCategorizer,
        stat_calculator: TaxCalculator,
        pdf_decrypter: PdfEncryptionChecker,
    ) -> Self {
        Self {
            statment_parser: pdf_parser,
            categorizer: categorize,
            statement_calculator: stat_calculator,
            pdf_decrypter,
        }
    }

    pub async fn process_statement(
        &self,
        pdf_datas: Vec<u8>,
        user_id: String,
        tax_type: TaxEntity,
        tax_year: u32,
        mode: ProcessorMode,
        user_uptodate_state: &UserTaxState,
    ) -> Result<(SingleStatementResult, Option<UserTaxState>), ProcessingError> {
        //okay so i need to collect the inputs and associate them properly
        // the pdf data that is given goes into the statment parser
        // the tax_type is used for both pit/llc entities and used for categorizer
        // then year is also used for an entity

        let pdf_processor = self.statment_parser.parse_pdf(pdf_data).await;

        let pdf_categorizer = self
            .categorizer
            .analyze_batch_parallel(pdf_processor, tax_type.clone());

        let (pdf_statement_calculator, updated_status) = match mode {
            ProcessorMode::Preview => {
                let result = self
                    .statement_calculator
                    .preview_calculation(pdf_categorizer.clone(), &user_uptodate_state);
                (result, None)
            }
            ProcessorMode::Final => {
                let (result, state) = self
                    .statement_calculator
                    .finalize_calculation(pdf_categorizer.clone(), &user_uptodate_state);
                (result, Some(state))
            }
        };

        // i need an id for each process that happens
        let process_id = uuid::Uuid::new_v4().to_string();

        if pdf_statement_calculator.is_valid_for_use {
            Ok((
                SingleStatementResult::Valid(ValidStatement {
                    statement_id: process_id,
                    taxable_income_delta: pdf_statement_calculator.taxable_income_delta,
                    total_credit_flow: pdf_statement_calculator.total_credit_flow,
                    unknown_transactions: pdf_statement_calculator.unknown_transactions,
                    unknown_count: pdf_statement_calculator.unknown_transactions.len(),
                }),
                Some(updated_status),
            ))
        } else {
            Ok((
                SingleStatementResult::Invalid(InvalidStatement {
                    statement_id: process_id,
                    taxable_income_delta: pdf_statement_calculator.taxable_income_delta,
                    total_credit_flow: pdf_statement_calculator.total_credit_flow,
                    unknown_transactions: pdf_statement_calculator.unknown_transactions,
                    unknown_count: pdf_statement_calculator.unknown_transactions.len(),
                }),
                None,
            ))
        }
    }

    // what is batch statement processor doing?
    // what are the inputs and output?
    // i want the batch processor to collect a bunch of pdfs
    // loop through the bunch and process one statement at a time
    // then store valid and invalid statements seperatly
    // then use the struct BatchProcessed
    //
    pub async fn batch_statement_processor(
        &self,
        pdfs: Vec<Vec<u8>>,
        user_id: String,
        tax_type: TaxEntity,
        tax_year: u32,
        mode: ProcessorMode,
        user_state: &UserTaxState,
    ) -> Result<BatchProcessed, ProcessorError> {
        // i need to go through each pdf and pass it to process statement
        // i need to pass references or clones to that function
        // i need a storage for the valid and invalid statments
        // i need to create a batch id
        // i need a something that stores the overall state of all statements that was accepted
        let batch_id = uuid.Uuid.new_v4().to_string();
        let mut valid_statement_container: Vec<ValidStatement> = Vec::new();
        let mut invalid_statement_container: Vec<InvalidStatement> = Vec::new();

        let mut updated_state = user_state.clone();

        for pdf in pdfs {
            let (statment, maybe_updated_state) = self
                .process_statement(
                    pdf,
                    user_id.clone(),
                    tax_type.clone(),
                    tax_year,
                    mode,
                    &updated_state, // take note of why this var was created to begin with, so i save the output of the previous state outputed by calculate_statement
                )
                .await?;

            if let Some(update_s) = maybe_updated_status {
                // remember preview and finalize outputs
                updated_state = updated_s; // i always updating the state for the next statement to work with
            }

            if let SingleStatementResult::Valid(valid_s) = statment {
                valid_statement_container.push(valid_s)
            }

            if let SingleStatementResult::Invalid(invalid_s) = statment {
                invalid_statement_container.push(invalid_s)
            }
        }

        let there_is_state = match &mode {
            ProcessorMode::Preview => None,
            ProcessorMode::Final => Some(updated_state),
        };
        if invalid_statement_container.len() == 0 {
            Ok(BatchProcessed {
                batch_id,
                valid_statments: valid_statement_container,
                invalid_statements: invalid_statement_container,
                updated_user_state: Some(updated_state),
            })
        } else {
            Ok(BatchProcessed {
                batch_id,
                valid_statments: valid_statement_container,
                invalid_statements: invalid_statement_container,
                updated_user_state: there_is_state,
            })
        }
    }

    // giving the frontend all invalid txns to display
    pub fn get_all_invalid_transactions(&self, batch: &BatchProcessed) -> Vec<IndexedTransaction> {
        let mut result = Vec::new();
        let mut index = 0;

        for stmt in &batch.invalid_statements {
            for txn in &stmt.unknown_transactions {
                result.push(IndexedTransaction {
                    statement_id: stmt.statement_id.clone(),
                    transaction: txn.clone(),
                    global_index: index,
                });
                index += 1;
            }
        }

        result
    }

    // after user clarifies and skips, recalculate what's left
    pub async fn apply_clarifications(
        &self,
        batch: &mut BatchProcessed,
        clarifications: Vec<(usize, TransactionRole)>, // index and new role
        skipped_indices: Vec<usize>,                   // indices user clicked x on
        current_state: &UserTaxState,
    ) -> Result<(), ProcessorError> {
        // get indexed txns to know which statement owns which txn
        let indexed = self.get_all_invalid_transactions(batch);

        // count skips per statement
        let mut skips_per_statement: HashMap<String, usize> = HashMap::new();
        for idx in &skipped_indices {
            if let Some(item) = indexed.get(*idx) {
                *skips_per_statement
                    .entry(item.statement_id.clone())
                    .or_insert(0) += 1;
            }
        }

        // apply clarifications using mutable access
        let mut all_txns = Vec::new();
        for stmt in &mut batch.invalid_statements {
            for txn in &mut stmt.unknown_transactions {
                all_txns.push(txn);
            }
        }

        for (index, role) in clarifications {
            if let Some(txn) = all_txns.get_mut(index) {
                txn.role = role;
            }
        }

        // recalculate each statement
        let mut newly_valid = Vec::new();
        let mut still_invalid = Vec::new();

        for invalid_stmt in batch.invalid_statements.drain(..) {
            let skip_count = skips_per_statement
                .get(&invalid_stmt.statement_id)
                .unwrap_or(&0);
            let skip_percentage =
                (*skip_count as f64 / invalid_stmt.total_transaction_count as f64) * 100.0;

            // if too many skips, keep in invalid without processing
            if skip_percentage >= 30.0 {
                still_invalid.push(invalid_stmt);
                continue;
            }

            // recalculate with updated txns
            let calc_result = self
                .statement_calculator
                .preview_calculation(invalid_stmt.unknown_transactions.clone(), current_state);

            // check if now valid
            if calc_result.is_valid_for_use {
                newly_valid.push(ValidStatement {
                    statement_id: invalid_stmt.statement_id,
                    taxable_income_delta: calc_result.taxable_income_delta,
                    total_credit_flow: calc_result.total_credit_flow,
                    unknown_transactions: calc_result.unknown_transactions.clone(),
                    unknown_count: calc_result.unknown_transactions.len(),
                });
            } else {
                still_invalid.push(InvalidStatement {
                    statement_id: invalid_stmt.statement_id,
                    total_transaction_count: invalid_stmt.total_transaction_count,
                    taxable_income_delta: calc_result.taxable_income_delta,
                    total_credit_flow: calc_result.total_credit_flow,
                    unknown_transactions: calc_result.unknown_transactions.clone(),
                    unknown_count: calc_result.unknown_transactions.len(),
                });
            }
        }

        // update batch
        batch.valid_statements.extend(newly_valid);
        batch.invalid_statements = still_invalid;

        Ok(())
    }

    // when the user puts a bunch of pdfs i want to seperate the locked and unlocked ones
    // but if all are unlocked i want to move straight to processing
    // if not all are unlocked i want to show the users the locked ones so they put pass
    // i created a new struct because i need to track states of pdfs and know where they belong
    // i created the PdfFile struct that stores id, name of pdf and the actual data
    // i'll use that as input, a collection of it, and my output is a new struct
    // the struct contains items like lockedpdfs, unlockedpdfs, corrupted pdf
    // it is called PdfState
    pub async fn batch_pdf_checks(&self, pdfs: Vec<PdfFile>) -> PdfState {
        // okay so i need to go through each pdf and call the checkpdf function on each
        // based on the output of that function i categorize
        // i need storages to allocate back to pdfstate
        let mut locked_pdfs = Vec::new();
        let mut unlocked_pdfs = Vec::new();
        let mut corrupted_pdfs = Vec::new();

        for pdf in pdfs {
            // i need to pass the data to the check_pdf function
            let checked_pdf = self.pdf_decrypter.check_pdf(pdf.data)?;

            match checked_pdf {
                Ok(CheckedResult::LockedPdf(_)) => locked_pdfs.push(pdf),
                Ok(CheckedResult::UnlockedPdf(_)) => unlocked_pdfs.push(pdf),
                Err(CheckError::CorruptedPdf(msg)) => corrupted_pdfs.push(CorruptedPdfInfo {
                    id: pdf.id,
                    name: pdf.name,
                    error_msg: msg,
                }),
                _ => {}
            }
        }

        PdfState {
            locked_pdf: locked_pdfs,
            unlocked_pdf: unlocked_pdfs,
            corrupted_pdf: corrupted_pdfs,
        }
    }

    // whats
    pub async fn decrypt_pdfs(&self, )
}
