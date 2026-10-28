//! Coinroost's bank statement normalizer.
//!
//! [`normalize`] reads one export (CSV, XLSX or PDF) with the [`Profile`] of its bank and returns every
//! transaction with an ISO date, a signed decimal amount, the description as printed and a stable
//! deduplication key. Nothing is dropped silently: rows that cannot be read come back as [`RowError`]s,
//! suspicious ones as [`Ambiguity`]s, and rows that are part of the layout (headers, opening balances,
//! footers) are counted as structural.
//!
//! The function is pure: no I/O, no clock, no randomness. The same bytes always give the same result.

pub mod amount;
mod csv;
pub mod date;
pub mod detect;
mod external_id;
mod normalize;
pub mod pdf;
pub mod profile;
mod report;
pub mod table;
pub mod text;
mod xlsx;

pub use amount::Amount;
pub use external_id::external_id;
pub use normalize::normalize;
pub use profile::{FileFormat, Profile};

/// One normalized transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transaction {
    /// ISO 8601 booking date, `YYYY-MM-DD`.
    pub date: String,
    /// Signed: negative is money leaving the account.
    pub amount: Amount,
    /// As printed, surrounding whitespace removed.
    pub description: String,
    /// Stable deduplication key; see [`external_id`].
    pub external_id: String,
}

/// Why a row could not become a transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowProblem {
    /// The date cell is not empty but matches none of the profile's formats.
    UnreadableDate(String),
    /// The row has a date but no amount that can be read.
    UnreadableAmount,
}

/// A row that could not be read, kept with its cells so it can be shown and fixed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowError {
    /// Human row number: 1 is the header, 2 the first data row.
    pub row: usize,
    pub problem: RowProblem,
    /// `(header, cell)` pairs.
    pub cells: Vec<(String, String)>,
}

/// A transaction that was read but deserves a look before it is imported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ambiguity {
    EmptyDescription {
        row: usize,
    },
    /// Both a debit and a credit on the same row.
    DebitAndCredit {
        row: usize,
        debit: Amount,
        credit: Amount,
    },
    /// More than two decimals; minor units round half away from zero.
    SubCents {
        row: usize,
        amount: Amount,
    },
    /// The profile reads dates day first, but no day above 12 confirms it.
    UnconfirmedDayMonthOrder,
}

/// Totals of one normalization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    /// Data rows read (the header excluded).
    pub rows: usize,
    pub transactions: usize,
    pub errors: usize,
    /// Layout rows: blank, header or footer lines, opening balances.
    pub structural: usize,
    /// Earliest and latest transaction date.
    pub date_range: Option<(String, String)>,
    pub currency: String,
    /// Sum of the negative amounts.
    pub debit_total: Amount,
    /// Sum of the positive amounts.
    pub credit_total: Amount,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Normalization {
    pub transactions: Vec<Transaction>,
    pub errors: Vec<RowError>,
    pub ambiguities: Vec<Ambiguity>,
    pub summary: Summary,
}

/// A file that cannot be normalized at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The bytes are not a readable file of the profile's format.
    Unreadable(String),
    /// Required columns are not in the file; `found` lists the headers it has.
    MissingColumns {
        missing: Vec<&'static str>,
        found: Vec<String>,
    },
    /// The profile's file format has no reader in this build.
    Unsupported(FileFormat),
    InvalidProfile(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unreadable(reason) => write!(f, "the file cannot be read: {reason}"),
            Error::MissingColumns { missing, found } => {
                write!(
                    f,
                    "columns not found: {}; the file has {found:?}",
                    missing.join(", ")
                )
            }
            Error::Unsupported(format) => write!(f, "{format:?} files are not supported yet"),
            Error::InvalidProfile(reason) => write!(f, "invalid profile: {reason}"),
        }
    }
}

impl std::error::Error for Error {}
