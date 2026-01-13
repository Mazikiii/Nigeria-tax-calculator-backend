use crate::domain::entities::{ParserError, RawStatement};
use crate::domain::ports::StatementParser;
use async_trait::async_trait;
use lopdf::Document;
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
}

// i need to to implement some sort of adapter

pub struct PdfParserAdapter;
