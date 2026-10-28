//! Transfer pairing. Each bank statement is imported on its own, so money moved between two own accounts arrives
//! twice: an outflow in one account and an inflow in the other. Pairing them into one transfer keeps every balance
//! as it was (the same two legs exist) and stops the movement from counting as spending and as income.
//!
//! Only what is certain is paired. A leg with no counterpart (its other statement was never imported) or with
//! several possible counterparts is left alone and reported: with money involved, nothing is guessed.
//!
//! Three passes, in order:
//! 1. Same amount, opposite sign, different accounts, same currency, dates at most [`TOLERANCE_DAYS`] apart, and
//!    the match must be mutually unique: the outflow has exactly one candidate and that inflow has exactly one.
//!    Mutual uniqueness makes the result independent of the input order.
//! 2. For what pass 1 left, an outflow whose text names its target account (a run of 8 or more digits) pairs with
//!    the one free inflow of that amount in that account. The number → account map is learned from known truth
//!    (pass 1 pairs and transfers already made) and needs two consistent pieces of evidence; it is never stored.
//! 3. Pass 1 orphans that pass 2 did not take are tried again with business days, for a transfer sent on a Friday
//!    that arrives on Monday.
//!
//! Pure: no database and no clock. Legs are identified by the caller's ids; the plan refers to them by their
//! position in the input.

mod targets;

use std::hash::Hash;

use time::{Date, Weekday};

pub use targets::{account_numbers, learn_targets};

/// Most days allowed between the two legs. Measured on real data: a wider window finds fewer pairs, because more
/// candidates break mutual uniqueness.
pub const TOLERANCE_DAYS: u32 = 2;

/// One leg of a possible transfer: a single-leg transaction in an own account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Leg<I, A> {
    pub id: I,
    pub date: Date,
    /// Below zero the money leaves the account; zero or above it comes in.
    pub amount_minor: i64,
    pub currency: String,
    pub account: A,
    /// What the bank printed.
    pub text: String,
}

impl<I, A> Leg<I, A> {
    fn is_outflow(&self) -> bool {
        self.amount_minor < 0
    }
}

/// Which pass found a pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pass {
    /// Mutually unique within [`TOLERANCE_DAYS`] calendar days.
    Unique = 1,
    /// The outflow's text names the target account.
    Target = 2,
    /// Mutually unique within [`TOLERANCE_DAYS`] business days, among pass 1 orphans.
    BusinessDays = 3,
}

/// Two legs of the same movement, as positions in the input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pair {
    pub outflow: usize,
    pub inflow: usize,
    pub pass: Pass,
}

/// What pairing would do. Orphans and ambiguous legs are reported, never changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub pairs: Vec<Pair>,
    /// Legs with no counterpart at all.
    pub orphans: Vec<usize>,
    /// Legs with more than one possible counterpart, or whose counterpart has more than one; with the candidates
    /// pass 1 found (some may have been paired since).
    pub ambiguous: Vec<(usize, Vec<usize>)>,
}

/// Pairs what is certain among `legs`. `evidence` is known truth from transfers already made: the outflow's text
/// and the account the money went to.
pub fn plan<I: Ord, A: Eq + Hash + Clone>(legs: &[Leg<I, A>], evidence: &[(&str, A)]) -> Plan {
    let all: Vec<usize> = (0..legs.len()).collect();
    let first = pair_up(legs, &all, calendar_days, Pass::Unique);
    let mut used = vec![false; legs.len()];
    for p in &first.pairs {
        used[p.outflow] = true;
        used[p.inflow] = true;
    }

    let mut known: Vec<(&str, A)> = first
        .pairs
        .iter()
        .map(|p| (legs[p.outflow].text.as_str(), legs[p.inflow].account.clone()))
        .collect();
    known.extend(evidence.iter().cloned());
    let map = learn_targets(&known);
    let left: Vec<usize> = all.iter().copied().filter(|&i| !used[i]).collect();
    let second = targets::pair_by_target(legs, &left, &map);
    for p in &second {
        used[p.outflow] = true;
        used[p.inflow] = true;
    }

    let orphans: Vec<usize> = first.orphans.iter().copied().filter(|&i| !used[i]).collect();
    let third = pair_up(legs, &orphans, business_days, Pass::BusinessDays).pairs;
    for p in &third {
        used[p.outflow] = true;
        used[p.inflow] = true;
    }

    Plan {
        pairs: first.pairs.into_iter().chain(second).chain(third).collect(),
        orphans: first.orphans.into_iter().filter(|&i| !used[i]).collect(),
        ambiguous: first.ambiguous.into_iter().filter(|(i, _)| !used[*i]).collect(),
    }
}

/// Positions of `legs` among `within` in date order, ties by id, split into outflows and inflows.
fn sorted<I: Ord, A>(legs: &[Leg<I, A>], within: &[usize]) -> (Vec<usize>, Vec<usize>) {
    let mut order = within.to_vec();
    order.sort_by(|&a, &b| (legs[a].date, &legs[a].id).cmp(&(legs[b].date, &legs[b].id)));
    order.into_iter().partition(|&i| legs[i].is_outflow())
}

fn matches<I, A: Eq>(out: &Leg<I, A>, inn: &Leg<I, A>, days: fn(Date, Date) -> u32) -> bool {
    out.amount_minor.checked_neg() == Some(inn.amount_minor)
        && out.account != inn.account
        && out.currency == inn.currency
        && days(out.date, inn.date) <= TOLERANCE_DAYS
}

/// Pass 1 (and pass 3 with business days): mutually unique matches among `within`.
fn pair_up<I: Ord, A: Eq>(
    legs: &[Leg<I, A>],
    within: &[usize],
    days: fn(Date, Date) -> u32,
    pass: Pass,
) -> Plan {
    let (outflows, inflows) = sorted(legs, within);
    let candidates = |of: usize, among: &[usize]| -> Vec<usize> {
        among
            .iter()
            .copied()
            .filter(|&j| {
                let (out, inn) = if legs[of].is_outflow() { (of, j) } else { (j, of) };
                matches(&legs[out], &legs[inn], days)
            })
            .collect()
    };
    let mut plan = Plan::default();
    let mut taken = vec![false; legs.len()];
    for &w in &outflows {
        let found = candidates(w, &inflows);
        match found.as_slice() {
            [] => plan.orphans.push(w),
            [d] if candidates(*d, &outflows).len() == 1 => {
                plan.pairs.push(Pair {
                    outflow: w,
                    inflow: *d,
                    pass,
                });
                taken[*d] = true;
            }
            _ => plan.ambiguous.push((w, found)),
        }
    }
    for &d in inflows.iter().filter(|&&d| !taken[d]) {
        let found = candidates(d, &outflows);
        if found.is_empty() {
            plan.orphans.push(d);
        } else {
            plan.ambiguous.push((d, found));
        }
    }
    plan
}

/// Days between two dates, either way round.
pub fn calendar_days(a: Date, b: Date) -> u32 {
    (a.to_julian_day() - b.to_julian_day()).unsigned_abs()
}

/// Business days (Monday to Friday) after the earlier date up to the later one, inclusive. Friday → Monday is 1;
/// the same day is 0. Holidays are not known.
pub fn business_days(a: Date, b: Date) -> u32 {
    let (first, last) = if a <= b { (a, b) } else { (b, a) };
    let mut count = 0;
    let mut day = first;
    while day < last {
        day = day.next_day().unwrap_or(last);
        if !matches!(day.weekday(), Weekday::Saturday | Weekday::Sunday) {
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests;
