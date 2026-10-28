//! The deduplication key.

use std::fmt::Write;

use sha2::{Digest, Sha256};

use crate::text::fold;

/// First 32 hex digits of SHA-256 over `namespace|date|amount|description|occurrence`, with the
/// description folded (see [`fold`]) and the amount as written (`12.50`, not `12.5`).
///
/// `namespace` identifies the account, so the same purchase on two cards gets two keys. `occurrence`
/// counts identical `(date, amount, description)` rows within one file, starting at 1: two equal coffees
/// on the same day are two transactions, and reordering a file does not change its set of keys.
pub fn external_id(
    namespace: &str,
    date: &str,
    amount: &str,
    description: &str,
    occurrence: usize,
) -> String {
    let base = format!("{namespace}|{date}|{amount}|{}|{occurrence}", fold(description));
    let digest = Sha256::digest(base.as_bytes());
    digest
        .iter()
        .take(16)
        .fold(String::with_capacity(32), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

#[cfg(test)]
mod tests {
    use super::external_id;

    #[test]
    fn hashes_the_documented_text() {
        // sha256("bac|2026-06-02|-45.50|supermercado xtra|1")
        assert_eq!(
            external_id("bac", "2026-06-02", "-45.50", " SUPERMERCADO XTRA ", 1),
            "ce8a9af66863e6e7053dcf19c703d8a2"
        );
    }
}
