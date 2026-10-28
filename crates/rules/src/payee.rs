//! The payee named in a description after a payment channel. A channel (a mobile wallet, online banking, an
//! ACH transfer) says how the money moved; the person or shop it went to comes after it, and changes with every
//! transaction. Grouping them all under the channel's name would hide who was paid.

use regex::Regex;
use serde::Deserialize;

/// The words of one locale's statements that are never part of a payee's name (`spec/locales/<locale>/payees.toml`).
#[derive(Debug)]
pub struct PayeeText {
    leading: Option<Regex>,
    banks: Option<Regex>,
    number: Regex,
    cut: Option<Regex>,
    other: Regex,
    max_words: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    /// Characters a name is made of, as the body of a regular expression class, besides the space.
    letters: String,
    leading: Vec<String>,
    cut_from: Vec<String>,
    banks: Vec<String>,
    max_words: usize,
}

const ES_PA: &str = include_str!("../../../spec/locales/es-PA/payees.toml");

fn any_word(words: &[String], pattern: impl Fn(&str) -> String) -> Result<Option<Regex>, String> {
    if words.is_empty() {
        return Ok(None);
    }
    let alternatives: Vec<String> = words.iter().map(|w| regex::escape(w)).collect();
    Regex::new(&pattern(&alternatives.join("|")))
        .map(Some)
        .map_err(|e| e.to_string())
}

impl PayeeText {
    /// The built-in locale: `es-PA` is the only one so far.
    pub fn builtin(locale: &str) -> Option<PayeeText> {
        match locale {
            "es-PA" => PayeeText::from_toml(ES_PA).ok(),
            _ => None,
        }
    }

    pub fn from_toml(text: &str) -> Result<PayeeText, String> {
        let spec: Spec = toml::from_str(text).map_err(|e| e.to_string())?;
        if spec.max_words == 0 {
            return Err("max_words must be at least 1".to_owned());
        }
        Ok(PayeeText {
            leading: any_word(&spec.leading, |w| format!(r"(?i)^[\s:]*(?:{w})\b"))?,
            banks: any_word(&spec.banks, |w| format!(r"(?i)\b(?:{w})\b"))?,
            number: Regex::new(r"\d[\d\-\*]{3,}").map_err(|e| e.to_string())?,
            cut: any_word(&spec.cut_from, |w| format!(r"(?i)\b(?:{w})\b.*"))?,
            other: Regex::new(&format!("[^{} ]", spec.letters)).map_err(|e| e.to_string())?,
            max_words: spec.max_words,
        })
    }

    /// The name after `channel` in `description`, or `None` when nothing readable is left.
    pub fn name(&self, description: &str, channel: &str) -> Option<String> {
        let mut text = description
            .to_uppercase()
            .replacen(&channel.to_uppercase(), " ", 1);
        if let Some(leading) = &self.leading {
            text = leading.replace(&text, " ").into_owned();
        }
        if let Some(banks) = &self.banks {
            text = banks.replace_all(&text, " ").into_owned();
        }
        text = self.number.replace_all(&text, " ").into_owned();
        if let Some(cut) = &self.cut {
            text = cut.replace_all(&text, " ").into_owned();
        }
        text = self.other.replace_all(&text, " ").into_owned();
        let words: Vec<&str> = text
            .split_whitespace()
            .filter(|w| w.chars().count() > 1)
            .take(self.max_words)
            .collect();
        (!words.is_empty()).then(|| words.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic descriptions with the payee each one names (`spec/fixtures/payees/`).
    #[test]
    fn reads_the_person_after_the_channel() {
        let text = PayeeText::builtin("es-PA").unwrap();
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../../../spec/fixtures/payees/es-PA.json")).unwrap();
        assert_ne!(cases.len(), 0);
        for case in cases {
            let description = case["description"].as_str().unwrap();
            let name = text.name(description, case["channel"].as_str().unwrap());
            assert_eq!(name.as_deref(), case["payee"].as_str(), "{description}");
        }
    }
}
