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
    // TODO: Add amount column rules (single signed column vs debit/credit columns)
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

        let mut results = Vec::new();
        for row in rows {
            if self.is_header_row(&row) {
                continue;
            }

            if let Some(stmt) = self.map_row_to_statement(&row, &layout) {
                results.push(stmt);
            }
        }

        Ok(results)
    }
}

// HELPERs that processes a pdf
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

    fn cluster_rows(&self, _items: Vec<TextItem>) -> Vec<TableRow> {
        vec![]
    }

    fn detect_layout(&self, _rows: &[TableRow]) -> Result<LayoutRules, ParserError> {
        Err(ParserError::UnknownFormat)
    }

    fn map_row_to_statement(&self, _row: &TableRow, _layout: &LayoutRules) -> Option<RawStatement> {
        None
    }

    fn is_header_row(&self, _row: &TableRow) -> bool {
        false
    }
}
