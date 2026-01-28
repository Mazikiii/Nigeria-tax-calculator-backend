use crate::domain::entities::{ParserError, RawStatement};
use crate::domain::ports::StatementParser;
use async_trait::async_trait;
use lopdf::content::{Content, Operation};
use lopdf::dictionary;
use lopdf::{Document, Object, Stream};
use rust_decimal::Decimal;

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
    debit_col_x_start: Option<f64>,
    debit_col_x_end: Option<f64>,
    credit_col_x_start: Option<f64>,
    credit_col_x_end: Option<f64>,
    amount_col_x_start: Option<f64>,
    amount_col_x_end: Option<f64>,
    counterparty_col_x_start: Option<f64>,
    counterparty_col_x_end: Option<f64>,
}

// particularly used for detect_layout function
#[derive(Debug, PartialEq, Copy, Clone)]
enum ColumnType {
    Date,
    Narration,
    Debit,
    Credit,
    Amount,
    Counterparty,
    Balance, //  track just so i know where the previous col ends
}
// need this because i want a list of columns position in the detect_layout function
struct Marker {
    x: f64,
    col_type: ColumnType,
}

// i need to to implement some sort of adapter
pub struct PdfParserAdapter;

impl PdfParserAdapter {
    pub fn new() -> Self {
        Self
    }
}
#[async_trait]
impl StatementParser for PdfParserAdapter {
    async fn parse_pdf(&self, data: &mut Vec<u8>) -> Result<Vec<RawStatement>, ParserError> {
        let doc = Document::load_mem(&data).map_err(|_| ParserError::FileError)?;

        let raw_items = self.extract_text_items(&doc)?;

        let rows = self.cluster_rows(raw_items);

        let (layout, header_y) = self.detect_layout(&rows)?;

        let mut results = Vec::new();
        for row in rows {
            // remember that in pdfs y starts from the highest number and decreases as it goes down
            // anything above the header is meta data thats useless to me rn
            if row.y_position >= header_y {
                continue;
            }

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
        let mut pdf_items = Vec::new();

        // i need to loop through the pages, what is a page dipected as?
        for (page_num, page_id) in _doc.get_pages() {
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
                            //  Td operator only has 2 operands (tx, ty) so it should be operands[0] and operands[1]
                            (commands.operands[0].as_f64(), commands.operands[1].as_f64())
                        {
                            current_x = e;
                            current_y = f;
                        }
                    }
                    "Tj" => {
                        if let Ok(text_byte) = commands.operands[0].as_str() {
                            let text = String::from_utf8_lossy(text_byte).to_string();
                            pdf_item.push(TextItems {
                                text: text.trim().to_string(),
                                x: current_x,
                                y: current_y,
                            })
                        }
                    }
                    "TJ" => {
                        for words in &commands.operands {
                            if let Ok(text_byte) = words.as_string() {
                                let text = String::from_ut8_lossy(text_byte).to_string();
                                pdf_item.push(TextItems {
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
    fn cluster_rows(&self, mut _items: Vec<TextItem>) -> Vec<TableRow> {
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
            let last_row = rows.last_mut(); // get the last item in the storage, we need it to compare

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

    ///finding the headers.
    /// i work with the returned value of cluster_row function and identify the layout and fix it accordingly using the LayoutRule
    fn detect_layout(&self, _rows: &Vec<TableRow>) -> Result<(LayoutRules, f64), ParserError> {
        let mut markers = Vec::new();

        // okay so i want to search the first 40 rows
        // in the search there has to be a sequence of connection
        // since we have rows arranged, we have to check a row that has our four to five words, date, narration, amount etc
        for rows in _rows.iter().take(40) {
            for words in &rows.items {
                let text = words.text.to_uppercase();
                // what am i trying to do?
                // i want to see if a row contains 4 things(varies based on statement)

                if text.contains("DATE") || text.contains("VAL") {
                    markers.push(Marker {
                        x: words.x,
                        col_type: ColumnType::Date,
                    });
                } else if text.contains("REMARK")
                    || text.contains("DESC")
                    || text.contains("PARTICULARS")
                    || text.contains("NARRATION")
                    || text.contains("DETAILS")
                {
                    markers.push(Marker {
                        x: words.x,
                        col_type: ColumnType::Narration,
                    });
                } else if text.contains("DEBIT") || text.contains("WITHDRAWAL") {
                    markers.push(Marker {
                        x: words.x,
                        col_type: ColumnType::Debit,
                    });
                } else if text.contains("CREDIT") || text.contains("DEPOSIT") {
                    markers.push(Marker {
                        x: words.x,
                        col_type: ColumnType::Credit,
                    });
                } else if text == "AMOUNT" {
                    markers.push(Marker {
                        x: words.x,
                        col_type: ColumnType::Amount,
                    });
                } else if text.contains("BENEFICIARY")
                    || text.contains("SENDER")
                    || text.contains("RECEIVER")
                    || text.contains("COUNTERPARTY")
                    || text.contains("FROM/TO")
                    || text.contains("TO")
                {
                    markers.push(Marker {
                        x: words.x,
                        col_type: ColumnType::Counterparty,
                    });
                }
                // used as a right-side wall for other columns if it exists
                else if text.contains("BALANCE") || text.contains("BAL") {
                    markers.push(Marker {
                        x: words.x,
                        col_type: ColumnType::Balance,
                    });
                }
            }

            // i need to validate that at least i have the three major column we need from a statement
            let has_date = markers.iter().any(|m| m.col_type == ColumnType::Date);
            let has_narr = markers.iter().any(|m| m.col_type == ColumnType::Narration);
            let has_money = markers.iter().any(|m| {
                matches!(
                    m.col_type,
                    ColumnType::Debit | ColumnType::Credit | ColumnType::Amount
                )
            });

            // this covers all my cases
            if has_date && has_money && has_narr {
                // sort markers Left-to-Right by X coordinate, why? i don't know the order the words appeared in the raw pdf parsed
                // and i need the order to be correct to accuratly get the start and end of each column
                // sorting like this ensures that `markers[i+1]` is actually the neighbor to the right
                markers.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

                // need to initialize the layoutRule but fill everything with default values or state
                let mut rules = LayoutRules {
                    date_col_x_start: 0.0,
                    date_col_x_end: 0.0,
                    narration_col_x_start: 0.0,
                    narration_col_x_end: 0.0,
                    debit_col_x_start: None,
                    debit_col_x_end: None,
                    credit_col_x_start: None,
                    credit_col_x_end: None,
                    amount_col_x_start: None,
                    amount_col_x_end: None,
                    counterparty_col_x_start: None,
                    counterparty_col_x_end: None,
                };
                //  iterate and assign boundaries based on neighbors
                for i in 0..markers.len() {
                    let current = &markers[i];

                    // determine the start first
                    let start_x = if current.x > 10.0 {
                        current.x - 10.0
                    } else {
                        0.0
                    };

                    // determine end by Looking at the next marker
                    let end_x = if i + 1 < markers.len() {
                        let next_marker = &markers[i + 1];
                        if next_marker.x > 20.0 {
                            next_marker.x - 10.0
                        } else {
                            next_marker.x
                        }
                    } else {
                        1000.0 // Page Edge, meaning no right-side neighbor
                    };

                    //  fixed Width i want for Date
                    let final_end_x = if current.col_type == ColumnType::Date {
                        current.x + 90.0
                    } else {
                        end_x
                    };

                    // now i map everything to rules
                    match current.col_type {
                        ColumnType::Date => {
                            rules.date_col_x_start = start_x;
                            rules.date_col_x_end = final_end_x;
                        }
                        ColumnType::Narration => {
                            rules.narration_col_x_start = start_x;
                            rules.narration_col_x_end = final_end_x;
                        }
                        ColumnType::Debit => {
                            rules.debit_col_x_start = Some(start_x);
                            rules.debit_col_x_end = Some(final_end_x);
                        }
                        ColumnType::Credit => {
                            rules.credit_col_x_start = Some(start_x);
                            rules.credit_col_x_end = Some(final_end_x);
                        }
                        ColumnType::Amount => {
                            rules.amount_col_x_start = Some(start_x);
                            rules.amount_col_x_end = Some(final_end_x);
                        }
                        ColumnType::Counterparty => {
                            rules.counterparty_col_x_start = Some(start_x);
                            rules.counterparty_col_x_end = Some(final_end_x);
                        }
                        ColumnType::Balance => {
                            // no storing Balance rules,
                            // needed it for next marker tho
                        }
                    }
                }
                return Ok((rules, rows.y_position));
            }
        }

        Err(ParserError::UnknownFormat)
    }

    fn map_row_to_statement(&self, _row: &TableRow, _layout: &LayoutRules) -> Option<RawStatement> {
        // I have a valid layout rule that has coordinates
        // so how does row and layout work hand in hand
        // what does row actually contain? a y position and collect Of TextItems(x,y, item)
        // go into each row and just use the layout rule coordinates to drag out the content, but i have to begin this after the header row
        // i first do need to set default containers that would be passed to raw statment
        let mut date_str = String::new();
        let mut narration_str = String::new();
        let mut cp_str = String::new();

        // for amounts i need to collect them as strings first because they might be fragmented
        let mut debit_str = String::new();
        let mut credit_str = String::new();
        let mut amount_str = String::new();
        let mut amount_is_negative = false;

        //now that i'm on the row after the header what now?
        // i want to confirm if the x position of that word is greater than or equal various layout
        // if it is greater than or equal to the layouts rule x_start && less than or equal to x_end then allocate the value
        // go into each row and iterate items
        for item in &_row.items {
            let item_x_pos = item.x;
            let actual_item = item.text.trim();

            if actual_item.is_empty() {
                continue;
            }

            // lets go one column at a time
            // date filler
            if item_x_pos >= _layout.date_col_x_start && item_x_pos <= _layout.date_col_x_end {
                // there may be multiple items that fall between the x_end and y_end
                // so its better to push the string and a space incase there are multiple things
                date_str.push_str(actual_item);
                date_str.push_str(" ");
            }
            // narration filler
            else if item_x_pos >= _layout.narration_col_x_start
                && item_x_pos <= _layout.narration_col_x_end
            {
                narration_str.push_str(actual_item);
                narration_str.push_str(" ");
            }

            // remember that the LayoutRules struct has some options? like something can exist
            // or not exist in that particular statment
            // counterparty is one
            if let (Some(start), Some(end)) = (
                _layout.counterparty_col_x_start,
                _layout.counterparty_col_x_end,
            ) {
                if item_x_pos >= start && item_x_pos <= end {
                    cp_str.push_str(actual_item);
                    cp_str.push_str(" ");
                }
            }

            // then theres Amount, i have to be careful how i collect numbers
            // so i rather make sure i get all the numbers (bits under the y section of amount column)

            if let (Some(start), Some(end)) = (_layout.amount_col_x_start, _layout.amount_col_x_end)
            {
                // i want to put the bits togetther before allocating it
                if item_x_pos >= start && item_x_pos <= end {
                    amount_str.push_str(actual_item);

                    if item.text.contains("-")
                        || item.text.contains("DR")
                        || item.text.contains("Dr")
                        || item.text.contains("dr")
                    {
                        amount_is_negative = true;
                    }
                }
            }

            // now finally for debit and credit
            // similar to amount
            // Debit first
            if let (Some(start), Some(end)) = (_layout.debit_col_x_start, _layout.debit_col_x_end) {
                if item_x_pos >= start && item_x_pos <= end {
                    debit_str.push_str(actual_item);
                }
            }

            // Credit col
            if let (Some(start), Some(end)) = (_layout.credit_col_x_start, _layout.credit_col_x_end)
            {
                if item_x_pos >= start && item_x_pos <= end {
                    credit_str.push_str(actual_item);
                }
            }
        }

        // now finally calculate the money
        // just incase i need to clean non numeric chars that i may not know about
        // .chars is iterating and is_digit makes sure a character is a digit. When you iterate you collect
        let mut amount_val = Decimal::ZERO;
        let mut found_money = false;

        if !amount_str.is_empty() {
            let clean_money: String = amount_str
                .chars()
                .filter(|c| c.is_digit(10) || *c == '.')
                .collect();
            if let Ok(val) = clean_money.parse::<Decimal>() {
                if amount_is_negative {
                    amount_val = -val;
                } else {
                    amount_val = val;
                }
                found_money = true;
            }
        }

        if !debit_str.is_empty() {
            let clean_debit: String = debit_str
                .chars()
                .filter(|c| c.is_digit(10) || *c == '.')
                .collect();
            if let Ok(val) = clean_debit.parse::<Decimal>() {
                amount_val -= val;
                found_money = true;
            }
        }

        if !credit_str.is_empty() {
            let clean_credit: String = credit_str
                .chars()
                .filter(|c| c.is_digit(10) || *c == '.')
                .collect();
            if let Ok(val) = clean_credit.parse::<Decimal>() {
                amount_val += val;
                found_money = true;
            }
        }

        // validation to make sure its not a footer
        if date_str.trim().is_empty() || !found_money {
            return None;
        }

        Some(RawStatement {
            date: date_str.trim().to_string(),
            narration: narration_str.trim().to_string(),
            amount: amount_val,
            counterparty: if cp_str.is_empty() {
                None
            } else {
                Some(cp_str.trim().to_string())
            },
        })
    }
}
