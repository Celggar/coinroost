//! Which account a file belongs to, read from the file itself, so a batch of exports from several accounts
//! can be dropped at once.
//!
//! Profiles describe formats; account rules describe one household's accounts. A file matches an account
//! when its header text contains every detection text of the account's profile and every marker of the
//! account, and, when the account declares a type label or an account number hash, at least one of them.
//! Two cards that share a format are told apart by the card network prefix (ISO/IEC 7812: American Express
//! 34 and 37, Visa 4) of the masked card number. When the number is masked too much, the answer is
//! ambiguous: with money involved, nothing is guessed.

use regex::Regex;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::profile::{FileFormat, Profile};
use crate::text::{Encoding, decode, fold};

/// One account of the household and how to recognize its exports.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountRule {
    /// Account key; it goes into every `external_id` of the account.
    pub namespace: String,
    /// Id of the profile its exports use.
    pub profile: String,
    /// Texts that must also appear, such as the account name as the bank prints it.
    #[serde(default)]
    pub markers: Vec<String>,
    /// Card network prefixes, for cards that share a format.
    #[serde(default)]
    pub card_prefixes: Vec<String>,
    /// Account type label printed in the header.
    #[serde(default)]
    pub label: Option<String>,
    /// First 16 hex digits of the SHA-256 of the account number printed in the header. An unsalted hash
    /// of a short number can be reversed by brute force: keep it with the household's private settings.
    #[serde(default)]
    pub number_hash: Option<String>,
}

/// The account rules of a household, as `[[account]]` tables.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountRules {
    #[serde(default)]
    pub account: Vec<AccountRule>,
}

impl AccountRules {
    pub fn from_toml(text: &str) -> Result<AccountRules, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Detection {
    /// Exactly one account matched: its namespace.
    Found(String),
    /// Several accounts matched; the person chooses.
    Ambiguous(Vec<String>),
    NotFound,
}

/// The file format from the leading bytes, or from the extension when they say nothing.
pub fn sniff(bytes: &[u8], file_name: &str) -> FileFormat {
    if bytes.starts_with(b"%PDF") {
        return FileFormat::Pdf;
    }
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"\xd0\xcf\x11\xe0") {
        return FileFormat::Xlsx;
    }
    let extension = file_name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "pdf" => FileFormat::Pdf,
        "xlsx" | "xls" => FileFormat::Xlsx,
        _ => FileFormat::Csv,
    }
}

/// The text the rules are matched against: the whole file for CSV, the first page for PDF.
pub fn header_text(bytes: &[u8], format: FileFormat) -> Result<String, String> {
    match format {
        FileFormat::Csv => Ok(decode(bytes, Encoding::Utf8)),
        FileFormat::Pdf => crate::pdf::first_page_text(bytes),
        FileFormat::Xlsx => crate::xlsx::header_text(bytes),
    }
}

/// Detects the account of one file. A file that cannot be read is not found.
pub fn detect(bytes: &[u8], file_name: &str, profiles: &[Profile], accounts: &[AccountRule]) -> Detection {
    let format = sniff(bytes, file_name);
    match header_text(bytes, format) {
        Ok(text) => classify(&text, format, profiles, accounts),
        Err(_) => Detection::NotFound,
    }
}

/// Matches header text against the account rules whose profile has the given format.
pub fn classify(text: &str, format: FileFormat, profiles: &[Profile], accounts: &[AccountRule]) -> Detection {
    let folded = fold(text);
    let hashes = number_hashes(text);
    let mut candidates: Vec<&AccountRule> = accounts
        .iter()
        .filter(|account| {
            profiles
                .iter()
                .find(|p| p.id == account.profile)
                .is_some_and(|profile| {
                    profile.format == format && matches(profile, account, &folded, &hashes)
                })
        })
        .collect();
    let cards_only = candidates.len() > 1 && candidates.iter().all(|a| !a.card_prefixes.is_empty());
    if let Some(prefix) = card_prefix(text).filter(|_| cards_only) {
        let narrowed: Vec<&AccountRule> = candidates
            .iter()
            .copied()
            .filter(|a| a.card_prefixes.iter().any(|p| prefix.starts_with(p.as_str())))
            .collect();
        if !narrowed.is_empty() {
            candidates = narrowed;
        }
    }
    match candidates.as_slice() {
        [] => Detection::NotFound,
        [only] => Detection::Found(only.namespace.clone()),
        many => Detection::Ambiguous(many.iter().map(|a| a.namespace.clone()).collect()),
    }
}

fn matches(profile: &Profile, account: &AccountRule, folded: &str, hashes: &[String]) -> bool {
    let mut required = profile.detect.all.iter().chain(&account.markers).peekable();
    if required.peek().is_none() || !required.all(|text| folded.contains(&fold(text))) {
        return false;
    }
    if account.label.is_none() && account.number_hash.is_none() {
        return true;
    }
    let by_label = account
        .label
        .as_ref()
        .is_some_and(|label| folded.contains(&fold(label)));
    let by_hash = account
        .number_hash
        .as_ref()
        .is_some_and(|hash| hashes.iter().any(|h| h == hash));
    by_label || by_hash
}

/// Hashes of every run of 12 or more digits: account numbers, never short references.
fn number_hashes(text: &str) -> Vec<String> {
    static NUMBERS: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"\d{12,}").expect("valid pattern"));
    NUMBERS
        .find_iter(text)
        .map(|m| {
            let digest = Sha256::digest(m.as_str().as_bytes());
            digest
                .iter()
                .take(8)
                .fold(String::with_capacity(16), |mut hex, byte| {
                    use std::fmt::Write;
                    let _ = write!(hex, "{byte:02x}");
                    hex
                })
        })
        .collect()
}

/// The first four digits of a masked card number such as `3702-40**-****-9013`.
fn card_prefix(text: &str) -> Option<String> {
    static CARD: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"(\d{4})-\d{2}\*").expect("valid pattern"));
    CARD.captures(text).map(|c| c[1].to_owned())
}

#[cfg(test)]
mod tests {
    use super::{AccountRules, Detection, classify, sniff};
    use crate::profile::{FileFormat, Profile};

    const RULES: &str = r#"
        [[account]]
        namespace = "card_a"
        profile = "bac-card"
        card_prefixes = ["34", "37"]

        [[account]]
        namespace = "card_v"
        profile = "bac-card"
        card_prefixes = ["4"]

        [[account]]
        namespace = "coop_1"
        profile = "coopeduc"
        label = "ah corriente"

        [[account]]
        namespace = "coop_2"
        profile = "coopeduc"
        label = "ah navidad"
    "#;

    fn detect(text: &str, format: FileFormat) -> Detection {
        let rules = AccountRules::from_toml(RULES).unwrap();
        classify(text, format, &Profile::builtin(), &rules.account)
    }

    #[test]
    fn tells_cards_apart_by_network_prefix() {
        let header = "Product, Name, Date, Minimum payment/due date\n3702-40**-****-9013, JANE DOE";
        assert_eq!(detect(header, FileFormat::Csv), Detection::Found("card_a".into()));
        let visa = header.replace("3702-40", "4111-11");
        assert_eq!(detect(&visa, FileFormat::Csv), Detection::Found("card_v".into()));
        let masked = header.replace("3702-40**", "****-****");
        assert_eq!(
            detect(&masked, FileFormat::Csv),
            Detection::Ambiguous(vec!["card_a".into(), "card_v".into()])
        );
    }

    #[test]
    fn tells_shared_formats_apart_by_label_and_respects_the_format() {
        let header = "COOPERATIVA DE AHORRO Y CRÉDITO EL EDUCADOR\nNombre de la Cuenta: AH NAVIDAD";
        assert_eq!(detect(header, FileFormat::Pdf), Detection::Found("coop_2".into()));
        assert_eq!(detect(header, FileFormat::Csv), Detection::NotFound);
    }

    #[test]
    fn sniffs_by_magic_bytes_then_extension() {
        assert_eq!(sniff(b"%PDF-1.4", "x.csv"), FileFormat::Pdf);
        assert_eq!(sniff(b"PK\x03\x04", "x"), FileFormat::Xlsx);
        assert_eq!(sniff(b"a,b", "statement.XLSX"), FileFormat::Xlsx);
        assert_eq!(sniff(b"a,b", "statement"), FileFormat::Csv);
    }
}
