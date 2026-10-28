//! Firefly amounts: decimal strings, sometimes padded with zeros ("45.500000000000"), read into minor units.

/// `text` in minor units of a currency with `digits` decimals. Zeros past the currency's decimals are padding; any
/// other digit there is refused, never rounded.
pub fn minor_units(text: &str, digits: u32) -> Result<i64, String> {
    let text = text.trim();
    let refuse = || format!("{text:?} is not an amount with at most {digits} decimals");
    let (negative, unsigned) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let digit_only = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if whole.is_empty() || !digit_only(whole) || !digit_only(fraction) {
        return Err(refuse());
    }
    let digits = usize::try_from(digits).map_err(|_| refuse())?;
    let (kept, rest) = fraction.split_at(fraction.len().min(digits));
    if rest.bytes().any(|b| b != b'0') {
        return Err(refuse());
    }
    let minor: i64 = format!("{whole}{kept:0<digits$}").parse().map_err(|_| refuse())?;
    Ok(if negative { -minor } else { minor })
}

#[cfg(test)]
mod tests {
    use super::minor_units;

    #[test]
    fn padding_is_read_and_extra_digits_are_refused() {
        assert_eq!(minor_units("45.500000000000", 2), Ok(4550));
        assert_eq!(minor_units("1500.000000000000", 0), Ok(1500));
        assert_eq!(minor_units("-170.00", 2), Ok(-17000));
        assert_eq!(minor_units("0.5", 2), Ok(50));
        assert_eq!(minor_units("12", 2), Ok(1200));
        for bad in ["45.505", "1.5", "", "-", "1e3", "12.3.4", " -.5", "1,000.00"] {
            let digits = if bad == "1.5" { 0 } else { 2 };
            assert!(minor_units(bad, digits).is_err(), "{bad:?} should be refused");
        }
    }
}
