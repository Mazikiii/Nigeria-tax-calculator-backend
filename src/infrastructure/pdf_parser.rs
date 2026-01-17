use crate::domain::entities::{ParserError, RawStatement};
use crate::domain::ports::StatementParser;
use async_trait::async_trait;
use lopdf::Document;
use lopdf::content::{Content, Operation};
use lopdf::dictionary;
use lopdf::{Document, Object, Stream};
use rust_decimal::Decimal;
use std::collections::BTreeMap;

// i need something that works as coordination to identify keywords in columns
// so to pinpoint the position and word i'll define field x, y and text
// THE GRID STRUCTURE

/// represent a single word/phrase found in the pdf with its exact position
#[derive(Debug, Clone)]
struct TextItem {
    text: String,
    x: f64,
    y: f64,
}

/// represent a detected Row in the visual table
#[derive(Debug)]
struct TableRow {
    y_position: f64,
    items: Vec<TextItem>,
}

/// stores the discovered layout rules for A specific file.
#[allow(dead_code)]

struct LayoutRules {
    date_col_x_start: f64,
    date_col_x_end: f64,
    narration_col_x_start: f64,
    narration_col_x_end: f64,
    // some statments differ, some have debit and credit column some have just amoount
    // some statments have counterparties in narration, some have a seperate column for narration like opay
    // knowing this, i have to use the Option enum, since there is diversity
    debit_col_x_start: Option<u64>,
    debit_col_x_end: Option<u64>,
    credit_col_x_start: Option<u64>,
    credit_col_x_end: Option<u64>,
    amount_col_x_start: Option<u64>,
    amount_col_x_end: Option<u64>,
    counterparty_col_x_start: Option<u64>,
    counterparty_col_x_end: Option<u64>,
}

// i need to to implement some sort of adapter

pub struct PdfParserAdapter;

#[async_trait]
impl StatementParser for PdfParserAdapter {
    async fn parse_pdf(&self, data: Vec<u8>) -> Result<Vec<RawStatement>, ParserError> {

        let doc = Document::load_mem(&data).map_err(|_| ParserError::FileError)?;


        let raw_items = self.extract_text_items(&doc)?;


        let rows = self.cluster_rows(raw_items);

        let layout = self.detect_layout(&rows)?;

        // Step 5: Extract Data (The "Cookie Cutter")
        let mut results = Vec::new();
        for row in rows {
            // Skip the header row itself
            if self.is_header_row(&row) {
                continue;
            }

            // Apply the layout rules to this row
            if let Some(stmt) = self.map_row_to_statement(&row, &layout) {
                results.push(stmt);
            }
        }

        Ok(results)
    }
}

// helpers that processes a pdf

impl PdfParserAdapter {
    /// learnt about BT, ET, Tm, Td, Tj, TJ
    /// look into `lopdf::content::Content`, `Operation`.
    fn extract_text_items(&self, _doc: &Document) -> Result<Vec<TextItem>, ParserError> {
        // when we go through the various tm td, tj and TJ we need to store the pairs
        let pdf_items = Vec::new();

        // i need to loop through the pages, what is a page dipected as?
        for (page_num, page_id) in doc.get_pages() {
            // now i'm in page what do i need to do?, a page contains
            // a lot of commands that i have learnt and i need to go
            //  through all the commands, so store the content first
            let page_content = doc
                .get_page_content(page_id)
                .map_err(|_| ParserError::Error);
            // now that i have the content i need to iterate over the said content
            // and store things properly
            // seems i had to parse the page content to streams first, so i work with the various Tm and other commands
            let page_content = Content::decode(&content).unwrap_or_default();

            // we need a storage for x and y axis
            let mut current_x = 0.0;
            let mut current_y = 0.0;
            for commands in page_content.operations.iter() {
                // im now in the content and need to match according to what i hit
                // if i hit a Tm i need to store it then if i hit a tj next it had to be mapped to the stored tm
                match commands.operator.as_str() {
                    "Tm" => {
                        if let (Ok(e), Ok(f)) =
                            (commands.operands[4].as_f64(), commands.operands[5].as_f64())
                        {
                            current_x = e;
                            current_y = f;
                        }
                    }
                    "Td" => {
                        if let (Ok(e), Ok(f)) =
                            (commands.operands[4].as_f64(), commands.operands[5].as_f64())
                        {
                            current_x = e;
                            current_y = f;
                        }
                    }
                    "Tj" => {
                        if let Ok(text_byte) = commands.operands(0).as_str() {
                            let text = String::from_utf8_lossy(text_byte).to_string();
                            pdf_items.push(TextItems {
                                text: text.trim().to_string(),
                                x: current_x,
                                y: current_y,
                            })
                        }
                    }
                    "TJ" => {
                        for words in &commands.operands {
                            if let Ok(text_byte) = words {
                                let text = String::from_ut8_lossy(text_byte).to_string();
                                pdf_items.push(TextItems {
                                    text: text.to_string(),
                                    x: current_x,
                                    y: current_y,
                                })
                            }
                        }
                    }

                    _ => {}
                }
            }
        }

        Ok(pdf_items)
    }

    ///  grouping items by Y-coordinate.
    fn cluster_rows(&self, _items: Vec<TextItem>) -> Vec<TableRow> {
        let mut rows: Vec<TableRow> = Vec::new(); // storing rows in vec
        let threshold = 3.0;

        // how would i sort the TextItems based on y coordinate
        // we can get it by looking at item.y
        // so want to use quick sort or selection sort?
        // since i can't use .sort() on floats(because of NaN) use .sort_by()
        // i use b.y vs a.y because i need it sort in decending order
        // based on this sorting i will iterate from the very top so no need for hardcoding y=800
        _items.sort_by(|a, b| b.y.partial_cmp(&a.y).unwrap_or(std::cmp::Ordering::Equal));

        for current_row in _items {
            let mut new_row = true;
            let last_row = row.last_mut(); // get the last item in the storage, we need it to compare

            if let Some(last_row) = last_row {
                if (last_row.y_position - current_row.y).abs() < threshold {
                    last_row.items.push(current_row); // rows is a collection of TextItems
                    new_row = false;
                }
            }

            if new_row {
                rows.push(
                    (TableRow {
                        y_position: current_row.y,
                        items: vec![current_row], // start the collection with this row (allocates a new vector for THIS row)
                    }),
                );
                new_row = true;
            }
        }

        // i arranged the rows by decending order because thats how a pdf flows in terms of
        // y axis, but i can not forget about the x axis, that is important
        // from the beginning of the pdf down x axis flows in asending order compared to y axis
        // i still have to sort that very collection based on the x-axis, this changes positioning
        for row in &mut rows {
            row.items
                .sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
        }
        rows
    }

    // need this helper function for detect_layout function because i need to know
    // where the previous column ended
    fn boundary_before(next_column_x: f64) -> f64 {
          if next_column_x > 20.0 {
              next_column_x - 10.0 // 10px buffer to avoid clipping the first letter
          } else {
              0.0
          }
    ///finding the headers.
    /// i work with the returned value of cluster_row function and identify the layout and fix it accordingly using the LayoutRule
    fn detect_layout(&self, _rows: &Vec<TableRow>) -> Result<LayoutRules, ParserError> {
        // okay so i want to search the first 40 rows
        // in the search there has to be a sequence of connection
        // since we have rows arranged, we have to check a row that has our four to five words, date, narration, amount etc
        for rows in _rows.iter().take(40) {
            // take first 40
            // based on how many of this are true we know what type of statement
            // i am working with
            let mut has_date = false;
            let mut has_amount_keyword = false;
            let mut has_counterparty = false;
            let mut has_credit = false;
            let mut has_debit = false;
            let mut has_narration = false;

            // storage for  coordinates
            let mut credit_x = 0.0;
            let mut debit_x = 0.0;
            let mut narr_x = 0.0;
            let mut countp_x = 0.0;
            let mut amount = 0.0;
            let mut date = 0.0;

            for words in &rows.items {
                let upper_word = words.text.to_uppercase();
                // what am i trying to do?
                // i want to see if a row contains 4 things(varies based on statement)
                if upper_word.contains("DATE") || upper_word.contains("VAL") {
                    has_date = true; // the row has a date
                    date = words.x; // allocate the x position
                } else if text.contains("REMARK")
                    || text.contains("DESC")
                    || text.contains("PARTICULARS")
                    || text.contains("NARRATION")
                {
                    has_narration = true;
                    narr_x = words.x;
                } else if text.contains("BENEFICIARY")
                    || text.contains("RECEIVER")
                    || text.contains("COUNTERPARTY")
                {
                    has_counterparty = true;
                    cp_x = words.x;
                } else if text.contains("DEBIT") || text.contains("WITHDRAWAL") {
                    debit_x = words.x;
                    has_debit = true;
                } else if text.contains("CREDIT") || text.contains("DEPOSIT") {
                    credit_x = item.x;
                    has_credit = true;
                } else if text == "AMOUNT" {
                    amount_x = words.x;
                    has_amount_keyword = true;
                }
            }

            // what now? if a row has date, narration and amount that is a header
            // also if theres date, narration, debit AND credit that is a header
            // also if theres date, narration, amount and counterparty that is a header
            // also if theres date, narration, counterparty,debit AND credit that is a header

            //need to confirm theres a money column

            // this covers all my cases
            if has_date && has_money_col && has_narration {
                // this must be a header
                // if this conditions are met what do i want?
                // i want to save that row because it is a header
                if has_debit && has_credit {
                    return layoutRules {};
                }
            }
        }
        Err(ParserError::UnknownFormat)
    }


    fn map_row_to_statement(&self, _row: &TableRow, _layout: &LayoutRules) -> Option<RawStatement> {
              None
    }

    fn is_header_row(&self, _row: &TableRow) -> bool {

        false
    }
}
