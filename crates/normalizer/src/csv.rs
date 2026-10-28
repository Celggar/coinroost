//! CSV exports: section cropping and a tokenizer with the quoting rules of the reference reader.
//!
//! Fields may be quoted with `"` (a doubled quote inside is a literal quote); a quote anywhere else is a
//! literal character. With `skip_initial_space`, spaces (U+0020 only) at the start of a field are dropped.
//! Blank lines and lines holding only spaces and tabs are skipped. Every cell stays text.

use crate::profile::Csv;
use crate::table::{Table, name_headers};
use crate::text::{decode, fold, split_lines};

/// A row with more cells than the header: the file is not in the expected shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TooManyFields {
    pub line: usize,
    pub expected: usize,
    pub found: usize,
}

/// Decodes, crops to the transactions section and tokenizes.
pub fn read(bytes: &[u8], settings: &Csv) -> Result<Table, TooManyFields> {
    let text = decode(bytes, settings.encoding);
    let text = match &settings.section_start {
        Some(start) => crop_section(&text, start, settings.section_end.as_deref()),
        None => text,
    };
    let mut records = tokenize(&text, settings.delimiter, settings.skip_initial_space).into_iter();
    let headers = name_headers(records.next().unwrap_or_default());
    let mut rows = Vec::new();
    for (index, record) in records.enumerate() {
        if record.len() > headers.len() {
            return Err(TooManyFields {
                line: index + 2,
                expected: headers.len(),
                found: record.len(),
            });
        }
        rows.push(record);
    }
    Ok(Table::new(headers, rows))
}

/// The lines from the first one containing `start` (folded) up to, not including, the first blank line
/// (or one holding only commas, spaces and tabs) or the first line containing `end`. Without a line
/// containing `start`, the whole text.
pub fn crop_section(text: &str, start: &str, end: Option<&str>) -> String {
    let lines = split_lines(text);
    let start = fold(start);
    let Some(first) = lines.iter().position(|line| fold(line).contains(&start)) else {
        return text.to_owned();
    };
    let end = end.map(fold);
    let mut block = vec![lines[first]];
    for line in &lines[first + 1..] {
        if line.trim_matches([',', ' ', '\t']).is_empty() {
            break;
        }
        if end.as_ref().is_some_and(|end| fold(line).contains(end.as_str())) {
            break;
        }
        block.push(line);
    }
    block.join("\n")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    StartRecord,
    StartField,
    InField,
    InQuoted,
    QuoteInQuoted,
    WhitespaceLine,
}

struct Tokenizer {
    delimiter: char,
    skip_initial_space: bool,
    state: State,
    field: String,
    record: Vec<String>,
    records: Vec<Vec<String>>,
}

impl Tokenizer {
    fn end_field(&mut self) {
        self.record.push(std::mem::take(&mut self.field));
    }

    fn end_record(&mut self) {
        self.end_field();
        self.records.push(std::mem::take(&mut self.record));
        self.state = State::StartRecord;
    }

    fn feed(&mut self, c: char) {
        let newline = c == '\n' || c == '\r';
        match self.state {
            State::StartRecord if newline => {}
            State::StartRecord if (c == ' ' || c == '\t') && c != self.delimiter => {
                self.field.push(c);
                self.state = State::WhitespaceLine;
            }
            State::StartRecord | State::StartField => self.start_field(c),
            State::WhitespaceLine if newline => {
                self.field.clear();
                self.state = State::StartRecord;
            }
            State::WhitespaceLine if c == ' ' || c == '\t' => self.field.push(c),
            State::WhitespaceLine => {
                let pending = std::mem::take(&mut self.field);
                self.state = State::StartField;
                for p in pending.chars().chain(std::iter::once(c)) {
                    self.feed(p);
                }
            }
            State::InField if newline => self.end_record(),
            State::InField if c == self.delimiter => {
                self.end_field();
                self.state = State::StartField;
            }
            State::InQuoted if c == '"' => self.state = State::QuoteInQuoted,
            State::InField | State::InQuoted => self.field.push(c),
            State::QuoteInQuoted => self.after_closing_quote(c, newline),
        }
    }

    fn start_field(&mut self, c: char) {
        if c == '\n' || c == '\r' {
            self.end_record();
        } else if c == '"' {
            self.state = State::InQuoted;
        } else if c == ' ' && self.skip_initial_space {
            self.state = State::StartField;
        } else if c == self.delimiter {
            self.end_field();
            self.state = State::StartField;
        } else {
            self.field.push(c);
            self.state = State::InField;
        }
    }

    fn after_closing_quote(&mut self, c: char, newline: bool) {
        if c == '"' {
            self.field.push('"');
            self.state = State::InQuoted;
        } else if c == self.delimiter {
            self.end_field();
            self.state = State::StartField;
        } else if newline {
            self.end_record();
        } else {
            self.field.push(c);
            self.state = State::InField;
        }
    }

    fn finish(mut self) -> Vec<Vec<String>> {
        match self.state {
            State::StartRecord | State::WhitespaceLine => {}
            _ => self.end_record(),
        }
        self.records
    }
}

/// Splits text into records of fields.
pub fn tokenize(text: &str, delimiter: char, skip_initial_space: bool) -> Vec<Vec<String>> {
    let mut tokenizer = Tokenizer {
        delimiter,
        skip_initial_space,
        state: State::StartRecord,
        field: String::new(),
        record: Vec::new(),
        records: Vec::new(),
    };
    for c in text.chars() {
        tokenizer.feed(c);
    }
    tokenizer.finish()
}

#[cfg(test)]
mod tests {
    use super::{crop_section, tokenize};

    #[test]
    fn honors_quotes_and_initial_spaces() {
        let records = tokenize("a, \"b, c\", d \n  \n\"x\"\"y\",z\"w\n", ',', true);
        assert_eq!(records, [vec!["a", "b, c", "d "], vec!["x\"y", "z\"w"]]);
    }

    #[test]
    fn keeps_initial_spaces_when_asked() {
        assert_eq!(tokenize("a, b", ',', false), [vec!["a", " b"]]);
    }

    #[test]
    fn crops_to_the_section() {
        let text = "summary\n1,2\n\nDetalle\nFecha de Transacción, x\n1, 2\n, ,\nTotals\n";
        assert_eq!(
            crop_section(text, "Fecha de Transacción", None),
            "Fecha de Transacción, x\n1, 2"
        );
        let card = "Date, , Local\n1, a, 2\nCURRENT Interest, y\n";
        assert_eq!(
            crop_section(card, "Date, , Local", Some("CURRENT Interest")),
            "Date, , Local\n1, a, 2"
        );
    }
}
