//! The normalization rules, row by row.

use std::collections::HashMap;

use crate::amount::Amount;
use crate::date::{DateFormat, first_number, parse_date};
use crate::profile::{FileFormat, Profile};
use crate::table::{Table, find_column};
use crate::text::{fold, strip};
use crate::{
    Ambiguity, Error, Normalization, RowError, RowProblem, Summary, Transaction, csv, external_id, pdf,
};

/// Normalizes one export. `namespace` names the account the file belongs to and goes into every
/// `external_id`, so two accounts that share a format never share keys.
pub fn normalize(bytes: &[u8], profile: &Profile, namespace: &str) -> Result<Normalization, Error> {
    let formats = profile.date_formats().map_err(Error::InvalidProfile)?;
    let table = read_table(bytes, profile)?;
    let columns = Columns::resolve(&table, profile)?;
    let mut run = Run::new(profile, namespace, &formats, &columns);
    for (index, row) in table.rows.iter().enumerate() {
        run.row(index + 2, row, &table.headers);
    }
    Ok(run.finish(table.rows.len(), &formats))
}

fn read_table(bytes: &[u8], profile: &Profile) -> Result<Table, Error> {
    let invalid = |what: &str| Error::InvalidProfile(format!("{}: missing [{what}]", profile.id));
    match profile.format {
        FileFormat::Csv => {
            let settings = profile.csv.as_ref().ok_or_else(|| invalid("csv"))?;
            csv::read(bytes, settings).map_err(|e| {
                Error::Unreadable(format!(
                    "line {} has {} fields, expected {}",
                    e.line, e.found, e.expected
                ))
            })
        }
        FileFormat::Pdf => {
            let settings = profile.pdf.as_ref().ok_or_else(|| invalid("pdf"))?;
            let lines = pdf::read_lines(bytes).map_err(Error::Unreadable)?;
            pdf::lines_to_table(&lines, settings).map_err(Error::InvalidProfile)
        }
        FileFormat::Xlsx => crate::xlsx::read(bytes, profile).map_err(Error::Unreadable),
    }
}

/// Where each field is in this file, and which amount scheme it uses.
struct Columns {
    date: usize,
    description: usize,
    amount: AmountColumns,
}

enum AmountColumns {
    Signed(usize),
    DebitCredit {
        debit: Option<usize>,
        credit: Option<usize>,
    },
}

impl Columns {
    fn resolve(table: &Table, profile: &Profile) -> Result<Columns, Error> {
        let wanted = &profile.columns;
        let or_default = |list: &Vec<String>, name: &str| {
            if list.is_empty() && profile.format == FileFormat::Pdf {
                vec![name.to_owned()]
            } else {
                list.clone()
            }
        };
        let find = |list: Vec<String>| {
            if list.is_empty() {
                None
            } else {
                find_column(&table.headers, &list)
            }
        };
        let date = find(or_default(&wanted.date, "date"));
        let description = find(or_default(&wanted.description, "description"));
        let debit = find(or_default(&wanted.debit, "debit"));
        let credit = find(or_default(&wanted.credit, "credit"));
        let signed = find(or_default(&wanted.amount, "amount"));
        let amount = match (debit, credit, signed) {
            (None, None, Some(column)) => Some(AmountColumns::Signed(column)),
            (None, None, None) => None,
            (debit, credit, _) => Some(AmountColumns::DebitCredit { debit, credit }),
        };
        let mut missing = Vec::new();
        if date.is_none() {
            missing.push("date");
        }
        if description.is_none() {
            missing.push("description");
        }
        if amount.is_none() {
            missing.push("amount or debit/credit");
        }
        match (date, description, amount) {
            (Some(date), Some(description), Some(amount)) => Ok(Columns {
                date,
                description,
                amount,
            }),
            _ => Err(Error::MissingColumns {
                missing,
                found: table.headers.clone(),
            }),
        }
    }
}

/// State while walking the rows of one file.
struct Run<'a> {
    profile: &'a Profile,
    namespace: &'a str,
    formats: &'a [DateFormat],
    columns: &'a Columns,
    transactions: Vec<Transaction>,
    errors: Vec<RowError>,
    ambiguities: Vec<Ambiguity>,
    structural: usize,
    occurrences: HashMap<(String, String, String), usize>,
    saw_day_above_twelve: bool,
}

impl<'a> Run<'a> {
    fn new(
        profile: &'a Profile,
        namespace: &'a str,
        formats: &'a [DateFormat],
        columns: &'a Columns,
    ) -> Self {
        Run {
            profile,
            namespace,
            formats,
            columns,
            transactions: Vec::new(),
            errors: Vec::new(),
            ambiguities: Vec::new(),
            structural: 0,
            occurrences: HashMap::new(),
            saw_day_above_twelve: false,
        }
    }

    fn row(&mut self, number: usize, row: &[String], headers: &[String]) {
        let raw_date = strip(&row[self.columns.date]);
        // A row without a date is layout (opening balance, card number, footer), never an error.
        if raw_date.is_empty() {
            self.structural += 1;
            return;
        }
        if first_number(raw_date).is_some_and(|n| n > 12) {
            self.saw_day_above_twelve = true;
        }
        let date = parse_date(raw_date, self.formats);
        let (amount, both) = self.amount(row);
        let problem = match (&date, amount) {
            (None, None) => {
                self.structural += 1;
                return;
            }
            (None, _) => Some(RowProblem::UnreadableDate(raw_date.to_owned())),
            (_, None) => Some(RowProblem::UnreadableAmount),
            _ => None,
        };
        if let Some(problem) = problem {
            let cells = headers.iter().cloned().zip(row.iter().cloned()).collect();
            self.errors.push(RowError {
                row: number,
                problem,
                cells,
            });
            return;
        }
        let (Some(date), Some(amount)) = (date, amount) else {
            return;
        };
        let description = strip(&row[self.columns.description]).to_owned();
        self.flag(number, &description, amount, both);
        let amount_text = amount.to_string();
        let key = (date.clone(), amount_text.clone(), fold(&description));
        let occurrence = self.occurrences.entry(key).or_insert(0);
        *occurrence += 1;
        let external_id = external_id(self.namespace, &date, &amount_text, &description, *occurrence);
        self.transactions.push(Transaction {
            date,
            amount,
            description,
            external_id,
        });
    }

    /// The signed amount, and the debit and credit when both are present and not zero.
    fn amount(&self, row: &[String]) -> (Option<Amount>, Option<(Amount, Amount)>) {
        let settings = &self.profile.amounts;
        let read = |column: usize| parse_amount(&row[column], &settings.decimal, &settings.thousands);
        let (amount, both) = match self.columns.amount {
            AmountColumns::Signed(column) => (read(column), None),
            AmountColumns::DebitCredit { debit, credit } => {
                let debit = debit.and_then(read);
                let credit = credit.and_then(read);
                let nonzero = |value: Option<Amount>| value.filter(|a| !a.is_zero());
                let both = nonzero(debit).zip(nonzero(credit));
                let amount = (debit.is_some() || credit.is_some()).then(|| {
                    let debit = nonzero(debit).unwrap_or(Amount::ZERO);
                    nonzero(credit).unwrap_or(Amount::ZERO) + debit.times_sign(settings.debit_sign)
                });
                (amount, both)
            }
        };
        (
            if settings.invert_sign {
                amount.map(Amount::negate)
            } else {
                amount
            },
            both,
        )
    }

    fn flag(&mut self, row: usize, description: &str, amount: Amount, both: Option<(Amount, Amount)>) {
        if description.is_empty() {
            self.ambiguities.push(Ambiguity::EmptyDescription { row });
        }
        if let Some((debit, credit)) = both {
            self.ambiguities
                .push(Ambiguity::DebitAndCredit { row, debit, credit });
        }
        if amount.has_sub_cents() {
            self.ambiguities.push(Ambiguity::SubCents { row, amount });
        }
    }

    fn finish(mut self, rows: usize, formats: &[DateFormat]) -> Normalization {
        let day_first = formats.first().is_some_and(|f| f.day_first);
        if day_first && !self.transactions.is_empty() && !self.saw_day_above_twelve {
            self.ambiguities.push(Ambiguity::UnconfirmedDayMonthOrder);
        }
        let dates = self.transactions.iter().map(|t| t.date.clone());
        let date_range = dates.clone().min().zip(dates.max());
        let total = |keep: fn(&Amount) -> bool| {
            self.transactions
                .iter()
                .map(|t| t.amount)
                .filter(keep)
                .fold(Amount::ZERO, |sum, a| sum + a)
        };
        let summary = Summary {
            rows,
            transactions: self.transactions.len(),
            errors: self.errors.len(),
            structural: self.structural,
            date_range,
            currency: self.profile.currency.clone(),
            debit_total: total(|a| a.is_negative()),
            credit_total: total(|a| !a.is_negative() && !a.is_zero()),
        };
        Normalization {
            transactions: self.transactions,
            errors: self.errors,
            ambiguities: self.ambiguities,
            summary,
        }
    }
}

const CURRENCY_SYMBOLS: [&str; 4] = ["$", "B/.", "USD", "US$"];

/// Reads an amount cell: currency symbols removed, accounting parentheses and a leading or trailing minus
/// mean negative, thousands separators dropped, the bank's decimal separator turned into a point.
pub fn parse_amount(cell: &str, decimal: &str, thousands: &str) -> Option<Amount> {
    let mut text = strip(cell).to_owned();
    if text.is_empty() {
        return None;
    }
    let mut negative = false;
    if text.len() >= 2 && text.starts_with('(') && text.ends_with(')') {
        negative = true;
        text = strip(&text[1..text.len() - 1]).to_owned();
    }
    for symbol in CURRENCY_SYMBOLS {
        text = text.replace(symbol, "");
    }
    let mut text = strip(&text).to_owned();
    if text.starts_with('-') || text.ends_with('-') {
        negative = true;
    }
    text = strip(text.trim_matches(['+', '-'])).to_owned();
    if !thousands.is_empty() && thousands != decimal {
        text = text.replace(thousands, "");
    }
    if !decimal.is_empty() && decimal != "." {
        text = text.replace(decimal, ".");
    }
    let text = text.replace(' ', "");
    if text.is_empty() || text == "." || text == "," {
        return None;
    }
    let amount = Amount::parse(&text)?;
    Some(if negative { amount.negate() } else { amount })
}

#[cfg(test)]
mod tests {
    use super::parse_amount;

    fn read(cell: &str) -> Option<String> {
        parse_amount(cell, ".", ",").map(|a| a.to_string())
    }

    #[test]
    fn reads_signs_symbols_and_separators() {
        assert_eq!(read("(123.00)").as_deref(), Some("-123.00"));
        assert_eq!(read("(-123.00)").as_deref(), Some("-123.00"));
        assert_eq!(read("45.50-").as_deref(), Some("-45.50"));
        assert_eq!(read("$ 1,234.56").as_deref(), Some("1234.56"));
        assert_eq!(read("-$2,300.00").as_deref(), Some("-2300.00"));
        assert_eq!(
            parse_amount("1.500,00", ",", ".")
                .map(|a| a.to_string())
                .as_deref(),
            Some("1500.00")
        );
        assert_eq!(read("-0.00").as_deref(), Some("0.00"));
        assert_eq!(read("   "), None);
        assert_eq!(read("nan"), None);
    }
}
