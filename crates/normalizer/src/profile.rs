//! Bank profiles: how to read one bank's export, declared as data in `spec/profiles/*.toml`.

use serde::Deserialize;

use crate::date::DateFormat;
use crate::text::Encoding;

/// The file format of an export.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileFormat {
    Csv,
    Xlsx,
    Pdf,
}

/// Candidate names for each column, compared folded (no accents, no case) and, failing an exact match, by
/// containment. A profile reads either one signed `amount` column or separate `debit` and `credit`
/// columns; when it declares both, each file uses the scheme its columns support, debit/credit first.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Columns {
    #[serde(default)]
    pub date: Vec<String>,
    #[serde(default)]
    pub description: Vec<String>,
    #[serde(default)]
    pub amount: Vec<String>,
    #[serde(default)]
    pub debit: Vec<String>,
    #[serde(default)]
    pub credit: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Amounts {
    /// Decimal separator as printed by the bank.
    pub decimal: String,
    /// Thousands separator, removed before parsing.
    pub thousands: String,
    /// `amount = credit + debit_sign × debit`: `-1` when debits are printed as positive magnitudes, `1`
    /// when the export already prints them negative.
    #[serde(default = "minus_one")]
    pub debit_sign: i8,
    /// Card statements print purchases positive; this flips them so money leaving is negative.
    #[serde(default)]
    pub invert_sign: bool,
}

fn minus_one() -> i8 {
    -1
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Csv {
    pub encoding: Encoding,
    pub delimiter: char,
    /// Ignore spaces right after each delimiter.
    #[serde(default)]
    pub skip_initial_space: bool,
    /// Text on the header line of the transactions table, for files with several sections. The table runs
    /// until the first blank line (or a line holding only delimiters and spaces).
    #[serde(default)]
    pub section_start: Option<String>,
    /// Text on the first line after the table, for files where no blank line closes it.
    #[serde(default)]
    pub section_end: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Xlsx {
    /// Text in a cell of the header row, for sheets with metadata rows above the table. Without it, or when
    /// no row contains it, the first row is the header.
    #[serde(default)]
    pub header_marker: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PdfStrategy {
    /// One transaction per text line, split into fields by a regular expression.
    Lines,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pdf {
    pub strategy: PdfStrategy,
    /// A line that starts a transaction. Such a line that does not match `row` is reported, not dropped.
    pub row_start: String,
    /// Named groups become columns: `date`, `description`, and `amount` or `debit`/`credit`.
    pub row: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dates {
    /// Tried in order; the first is the bank's canonical format.
    pub formats: Vec<String>,
}

/// Text that identifies the format in a file's header (see [`crate::detect`]).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detect {
    /// Every entry must appear, folded, in the header text.
    #[serde(default)]
    pub all: Vec<String>,
}

/// One bank export format.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// Stable identifier of the format, such as `bac-account`.
    pub id: String,
    pub name: String,
    pub format: FileFormat,
    /// ISO 4217 code.
    pub currency: String,
    #[serde(default)]
    pub detect: Detect,
    #[serde(default)]
    pub columns: Columns,
    pub amounts: Amounts,
    pub dates: Dates,
    #[serde(default)]
    pub csv: Option<Csv>,
    #[serde(default)]
    pub xlsx: Option<Xlsx>,
    #[serde(default)]
    pub pdf: Option<Pdf>,
}

/// The profiles shipped with the library, one per file in `spec/profiles/`.
const BUILTIN: &[&str] = &[
    include_str!("../../../spec/profiles/bac-account.toml"),
    include_str!("../../../spec/profiles/bac-card.toml"),
    include_str!("../../../spec/profiles/bg-account.toml"),
    include_str!("../../../spec/profiles/coopeduc.toml"),
];

impl Profile {
    pub fn from_toml(text: &str) -> Result<Profile, String> {
        let profile: Profile = toml::from_str(text).map_err(|e| e.to_string())?;
        profile.validate()?;
        Ok(profile)
    }

    /// Every built-in profile.
    ///
    /// # Panics
    ///
    /// Never at run time: the built-in profiles are checked by the test suite.
    pub fn builtin() -> Vec<Profile> {
        BUILTIN
            .iter()
            .map(|text| Profile::from_toml(text).expect("built-in profile is valid"))
            .collect()
    }

    /// A built-in profile by id.
    pub fn find(id: &str) -> Option<Profile> {
        Profile::builtin().into_iter().find(|p| p.id == id)
    }

    pub(crate) fn date_formats(&self) -> Result<Vec<DateFormat>, String> {
        self.dates.formats.iter().map(|f| DateFormat::new(f)).collect()
    }

    fn validate(&self) -> Result<(), String> {
        if self.dates.formats.is_empty() {
            return Err(format!("{}: at least one date format is required", self.id));
        }
        self.date_formats()?;
        if !matches!(self.amounts.debit_sign, -1 | 1) {
            return Err(format!("{}: debit_sign must be 1 or -1", self.id));
        }
        match self.format {
            FileFormat::Csv if self.csv.is_none() => Err(format!("{}: a csv profile needs [csv]", self.id)),
            FileFormat::Pdf if self.pdf.is_none() => Err(format!("{}: a pdf profile needs [pdf]", self.id)),
            FileFormat::Xlsx if self.csv.is_some() || self.pdf.is_some() => {
                Err(format!("{}: an xlsx profile takes only [xlsx]", self.id))
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Profile;

    #[test]
    fn every_builtin_profile_loads() {
        let ids: Vec<String> = Profile::builtin().into_iter().map(|p| p.id).collect();
        assert_eq!(ids, ["bac-account", "bac-card", "bg-account", "coopeduc"]);
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let text =
            include_str!("../../../spec/profiles/bac-account.toml").replace("[amounts]", "[amounts]\nx = 1");
        assert!(Profile::from_toml(&text).is_err());
    }
}
