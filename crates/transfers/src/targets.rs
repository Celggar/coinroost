//! Pass 2: an outflow whose text names the account the money goes to. Banks print the target account number in
//! the description of a transfer out ("ONLINE TRANSFER 900555666001"), which tells apart two equal amounts sent the
//! same day to two different accounts.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::LazyLock;

use regex::Regex;

use crate::{Leg, Pair, Pass, TOLERANCE_DAYS, calendar_days, sorted};

/// Runs of 8 or more digits: shorter runs are amounts, dates and references.
static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d{8,}").expect("valid pattern"));

/// The runs of 8 or more digits in a text, in order, repeats included: candidate account numbers.
pub fn account_numbers(text: &str) -> impl Iterator<Item = &str> {
    NUMBER.find_iter(text).map(|m| m.as_str())
}

/// Learns `account number → account` from known truth: pairs of (outflow text, account the money went to). A
/// number is kept only when every piece of evidence points at the same account and there are at least two: a
/// number seen once may be a bank reference, not an account.
pub fn learn_targets<A: Eq + Hash + Clone>(known: &[(&str, A)]) -> HashMap<String, A> {
    let mut seen: HashMap<&str, HashMap<&A, u32>> = HashMap::new();
    for (text, account) in known {
        for number in account_numbers(text) {
            *seen.entry(number).or_default().entry(account).or_default() += 1;
        }
    }
    seen.into_iter()
        .filter_map(|(number, accounts)| {
            let mut only = accounts.into_iter();
            match (only.next(), only.next()) {
                (Some((account, count)), None) if count >= 2 => Some((number.to_owned(), account.clone())),
                _ => None,
            }
        })
        .collect()
}

/// Pairs each outflow among `within` that names exactly one known target account with the one free inflow of the
/// same amount in that account within [`TOLERANCE_DAYS`]. In date order, ties by id; an inflow is taken once.
pub(crate) fn pair_by_target<I: Ord, A: Eq + Hash + Clone>(
    legs: &[Leg<I, A>],
    within: &[usize],
    map: &HashMap<String, A>,
) -> Vec<Pair> {
    let (outflows, inflows) = sorted(legs, within);
    let mut taken = vec![false; legs.len()];
    let mut pairs = Vec::new();
    for &w in &outflows {
        let out = &legs[w];
        let mut targets = account_numbers(&out.text).filter_map(|n| map.get(n));
        let Some(target) = targets.next() else { continue };
        if targets.any(|other| other != target) || *target == out.account {
            continue;
        }
        let found: Vec<usize> = inflows
            .iter()
            .copied()
            .filter(|&d| {
                let inn = &legs[d];
                !taken[d]
                    && out.amount_minor.checked_neg() == Some(inn.amount_minor)
                    && inn.account == *target
                    && inn.currency == out.currency
                    && calendar_days(out.date, inn.date) <= TOLERANCE_DAYS
            })
            .collect();
        if let [d] = found.as_slice() {
            taken[*d] = true;
            pairs.push(Pair {
                outflow: w,
                inflow: *d,
                pass: Pass::Target,
            });
        }
    }
    pairs
}
