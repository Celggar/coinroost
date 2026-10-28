//! PDF exports: page text from glyph positions, then one table row per transaction line.

#[cfg(feature = "pdf")]
mod glyphs;
mod layout;

pub use layout::{Glyph, page_text};

use regex::Regex;

use crate::profile::{Pdf, PdfStrategy};
use crate::table::Table;
use crate::text::{split_lines, split_whitespace, strip};

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

/// Turns statement lines into a table, keeping only lines that start a transaction. A line that starts one
/// but does not match the full pattern keeps its first token as the date and the whole line as the
/// description, so it is reported as an error instead of disappearing.
pub fn lines_to_table(lines: &[String], settings: &Pdf) -> Result<Table, String> {
    let PdfStrategy::Lines = settings.strategy;
    // Both patterns match from the start of the line, whether or not they begin with `^`.
    let anchored = |pattern: &str| Regex::new(&format!("^(?:{pattern})")).map_err(|e| e.to_string());
    let row_start = anchored(&settings.row_start)?;
    let row = anchored(&settings.row)?;
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
    use super::lines_to_table;
    use crate::profile::Profile;

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
        let table = lines_to_table(&lines, profile.pdf.as_ref().unwrap()).unwrap();
        assert_eq!(table.headers, ["date", "description", "debit", "credit"]);
        assert_eq!(
            table.rows,
            [
                vec!["13/12/2024", "DEPOSITO EN EFECTIVO", "0.00", "1,000.00"],
                vec!["31/03/2025", "31/03/2025 90000002 MOVIMIENTO SIN MONTOS", "", ""],
            ]
        );
    }
}
