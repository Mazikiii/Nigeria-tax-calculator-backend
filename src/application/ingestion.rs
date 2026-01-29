// here is the orchestrator that connects the dot
// i need a function that processes a bunch of statements
// lets start with one function that handles the most base case
use crate::domain::entities::{
    LLCTaxState, PITaxState, ParsedTransaction, TaxEntity, UserTaxState,
};

use crate::domain::services::categorizer::TransactionCategorizer;
use crate::domain::tax_calculator::TaxCalculator;
use crate::infrastructure::pdf_parser::PdfParserAdapter;
use rust_decimal::Decimal;
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
    pub unknown_transactions: Vec<ParsedTransaction>,
    pub unknown_count: usize,
}

// so some transactions may possible have > 30% in unknowns
#[derive(Debug)]
pub struct InvalidStatement {
    pub statement_id: String,
    pub taxable_income_delta: Decimal,
    pub total_credit_flow: Decimal,
    pub unknown_transactions: Vec<ParsedTransaction>,
    pub unknown_count: usize,
}

pub enum SingleStatmentResult {
    ValidStatement,
    InvalidStatement,
}

// when a batch of pdfs are uploaded there a possiblility that some pdfs had alot
// of unknowns and some are okay so i need to identify them
pub struct BatchProcessed {
    // need an id for the batch
    // i need to store the list of okay statements
    // i need to store the list of not okay statemnts
    pub batch_id: String,
    pub valid_statments: Vec<StatementResult>,
    pub invalid_statements: Vec<InvalidStatement>,
}

// i need some sort of error type incase the statement processing fails
#[debug(Debug)]
pub enum ProcessorError {
    pdf_parsing_error(string),
}

//learn to construct structs
// since i'm doing something like a facade i need to think of a structure
// that embodies the processes happening, i want to Process a statement,
// that alone is a struct, i just define it values as the operations i want to perform
pub struct StatementProcessor {
    statment_parser: PdfParserAdapter,
    categorizer: TransactionCategorizer,
    statement_calculator: TaxCalculator,
}

impl StatementProcessor {
    //what are the inputs and outputs?
    // inputs : the data:statement, are they pit or llc and the userid and tax year
    // output :  i give the user a report to accept is the final thing before updating the bar
    // that is an option

    // constructor for my various StatementProcessor attributes, client passes them in
    pub fn new(
        pdf_parser: PdfParserAdapter,
        categorizer: TransactionCategorizer,
        statement_calculator: TaxCalculator,
    ) -> Self {
        Self {
            pdf_parser,
            categorizer,
            statement_calculator,
        }
    }

    pub async fn process_statement(
        &self,
        pdf_datas: Vec<u8>,
        user_id: String,
        tax_type: TaxEntity,
        tax_year: u32,
    ) -> Result<SingleStatementResult, ProcessingError> {
        //okay so i need to collect the inputs and associate them properly
        // the pdf data that is given goes into the statment parser
        // the tax_type is used for both pit/llc entities and used for categorizer
        // then year is also used for an entity

        let pdf_processor = self.statment_parser.parse_pdf(pdf_data);

        let pdf_categorizer = self
            .categorizer
            .analyze_batch_parallel(pdf_processor, tax_type.clone());

        let type_of_user = match tax_type {
            TaxEntity::PIT => UserTaxState::PIT(PitTaxState::new(user_id.clone(), tax_year)),
            TaxEntity::LLC => UserTaxState::LLC(LLCTaxState::new(user_id.clone(), tax_year)),
        };

        let (pdf_statement_calculator, user_state) = self
            .statement_calculator
            .calculate_statement(pdf_categorizer.clone(), &type_of_user);

        // i need an id for each process that happens
        let process_id = uuid::Uuid::new_v4().to_string();

        match pdf_statement_calculator.is_Valid {
            true => Ok(SingleStatementResult::ValidStatement {
                statement_id: process_id,
                taxable_income_delta: pdf_statement_calculator.taxable_income_delta,
                total_credit_flow: pdf_statement_calculator.total_credit_flow,
                unknown_transactions: pdf_statement_calculator.unknown_transactions,
                unknown_count: pdf_statement_calculator.unknown_transactions.len() + 1 as usize,
            }),

            false => OK(SingleStatementResult::InvalidStatement {
                statement_id: process_id,
                taxable_income_delta: pdf_statement_calculator.taxable_income_delta,
                total_credit_flow: pdf_statement_calculator.total_credit_flow,
                unknown_transactions: pdf_statement_calculator.unknown_transactions,
                unknown_count: pdf_statement_calculator.unknown_transactions.len() + 1 as usize,
            }),

            _ => Err(),
        }
    }

    // what is batch statement processor doing?
    // what are the inputs and output?
    // i want the batch processor to collect a bunch of pdfs
    // loop through the bunch and process one statement at a time
    // then store valid and invalid statements seperatly
    // then use the struct BatchProcessed
    //
    pub async fn batch_statement_processor(&self,)
}


// when i process statments i want the taxState too because i need that in the frontend
