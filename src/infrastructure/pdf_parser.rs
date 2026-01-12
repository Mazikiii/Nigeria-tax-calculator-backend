use async_trait::async_trait;
use domain::entities::{ParserError, RawStatement};
use domain::ports; // contains the interface that has parses and process, template defined
use lopdf::Document;
use rust_decimals::Decimal;
use std::collections::BTreeMap;

// i need something that works as coordination to identify keywords in columns
// so to pinpoint the position and word i'll define field x, y and text
#[derive(Debug, Clone)]
struct column_coordinate {
    x: f32,
    y: f32,
    text: String,
}

// i need to identify rows with y axies, Y=500.1` and `Y=500.2` are on the Same Row
// so i need to be getting rows and comparing with the column detector so i identify the words
//
#[derive(Debug, Clone)]
struct row_identifier {
    y: f64,
    row_storage: Vec<column_coordinate>,
}

// based on the row and column identification we need hold the layout
// that we need for rawstatement
#[derive(Debug, Clone)]
struct TableLayout {
    x_date_col_start: f64,
    x_date_col_end: f64,
    x_narration_col_start: f64,
    x_narration_col_end: f64,
}
