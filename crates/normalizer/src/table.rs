//! A statement read as a table of text cells, and the tolerant lookup of its columns.

use crate::text::fold;

/// Header names and rows of text cells. Every row has one cell per header.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// The text a cell reads as when its row is shorter than the header, as in the reference implementation
/// (a missing value printed as text). It is never a valid date or amount.
pub const MISSING: &str = "nan";

impl Table {
    /// Builds a table, padding short rows with [`MISSING`].
    pub fn new(headers: Vec<String>, rows: Vec<Vec<String>>) -> Table {
        let width = headers.len();
        let rows = rows
            .into_iter()
            .map(|mut row| {
                row.resize(width, MISSING.to_owned());
                row
            })
            .collect();
        Table { headers, rows }
    }
}

/// Names for headers that are empty or repeated: `Unnamed: <index>` for an empty one, `name.1`, `name.2`
/// for repeats.
pub fn name_headers(raw: Vec<String>) -> Vec<String> {
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut out = Vec::with_capacity(raw.len());
    for (index, name) in raw.into_iter().enumerate() {
        let mut name = if name.is_empty() {
            format!("Unnamed: {index}")
        } else {
            name
        };
        let mut count = counts.get(&name).copied().unwrap_or(0);
        while count > 0 {
            counts.insert(name.clone(), count + 1);
            name = format!("{name}.{count}");
            count = counts.get(&name).copied().unwrap_or(0);
        }
        counts.insert(name.clone(), count + 1);
        out.push(name);
    }
    out
}

/// Index of the first column that matches a candidate: an exact folded match for any candidate first,
/// then containment either way. When two headers fold to the same text, the later one is used.
pub fn find_column(headers: &[String], candidates: &[String]) -> Option<usize> {
    let mut folded: Vec<(String, usize)> = Vec::new();
    for (index, header) in headers.iter().enumerate() {
        let key = fold(header);
        match folded.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 = index,
            None => folded.push((key, index)),
        }
    }
    let wanted: Vec<String> = candidates.iter().map(|c| fold(c)).collect();
    for candidate in &wanted {
        if let Some((_, index)) = folded.iter().find(|(key, _)| key == candidate) {
            return Some(*index);
        }
    }
    for candidate in wanted.iter().filter(|c| !c.is_empty()) {
        for (key, index) in &folded {
            if key.contains(candidate.as_str()) || candidate.contains(key.as_str()) {
                return Some(*index);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{find_column, name_headers};

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn names_empty_and_repeated_headers() {
        assert_eq!(
            name_headers(strings(&["Date", "", "Local", "Local", "Local.1"])),
            ["Date", "Unnamed: 1", "Local", "Local.1", "Local.1.1"]
        );
    }

    #[test]
    fn finds_columns_exactly_first_then_by_containment() {
        let headers = strings(&[
            "Fecha de Transacción",
            "Descripción de Transacción",
            "Débito de Transacción",
        ]);
        assert_eq!(
            find_column(&headers, &strings(&["fecha de transaccion", "fecha"])),
            Some(0)
        );
        assert_eq!(find_column(&headers, &strings(&["debito"])), Some(2));
        assert_eq!(find_column(&headers, &strings(&["monto"])), None);
    }
}
