//! PDF exports. Two ways to turn a statement into rows, chosen by the profile: one transaction per text
//! line (`lines`), or the cells of tables drawn with ruling lines (`tables`), for statements whose
//! descriptions span several lines.

mod edges;
mod geometry;
#[cfg(feature = "pdf")]
mod glyphs;
mod layout;
#[cfg(feature = "pdf")]
mod paths;
mod tables;

pub use edges::{Shape, ShapeKind};
pub use layout::{Glyph, cell_text, page_text};
pub use tables::{Row, extract_tables};

use regex::Regex;

use crate::table::Table;
use crate::text::{fold, split_lines, split_whitespace, strip};

/// The column names a `lines` profile produces, in order.
const LINE_COLUMNS: [&str; 5] = ["date", "description", "amount", "debit", "credit"];

/// The text lines of every page, in page order.
#[cfg(feature = "pdf")]
pub fn read_lines(bytes: &[u8]) -> Result<Vec<String>, String> {
    let document = pdf_oxide::PdfDocument::from_bytes(bytes.to_vec()).map_err(|e| e.to_string())?;
    let pages = document.page_count().map_err(|e| e.to_string())?;
    let mut lines = Vec::new();
    for page in 0..pages {
        let glyphs = glyphs::page_glyphs(&document, page)?;
        lines.extend(split_lines(&page_text(&glyphs)).into_iter().map(str::to_owned));
    }
    Ok(lines)
}

/// Without the `pdf` feature there is no PDF reader.
#[cfg(not(feature = "pdf"))]
pub fn read_lines(_bytes: &[u8]) -> Result<Vec<String>, String> {
    Err("this build has no PDF support".to_owned())
}

/// The text of the first page, where statements print the account.
#[cfg(feature = "pdf")]
pub fn first_page_text(bytes: &[u8]) -> Result<String, String> {
    let document = pdf_oxide::PdfDocument::from_bytes(bytes.to_vec()).map_err(|e| e.to_string())?;
    if document.page_count().map_err(|e| e.to_string())? == 0 {
        return Ok(String::new());
    }
    Ok(page_text(&glyphs::page_glyphs(&document, 0)?))
}

#[cfg(not(feature = "pdf"))]
pub fn first_page_text(_bytes: &[u8]) -> Result<String, String> {
    Err("this build has no PDF support".to_owned())
}

/// Every ruled table of every page, in page order.
#[cfg(feature = "pdf")]
pub fn read_tables(bytes: &[u8]) -> Result<Vec<Vec<Row>>, String> {
    let document = pdf_oxide::PdfDocument::from_bytes(bytes.to_vec()).map_err(|e| e.to_string())?;
    let mut found = Vec::new();
    for page in 0..document.page_count().map_err(|e| e.to_string())? {
        let content = glyphs::page_content(&document, page)?;
        found.extend(extract_tables(&content.glyphs, &content.shapes));
    }
    Ok(found)
}

#[cfg(not(feature = "pdf"))]
pub fn read_tables(_bytes: &[u8]) -> Result<Vec<Vec<Row>>, String> {
    Err("this build has no PDF support".to_owned())
}

/// Turns the rows of every table into one table: the first row with a cell equal to `header` (folded)
/// names the columns, and the rows whose first cell matches `row_start` are the transactions. Other rows,
/// such as the header repeated on each page or an opening balance, are left out. Cells are flattened to
/// one line with single spaces; rows are cut or padded with empty cells to the header's width. Without a
/// header row, columns are named `col0`, `col1`… and no column is found.
pub fn tables_to_table(tables: &[Vec<Row>], header: &str, row_start: &str) -> Result<Table, String> {
    let row_start = anchored(row_start)?;
    let header = fold(header);
    let mut headers: Option<Vec<String>> = None;
    let mut rows: Vec<Vec<String>> = Vec::new();
    for row in tables.iter().flatten() {
        let cells: Vec<String> = row
            .iter()
            .map(|cell| {
                split_whitespace(cell.as_deref().unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        if headers.is_none() && cells.iter().any(|cell| fold(cell) == header) {
            headers = Some(cells);
        } else if cells.first().is_some_and(|cell| row_start.is_match(cell)) {
            rows.push(cells);
        }
    }
    let headers = headers.unwrap_or_else(|| {
        let width = rows.iter().map(Vec::len).max().unwrap_or(0);
        (0..width).map(|i| format!("col{i}")).collect()
    });
    for row in &mut rows {
        row.resize(headers.len(), String::new());
    }
    Ok(Table::new(headers, rows))
}

/// A pattern that matches from the start of the text, whether or not it begins with `^`.
fn anchored(pattern: &str) -> Result<Regex, String> {
    Regex::new(&format!("^(?:{pattern})")).map_err(|e| e.to_string())
}

/// Turns statement lines into a table, keeping only lines that start a transaction. A line that starts one
/// but does not match the full pattern keeps its first token as the date and the whole line as the
/// description, so it is reported as an error instead of disappearing.
pub fn lines_to_table(lines: &[String], row_start: &str, row: &str) -> Result<Table, String> {
    let row_start = anchored(row_start)?;
    let row = anchored(row)?;
    let headers: Vec<String> = LINE_COLUMNS
        .iter()
        .filter(|name| row.capture_names().any(|n| n == Some(name)))
        .map(|n| (*n).to_owned())
        .collect();
    let mut rows = Vec::new();
    for line in lines.iter().filter(|line| row_start.is_match(line)) {
        rows.push(match row.captures(line) {
            Some(captures) => headers
                .iter()
                .map(|h| {
                    let value = captures.name(h).map_or("", |m| m.as_str());
                    if h == "description" { strip(value) } else { value }.to_owned()
                })
                .collect(),
            None => headers
                .iter()
                .map(|h| match h.as_str() {
                    "date" => split_whitespace(line).next().unwrap_or_default().to_owned(),
                    "description" => strip(line).to_owned(),
                    _ => String::new(),
                })
                .collect(),
        });
    }
    Ok(Table::new(headers, rows))
}

#[cfg(test)]
mod tests {
    use super::{lines_to_table, tables_to_table};
    use crate::profile::{Pdf, Profile};

    #[test]
    fn keeps_transaction_lines_and_reports_broken_ones() {
        let profile = Profile::find("coopeduc").unwrap();
        let lines: Vec<String> = [
            "COOPERATIVA DE AHORRO Y CRÉDITO EL EDUCADOR, R.L.",
            "Fecha Referencia Detalle Débito Crédito Balance",
            "13/12/2024 90000001 DEPOSITO EN EFECTIVO 0.00 1,000.00 7,011.76",
            "31/03/2025 90000002 MOVIMIENTO SIN MONTOS",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let Some(Pdf::Lines { row_start, row }) = profile.pdf else {
            panic!("coopeduc reads lines");
        };
        let table = lines_to_table(&lines, &row_start, &row).unwrap();
        assert_eq!(table.headers, ["date", "description", "debit", "credit"]);
        assert_eq!(
            table.rows,
            [
                vec!["13/12/2024", "DEPOSITO EN EFECTIVO", "0.00", "1,000.00"],
                vec!["31/03/2025", "31/03/2025 90000002 MOVIMIENTO SIN MONTOS", "", ""],
            ]
        );
    }

    #[test]
    fn takes_the_header_row_and_the_dated_rows_of_every_table() {
        let cell = |s: &str| Some(s.to_owned());
        let tables = vec![
            vec![
                vec![cell("Fecha"), cell("Descripción"), cell("Monto")],
                vec![cell(""), cell("SALDO INICIAL"), None],
                vec![cell("05-jun-2026"), cell("DEPOSITO\nREF  7001"), cell("$150.00")],
            ],
            vec![
                vec![cell("Fecha"), cell("Descripción"), cell("Monto")],
                vec![cell("20-jun-2026"), cell("ABONO")],
            ],
        ];
        let table = tables_to_table(&tables, "fecha", r"\d{1,2}[-/]\w").unwrap();
        assert_eq!(table.headers, ["Fecha", "Descripción", "Monto"]);
        assert_eq!(
            table.rows,
            [
                vec!["05-jun-2026", "DEPOSITO REF 7001", "$150.00"],
                vec!["20-jun-2026", "ABONO", ""],
            ]
        );
    }
}
