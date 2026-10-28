//! Decimal amounts that keep the scale they were written with.
//!
//! The deduplication key hashes the amount as text, so `12.5` and `12.50` must stay different values with
//! different text. Parsing, negation, addition and formatting follow the General Decimal Arithmetic
//! specification (the semantics of Python's `decimal` module with its default context), which is what
//! existing `external_id` values were computed with.

use std::cmp::Ordering;
use std::fmt;
use std::ops::Add;

/// A finite decimal number: `(-1)^negative × coefficient × 10^exponent`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Amount {
    negative: bool,
    coefficient: u128,
    exponent: i32,
}

impl Amount {
    /// Zero with exponent 0, written `0`.
    pub const ZERO: Amount = Amount {
        negative: false,
        coefficient: 0,
        exponent: 0,
    };

    /// Parses decimal text such as `1234.50`, `.5`, `7.` or `1e3`. Signs must already be removed by the
    /// caller; `NaN` and infinities are rejected.
    pub fn parse(text: &str) -> Option<Amount> {
        let text = text.trim_matches(|c: char| c.is_ascii_whitespace());
        let (mantissa, exponent_part) = match text.find(['e', 'E']) {
            Some(at) => (&text[..at], Some(&text[at + 1..])),
            None => (text, None),
        };
        let (int_part, fraction_part) = match mantissa.find('.') {
            Some(at) => (&mantissa[..at], &mantissa[at + 1..]),
            None => (mantissa, ""),
        };
        let int_digits = digits(int_part)?;
        let fraction_digits = digits(fraction_part)?;
        if int_digits.is_empty() && fraction_digits.is_empty() {
            return None;
        }
        let mut exponent = -i32::try_from(fraction_digits.len()).ok()?;
        if let Some(part) = exponent_part {
            exponent = exponent.checked_add(parse_exponent(part)?)?;
        }
        let all = format!("{int_digits}{fraction_digits}");
        let coefficient = all.parse::<u128>().ok()?;
        Some(Amount {
            negative: false,
            coefficient,
            exponent,
        })
    }

    pub fn is_zero(self) -> bool {
        self.coefficient == 0
    }

    pub fn is_negative(self) -> bool {
        self.negative && !self.is_zero()
    }

    /// Unary minus. A zero result is always positive.
    #[must_use]
    pub fn negate(self) -> Amount {
        if self.is_zero() {
            return Amount {
                negative: false,
                ..self
            };
        }
        Amount {
            negative: !self.negative,
            ..self
        }
    }

    /// Multiplication by an integer sign (`1` or `-1`). Unlike [`Amount::negate`], `-1 × 0` is `-0`.
    #[must_use]
    pub fn times_sign(self, sign: i8) -> Amount {
        Amount {
            negative: self.negative != (sign < 0),
            ..self
        }
    }

    /// The amount in cents (minor units for a two-decimal currency), rounding half away from zero.
    pub fn minor_units(self) -> i64 {
        let shift = self.exponent + 2;
        let magnitude = if shift >= 0 {
            self.coefficient * 10u128.pow(shift.unsigned_abs())
        } else {
            let divisor = 10u128.pow(shift.unsigned_abs());
            let (quotient, remainder) = (self.coefficient / divisor, self.coefficient % divisor);
            quotient + u128::from(remainder * 2 >= divisor)
        };
        let magnitude = i64::try_from(magnitude).unwrap_or(i64::MAX);
        if self.negative { -magnitude } else { magnitude }
    }

    /// True when the value is not a whole number of cents (it would be rounded).
    pub fn has_sub_cents(self) -> bool {
        if self.exponent >= -2 {
            return false;
        }
        !self
            .coefficient
            .is_multiple_of(10u128.pow((-2 - self.exponent).unsigned_abs()))
    }

    fn coefficient_at(self, exponent: i32) -> u128 {
        self.coefficient * 10u128.pow((self.exponent - exponent).unsigned_abs())
    }
}

impl Add for Amount {
    type Output = Amount;

    /// Exact addition at the smaller of the two exponents.
    fn add(self, other: Amount) -> Amount {
        let exponent = self.exponent.min(other.exponent);
        let a = self.coefficient_at(exponent);
        let b = other.coefficient_at(exponent);
        if self.negative == other.negative {
            return Amount {
                negative: self.negative,
                coefficient: a + b,
                exponent,
            };
        }
        match a.cmp(&b) {
            Ordering::Equal => Amount {
                negative: false,
                coefficient: 0,
                exponent,
            },
            Ordering::Greater => Amount {
                negative: self.negative,
                coefficient: a - b,
                exponent,
            },
            Ordering::Less => Amount {
                negative: other.negative,
                coefficient: b - a,
                exponent,
            },
        }
    }
}

impl fmt::Display for Amount {
    /// Scientific notation only when the exponent is positive or the value is very small, as the
    /// specification's to-scientific-string does.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let digits = self.coefficient.to_string();
        let length = i64::try_from(digits.len()).unwrap_or(i64::MAX);
        let left_digits = i64::from(self.exponent) + length;
        let dot = if self.exponent <= 0 && left_digits > -6 {
            left_digits
        } else {
            1
        };
        let sign = if self.negative { "-" } else { "" };
        let body = if dot <= 0 {
            format!("0.{}{digits}", "0".repeat(usize::try_from(-dot).unwrap_or(0)))
        } else if dot >= length {
            format!(
                "{digits}{}",
                "0".repeat(usize::try_from(dot - length).unwrap_or(0))
            )
        } else {
            let (int, fraction) = digits.split_at(usize::try_from(dot).unwrap_or(0));
            format!("{int}.{fraction}")
        };
        let exponent = if left_digits == dot {
            String::new()
        } else {
            format!("E{:+}", left_digits - dot)
        };
        write!(f, "{sign}{body}{exponent}")
    }
}

/// ASCII digits with optional single underscores between them, returned without the underscores.
fn digits(part: &str) -> Option<String> {
    let mut out = String::with_capacity(part.len());
    let mut previous_digit = false;
    for c in part.chars() {
        if c.is_ascii_digit() {
            out.push(c);
            previous_digit = true;
        } else if c == '_' && previous_digit {
            previous_digit = false;
        } else {
            return None;
        }
    }
    if !part.is_empty() && !previous_digit {
        return None;
    }
    Some(out)
}

fn parse_exponent(part: &str) -> Option<i32> {
    let (negative, body) = match part.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, part.strip_prefix('+').unwrap_or(part)),
    };
    let value = digits(body).filter(|d| !d.is_empty())?.parse::<i32>().ok()?;
    Some(if negative { -value } else { value })
}

#[cfg(test)]
mod tests {
    use super::Amount;

    fn amount(text: &str) -> Amount {
        Amount::parse(text).expect(text)
    }

    #[test]
    fn keeps_the_written_scale() {
        assert_eq!(amount("12.50").to_string(), "12.50");
        assert_eq!(amount("12.5").to_string(), "12.5");
        assert_eq!(amount("007.50").to_string(), "7.50");
        assert_eq!(amount(".5").to_string(), "0.5");
        assert_eq!(amount("5.").to_string(), "5");
        assert_eq!(amount("1e3").to_string(), "1E+3");
        assert_eq!(amount("0.0000001").to_string(), "1E-7");
    }

    #[test]
    fn signed_zero_follows_the_specification() {
        let zero = Amount::ZERO;
        assert_eq!(zero.times_sign(-1).to_string(), "-0");
        assert_eq!((zero + zero.times_sign(-1)).to_string(), "0");
        assert_eq!(amount("0.00").negate().to_string(), "0.00");
        assert_eq!((zero + amount("12.50").times_sign(-1)).to_string(), "-12.50");
        assert_eq!(
            (amount("12.50") + amount("12.5").times_sign(-1)).to_string(),
            "0.00"
        );
    }

    #[test]
    fn rounds_cents_half_away_from_zero() {
        assert_eq!(amount("0.125").minor_units(), 13);
        assert_eq!(amount("0.125").negate().minor_units(), -13);
        assert_eq!(amount("1234.5").minor_units(), 123_450);
        assert!(amount("0.125").has_sub_cents());
        assert!(!amount("0.120").has_sub_cents());
    }

    #[test]
    fn rejects_what_is_not_a_number() {
        for text in ["", ".", "1.2.3", "nan", "inf", "1__0", "_1", "1,0"] {
            assert!(Amount::parse(text).is_none(), "{text}");
        }
    }
}
