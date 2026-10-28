//! Booking dates. Formats use the C `strptime` directives a profile can need: `%d %m %Y %H %M %S`.
//!
//! Matching follows Python's `datetime.strptime`: the format becomes a pattern whose alternatives prefer
//! two-digit fields, a run of whitespace in the format matches any run of whitespace, the first match must
//! cover the whole value, and the resulting date and time must exist.

use regex::Regex;

use crate::text::fold;

/// A compiled date format.
#[derive(Debug, Clone)]
pub struct DateFormat {
    pattern: Regex,
    /// The format leads with the day (`%d` before `%m`), so a day above 12 confirms the order.
    pub day_first: bool,
}

impl DateFormat {
    pub fn new(format: &str) -> Result<DateFormat, String> {
        let mut pattern = String::from("^");
        let mut chars = format.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '%' {
                let directive = chars
                    .next()
                    .ok_or_else(|| format!("format ends with '%': {format}"))?;
                pattern.push_str(
                    directive_pattern(directive)
                        .ok_or_else(|| format!("unsupported directive %{directive} in {format}"))?,
                );
            } else if c.is_whitespace() {
                while chars.peek().is_some_and(|next| next.is_whitespace()) {
                    chars.next();
                }
                pattern.push_str(r"\s+");
            } else {
                pattern.push_str(&regex::escape(&c.to_string()));
            }
        }
        let day = format.find("%d");
        let month = format.find("%m");
        let day_first = matches!((day, month), (Some(d), Some(m)) if d < m);
        let pattern = Regex::new(&format!("(?i){pattern}")).map_err(|e| e.to_string())?;
        Ok(DateFormat { pattern, day_first })
    }

    /// The ISO 8601 date (`YYYY-MM-DD`) when the whole value matches and the date exists.
    pub fn parse(&self, value: &str) -> Option<String> {
        let captures = self.pattern.captures(value)?;
        if captures.get(0)?.end() != value.len() {
            return None;
        }
        let field = |name: &str, default: u32| {
            captures
                .name(name)
                .map_or(Some(default), |m| m.as_str().trim_start().parse::<u32>().ok())
        };
        let (year, month, day) = (field("Y", 1900)?, field("m", 1)?, field("d", 1)?);
        let (hour, minute, second) = (field("H", 0)?, field("M", 0)?, field("S", 0)?);
        if year == 0 || !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
            return None;
        }
        if hour > 23 || minute > 59 || second > 59 {
            return None;
        }
        Some(format!("{year:04}-{month:02}-{day:02}"))
    }
}

fn directive_pattern(directive: char) -> Option<&'static str> {
    Some(match directive {
        'd' => r"(?P<d>3[0-1]|[1-2]\d|0[1-9]|[1-9]| [1-9])",
        'm' => r"(?P<m>1[0-2]|0[1-9]|[1-9])",
        'Y' => r"(?P<Y>\d\d\d\d)",
        'H' => r"(?P<H>2[0-3]|[0-1]\d|\d)",
        'M' => r"(?P<M>[0-5]\d|\d)",
        'S' => r"(?P<S>6[0-1]|[0-5]\d|\d)",
        '%' => "%",
        _ => return None,
    })
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Tries each format in order. Dates written with a Spanish month abbreviation (`20-jun-2026`) are also
/// tried with the month as a number (`20-06-2026`).
pub fn parse_date(value: &str, formats: &[DateFormat]) -> Option<String> {
    let value = crate::text::strip(value);
    if value.is_empty() {
        return None;
    }
    let numeric = spanish_month_to_number(value);
    std::iter::once(value)
        .chain(numeric.as_deref())
        .find_map(|candidate| formats.iter().find_map(|format| format.parse(candidate)))
}

fn spanish_month_to_number(value: &str) -> Option<String> {
    static PATTERN: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r"^(\d{1,2})[- ]([A-Za-z\u{c1}-\u{fa}ñÑ]{3})\.?[- ](\d{4})\n?$").expect("valid pattern")
    });
    let captures = PATTERN.captures(value)?;
    let month = match fold(&captures[2]).as_str() {
        "ene" => "01",
        "feb" => "02",
        "mar" => "03",
        "abr" => "04",
        "may" => "05",
        "jun" => "06",
        "jul" => "07",
        "ago" => "08",
        "sep" | "set" => "09",
        "oct" => "10",
        "nov" => "11",
        "dic" => "12",
        _ => return None,
    };
    Some(format!("{}-{month}-{}", &captures[1], &captures[3]))
}

/// The first run of digits as a number, used to notice a day above 12.
pub fn first_number(value: &str) -> Option<u64> {
    let start = value.find(|c: char| c.is_ascii_digit())?;
    let digits: String = value[start..].chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{DateFormat, parse_date};

    fn formats(list: &[&str]) -> Vec<DateFormat> {
        list.iter().map(|f| DateFormat::new(f).expect(f)).collect()
    }

    #[test]
    fn parses_day_first_and_timestamps() {
        let day_first = formats(&["%d/%m/%Y"]);
        assert_eq!(
            parse_date("15/06/2026", &day_first).as_deref(),
            Some("2026-06-15")
        );
        assert_eq!(parse_date("1/5/2026", &day_first).as_deref(), Some("2026-05-01"));
        assert_eq!(parse_date("31/02/2026", &day_first), None);
        assert_eq!(parse_date("15/06/2026 x", &day_first), None);
        let stamped = formats(&["%Y-%m-%d %H:%M:%S", "%Y-%m-%d"]);
        assert_eq!(
            parse_date("2026-06-26 23:49:57", &stamped).as_deref(),
            Some("2026-06-26")
        );
        assert_eq!(parse_date("2026-06-26", &stamped).as_deref(), Some("2026-06-26"));
    }

    #[test]
    fn translates_spanish_month_abbreviations() {
        let numeric = formats(&["%d-%m-%Y"]);
        assert_eq!(parse_date("20-jun-2026", &numeric).as_deref(), Some("2026-06-20"));
        assert_eq!(parse_date("13-DIC-2024", &numeric).as_deref(), Some("2024-12-13"));
        assert_eq!(parse_date("xx-zzz-2026", &numeric), None);
    }
}
