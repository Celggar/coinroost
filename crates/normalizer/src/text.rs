//! Text helpers with exact, documented semantics: decoding, whitespace, line splitting and the folded form
//! of a description that goes into the deduplication key.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::canonical_combining_class;

/// Unicode whitespace plus the information separators U+001C..U+001F, the set Python's `str.isspace`
/// accepts and therefore the set existing deduplication keys were stripped with.
pub fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

pub fn strip(text: &str) -> &str {
    text.trim_matches(is_space)
}

/// Splits on runs of whitespace, dropping empty pieces.
pub fn split_whitespace(text: &str) -> impl Iterator<Item = &str> {
    text.split(is_space).filter(|piece| !piece.is_empty())
}

/// Splits into lines on every line boundary Python's `str.splitlines` recognizes, without keeping the
/// terminators. A trailing terminator does not produce an empty last line.
pub fn split_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if !is_line_boundary(c) {
            continue;
        }
        lines.push(&text[start..at]);
        let mut end = at + c.len_utf8();
        if c == '\r' && chars.peek().is_some_and(|&(_, next)| next == '\n') {
            chars.next();
            end += 1;
        }
        start = end;
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

fn is_line_boundary(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r'
            | '\u{0b}'
            | '\u{0c}'
            | '\u{1c}'
            | '\u{1d}'
            | '\u{1e}'
            | '\u{85}'
            | '\u{2028}'
            | '\u{2029}'
    )
}

/// Compatibility decomposition, combining marks removed, whitespace stripped, lowercase. Used to compare
/// column names and to build the deduplication key, so `Débito`, `DEBITO` and ` debito ` are equal.
pub fn fold(text: &str) -> String {
    let decomposed: String = text
        .nfkd()
        .filter(|&c| canonical_combining_class(c) == 0)
        .collect();
    strip(&decomposed).to_lowercase()
}

/// A text encoding a bank profile can declare.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub enum Encoding {
    /// UTF-8, with or without a byte order mark.
    #[serde(rename = "utf-8")]
    Utf8,
    #[serde(rename = "windows-1252")]
    Windows1252,
}

/// Decodes with the declared encoding; bytes that are invalid in it fall back to windows-1252 and then to
/// ISO-8859-1, which accepts every byte. A file is never rejected for its encoding.
pub fn decode(bytes: &[u8], encoding: Encoding) -> String {
    if encoding == Encoding::Utf8 {
        let body = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
        if let Ok(text) = std::str::from_utf8(body) {
            return text.to_owned();
        }
    }
    decode_windows_1252(bytes).unwrap_or_else(|| bytes.iter().map(|&b| char::from(b)).collect())
}

/// Strict windows-1252: the five bytes the code page leaves undefined make it fail.
fn decode_windows_1252(bytes: &[u8]) -> Option<String> {
    bytes
        .iter()
        .map(|&b| match b {
            0x80..=0x9f => WINDOWS_1252_HIGH[usize::from(b - 0x80)],
            _ => Some(char::from(b)),
        })
        .collect()
}

const WINDOWS_1252_HIGH: [Option<char>; 32] = [
    Some('\u{20ac}'),
    None,
    Some('\u{201a}'),
    Some('\u{0192}'),
    Some('\u{201e}'),
    Some('\u{2026}'),
    Some('\u{2020}'),
    Some('\u{2021}'),
    Some('\u{02c6}'),
    Some('\u{2030}'),
    Some('\u{0160}'),
    Some('\u{2039}'),
    Some('\u{0152}'),
    None,
    Some('\u{017d}'),
    None,
    None,
    Some('\u{2018}'),
    Some('\u{2019}'),
    Some('\u{201c}'),
    Some('\u{201d}'),
    Some('\u{2022}'),
    Some('\u{2013}'),
    Some('\u{2014}'),
    Some('\u{02dc}'),
    Some('\u{2122}'),
    Some('\u{0161}'),
    Some('\u{203a}'),
    Some('\u{0153}'),
    None,
    Some('\u{017e}'),
    Some('\u{0178}'),
];

#[cfg(test)]
mod tests {
    use super::{Encoding, decode, fold, split_lines, strip};

    #[test]
    fn folds_accents_case_and_spaces() {
        assert_eq!(fold(" Débito de Transacción "), "debito de transaccion");
        assert_eq!(fold("ﬁ"), "fi");
    }

    #[test]
    fn strips_the_information_separators_too() {
        assert_eq!(strip("\u{1f}\u{a0} PAGO \u{3000}"), "PAGO");
    }

    #[test]
    fn splits_lines_on_every_boundary() {
        assert_eq!(split_lines("a\r\nb\rc\nd\u{2028}e\n"), ["a", "b", "c", "d", "e"]);
        assert_eq!(split_lines("a\n\nb"), ["a", "", "b"]);
    }

    #[test]
    fn decoding_never_fails() {
        assert_eq!(decode(b"\xef\xbb\xbfPE\xc3\x91A", Encoding::Utf8), "PEÑA");
        assert_eq!(decode(b"PE\xd1A \x80", Encoding::Utf8), "PEÑA €");
        assert_eq!(decode(b"x\x81\x93", Encoding::Windows1252), "x\u{81}\u{93}");
    }
}
