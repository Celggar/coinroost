//! Reading the fortnight a payment names in its description ("II Qna de Septiembre 2025", "Q1 Febrero"). The words
//! are locale data, matched on the description with accents, case and punctuation dropped.

use std::collections::HashMap;

use regex::Regex;
use serde::Deserialize;
use time::{Date, Duration};
use unicode_normalization::UnicodeNormalization;

use crate::Fortnight;

/// A label without a year reads with the payment's year, unless that puts it this far after the payment: nobody is
/// paid that far ahead, so a "II Qna Diciembre" paid in January is from the year before.
const MAX_ADVANCE_DAYS: i64 = 31;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Words {
    joiners: Vec<String>,
    year_joiners: Vec<String>,
    halves: HashMap<String, u8>,
    months: HashMap<String, u8>,
}

/// The words of one locale, compiled.
pub struct Labels {
    pattern: Regex,
    halves: HashMap<String, u8>,
    months: HashMap<String, u8>,
}

/// Alternatives for a pattern, longest first ("ii" before "i", "marzo" before "mar").
fn alternation<'a>(words: impl Iterator<Item = &'a String>) -> String {
    let mut words: Vec<&String> = words.collect();
    words.sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));
    words
        .iter()
        .map(|w| regex::escape(w))
        .collect::<Vec<_>>()
        .join("|")
}

impl Labels {
    /// The built-in words of a locale, such as `es-PA`.
    pub fn builtin(locale: &str) -> Option<Labels> {
        match locale {
            "es-PA" => Labels::from_toml(include_str!("../../../spec/locales/es-PA/payroll.toml")).ok(),
            _ => None,
        }
    }

    pub fn from_toml(text: &str) -> Result<Labels, String> {
        let words: Words = toml::from_str(text).map_err(|e| e.to_string())?;
        let bad = |w: &String| w.is_empty() || normalize(w) != *w;
        if words
            .halves
            .keys()
            .chain(words.months.keys())
            .chain(&words.joiners)
            .chain(&words.year_joiners)
            .any(bad)
        {
            return Err("every word is non-empty, in lower case, without accents or punctuation".to_owned());
        }
        if words.halves.values().any(|h| !(1..=2).contains(h))
            || words.months.values().any(|m| !(1..=12).contains(m))
        {
            return Err("a half is 1 or 2 and a month 1 to 12".to_owned());
        }
        // The half sits right before the month (with joiners between), so "xiii mes agosto" is not "ii agosto":
        // there is no word boundary between the x and the i.
        let pattern = format!(
            r"\b(?P<half>{})(?:\s+(?:{}))*\s+(?P<month>{})\b(?:\s+(?:(?:{})\s+)?(?P<year>20\d\d)\b)?",
            alternation(words.halves.keys()),
            alternation(words.joiners.iter()),
            alternation(words.months.keys()),
            alternation(words.year_joiners.iter()),
        );
        Ok(Labels {
            pattern: Regex::new(&pattern).map_err(|e| e.to_string())?,
            halves: words.halves,
            months: words.months,
        })
    }

    /// The fortnight a payment's description names, or none.
    pub fn parse(&self, description: &str, paid_on: Date) -> Option<Fortnight> {
        let text = normalize(description);
        let found = self.pattern.captures(&text)?;
        let half = self.halves[&found["half"]];
        let month = self.months[&found["month"]];
        if let Some(year) = found.name("year") {
            return Some(Fortnight::new(year.as_str().parse().ok()?, month, half));
        }
        let fortnight = Fortnight::new(paid_on.year(), month, half);
        if fortnight.first_day() > paid_on + Duration::days(MAX_ADVANCE_DAYS) {
            return Some(Fortnight::new(paid_on.year() - 1, month, half));
        }
        Some(fortnight)
    }
}

/// Lower case ASCII without accents; punctuation becomes a space and runs of spaces become one: "Sept." and "SEPT"
/// read the same.
pub fn normalize(text: &str) -> String {
    let plain: String = text
        .nfkd()
        .filter(char::is_ascii)
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c.is_ascii_whitespace() {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect();
    plain.split_ascii_whitespace().collect::<Vec<_>>().join(" ")
}
