//! XLSX exports read as a grid of text cells from A1, the way the reference reads them (openpyxl values
//! printed by pandas):
//!
//! - a whole number prints without decimals (`1300`), any other number in the shortest form that reads back
//!   to the same double (`1000.79`, `1e-05`);
//! - a number with a date format becomes `YYYY-MM-DD HH:MM:SS` (plus `.ffffff` when there are fractions of a
//!   second), rounded to the millisecond, with the 1900 leap-year bug of the format honored;
//! - booleans print `True`/`False`, error cells `nan`, empty cells nothing;
//! - rows and columns start at A1 even when the sheet starts further in; trailing empty rows and columns go.

use std::io::Cursor;

use calamine::{Data, Reader, Xlsx};

use crate::profile::Profile;
use crate::table::{Table, name_headers};
use crate::text::fold;

/// The first sheet as a table whose header is the first row containing the profile's `header_marker`
/// (folded), or the first row when there is none.
pub fn read(bytes: &[u8], profile: &Profile) -> Result<Table, String> {
    let mut grid = first_sheet(bytes)?;
    let marker = profile
        .xlsx
        .as_ref()
        .and_then(|x| x.header_marker.as_deref())
        .map(fold);
    let header_index = marker
        .and_then(|marker| {
            grid.iter()
                .position(|row| row.iter().any(|cell| fold(cell).contains(&marker)))
        })
        .unwrap_or(0);
    if grid.len() <= header_index {
        return Ok(Table::default());
    }
    let rows = grid.split_off(header_index + 1);
    let headers = name_headers(grid.pop().unwrap_or_default());
    Ok(Table::new(headers, rows))
}

/// Sheet names and the first 12 rows of up to 20 columns of every sheet, for account detection.
pub fn header_text(bytes: &[u8]) -> Result<String, String> {
    let mut workbook: Xlsx<_> = Xlsx::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut parts = Vec::new();
    for name in workbook.sheet_names() {
        parts.push(name.clone());
        let range = workbook.worksheet_range(&name).map_err(|e| e.to_string())?;
        for row in grid(&range).into_iter().take(12) {
            parts.extend(row.into_iter().take(20).filter(|cell| !cell.is_empty()));
        }
    }
    Ok(parts.join(" \n"))
}

fn first_sheet(bytes: &[u8]) -> Result<Vec<Vec<String>>, String> {
    let mut workbook: Xlsx<_> = Xlsx::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or("the workbook has no sheets")?;
    let range = workbook.worksheet_range(&name).map_err(|e| e.to_string())?;
    Ok(grid(&range))
}

/// Cells as text from A1, trailing empty rows dropped, every row padded to the widest one.
fn grid(range: &calamine::Range<Data>) -> Vec<Vec<String>> {
    let Some((first_row, first_column)) = range.start() else {
        return Vec::new();
    };
    let offset = usize::try_from(first_column).unwrap_or(0);
    let mut rows: Vec<Vec<String>> = vec![Vec::new(); usize::try_from(first_row).unwrap_or(0)];
    for row in range.rows() {
        let mut cells: Vec<String> = vec![String::new(); offset];
        cells.extend(row.iter().map(cell_text));
        while cells.last().is_some_and(String::is_empty) {
            cells.pop();
        }
        rows.push(cells);
    }
    while rows.last().is_some_and(Vec::is_empty) {
        rows.pop();
    }
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    for row in &mut rows {
        row.resize(width, String::new());
    }
    rows
}

fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(text) | Data::DateTimeIso(text) | Data::DurationIso(text) => text.clone(),
        Data::Int(value) => value.to_string(),
        Data::Float(value) => number_text(*value),
        Data::Bool(value) => if *value { "True" } else { "False" }.to_owned(),
        Data::Error(_) => crate::table::MISSING.to_owned(),
        Data::DateTime(value) => {
            if value.is_duration() {
                number_text(value.as_f64())
            } else {
                serial_to_text(value.as_f64())
            }
        }
    }
}

/// A whole number without decimals; anything else in the shortest round-trip form, in scientific notation
/// when the exponent is below -4 or at least 16.
pub fn number_text(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e30 {
        #[allow(clippy::cast_possible_truncation)]
        return (value as i128).to_string();
    }
    let scientific = format!("{value:e}");
    let Some((mantissa, exponent)) = scientific.split_once('e') else {
        return scientific;
    };
    let exponent: i32 = exponent.parse().unwrap_or(0);
    if (-4..16).contains(&exponent) {
        return format!("{value}");
    }
    format!(
        "{mantissa}e{}{:02}",
        if exponent < 0 { '-' } else { '+' },
        exponent.abs()
    )
}

/// An Excel date serial (1900 system) as printed by the reference: `YYYY-MM-DD HH:MM:SS[.ffffff]`, or the
/// time alone for a value below one day.
fn serial_to_text(serial: f64) -> String {
    let day = serial.floor();
    let fraction = serial - day;
    #[allow(clippy::cast_possible_truncation)]
    let mut milliseconds = (fraction * 86_400_000.0).round_ties_even() as i64;
    #[allow(clippy::cast_possible_truncation)]
    let mut days = day as i64;
    days += milliseconds.div_euclid(86_400_000);
    milliseconds = milliseconds.rem_euclid(86_400_000);
    let clock = clock_text(milliseconds);
    if (0.0..1.0).contains(&serial) && days == 0 {
        return clock;
    }
    if serial > 0.0 && serial < 60.0 {
        days += 1;
    }
    // Day 0 of the 1900 system is 1899-12-30.
    let (year, month, day) = civil_from_days(days + days_from_civil(1899, 12, 30));
    format!("{year:04}-{month:02}-{day:02} {clock}")
}

fn clock_text(milliseconds: i64) -> String {
    let seconds = milliseconds / 1000;
    let micros = (milliseconds % 1000) * 1000;
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if micros == 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{hours:02}:{minutes:02}:{seconds:02}.{micros:06}")
    }
}

/// Days since 1970-01-01 of a proleptic Gregorian date (H. Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::{number_text, serial_to_text};

    #[test]
    fn prints_numbers_like_the_reference() {
        assert_eq!(number_text(1300.0), "1300");
        assert_eq!(number_text(-800.0), "-800");
        assert_eq!(number_text(1000.79), "1000.79");
        assert_eq!(number_text(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(number_text(0.00001), "1e-05");
        assert_eq!(number_text(0.0001), "0.0001");
    }

    #[test]
    fn prints_date_serials_like_the_reference() {
        // Expected values are openpyxl's `from_excel`, printed with `str`.
        assert_eq!(serial_to_text(46_199.992_997_685_19), "2026-06-26 23:49:55");
        assert_eq!(serial_to_text(46_199.500_000_1), "2026-06-26 12:00:00.009000");
        assert_eq!(serial_to_text(1.0), "1900-01-01 00:00:00");
        assert_eq!(serial_to_text(46_199.0), "2026-06-26 00:00:00");
        assert_eq!(serial_to_text(0.5), "12:00:00");
        assert_eq!(serial_to_text(59.0), "1900-02-28 00:00:00");
        assert_eq!(serial_to_text(61.0), "1900-03-01 00:00:00");
    }
}
