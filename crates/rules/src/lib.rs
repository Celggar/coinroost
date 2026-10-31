//! Rules that sort transactions. Rules are kept in order; the first enabled rule whose conditions all hold sets the
//! transaction's category and payee, and no other rule is looked at. The order is what settles overlaps, so the
//! same rules always give the same answer.
//!
//! Pure: no database and no clock, so the server, the phone and the browser sort the same way. The identifiers of
//! accounts, categories and payees are type parameters: the server uses database ids, a rule set file uses names.

mod payee;
mod set;

pub use payee::PayeeText;
pub use set::{CategoryKind, NewCategory, NewRule, RuleSet, Then, When};

use serde::{Deserialize, Serialize};

/// Which way the money moves. A zero amount counts as coming in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    In,
    Out,
}

impl Direction {
    pub fn of(amount_minor: i64) -> Direction {
        if amount_minor < 0 {
            Direction::Out
        } else {
            Direction::In
        }
    }
}

/// How a rule names the payee.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PayeeAction<P> {
    /// The rule leaves the payee alone.
    None,
    Fixed(P),
    /// The matched text is a payment channel: the payee is the name that follows it in the description, or the
    /// matched text itself when no name can be read.
    FromText,
}

/// What a rule looks at, and what it sets.
#[derive(Clone, Debug)]
pub struct Rule<A, C, P> {
    contains: Option<String>,
    contains_upper: Option<String>,
    direction: Option<Direction>,
    account: Option<A>,
    amount_min: Option<i64>,
    amount_max: Option<i64>,
    category: Option<C>,
    payee: PayeeAction<P>,
}

/// One transaction as rules see it.
#[derive(Clone, Copy, Debug)]
pub struct Transaction<'a, A> {
    pub description: &'a str,
    /// Signed: negative is money leaving the account.
    pub amount_minor: i64,
    pub account: Option<&'a A>,
}

/// The payee a rule set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Payee<'r, P> {
    Fixed(&'r P),
    /// Read from the description; payees are told apart by name.
    Named(String),
}

/// What the first matching rule sets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome<'r, C, P> {
    /// The rule's index in the list.
    pub rule: usize,
    pub category: Option<&'r C>,
    pub payee: Option<Payee<'r, P>>,
}

impl<A: PartialEq, C, P> Rule<A, C, P> {
    /// A rule, or why it cannot be one: it needs at least one condition and one action; the text it looks for
    /// is kept exactly as given (spaces count) and cannot be empty; a payee read from the text needs that text.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        contains: Option<String>,
        direction: Option<Direction>,
        account: Option<A>,
        amount_min: Option<i64>,
        amount_max: Option<i64>,
        category: Option<C>,
        payee: PayeeAction<P>,
    ) -> Result<Rule<A, C, P>, RuleError> {
        if contains.as_deref() == Some("") {
            return Err(RuleError::EmptyText);
        }
        if contains.is_none()
            && direction.is_none()
            && account.is_none()
            && amount_min.is_none()
            && amount_max.is_none()
        {
            return Err(RuleError::NoCondition);
        }
        if amount_min.is_some_and(|a| a < 0) || amount_max.is_some_and(|a| a < 0) {
            return Err(RuleError::NegativeAmount);
        }
        if let (Some(min), Some(max)) = (amount_min, amount_max)
            && min > max
        {
            return Err(RuleError::MinAboveMax);
        }
        if category.is_none() && matches!(payee, PayeeAction::None) {
            return Err(RuleError::NothingToSet);
        }
        if matches!(payee, PayeeAction::FromText) && contains.is_none() {
            return Err(RuleError::PayeeTextMissing);
        }
        Ok(Rule {
            contains_upper: contains.as_ref().map(|c| c.to_uppercase()),
            contains,
            direction,
            account,
            amount_min,
            amount_max,
            category,
            payee,
        })
    }

    /// Whether every condition holds; `description_upper` is the description in upper case.
    fn holds(&self, transaction: &Transaction<'_, A>, description_upper: &str) -> bool {
        let magnitude = transaction.amount_minor.unsigned_abs();
        self.contains_upper
            .as_ref()
            .is_none_or(|text| description_upper.contains(text.as_str()))
            && self
                .direction
                .is_none_or(|d| d == Direction::of(transaction.amount_minor))
            && self
                .account
                .as_ref()
                .is_none_or(|a| transaction.account == Some(a))
            && self.amount_min.is_none_or(|min| magnitude >= min.unsigned_abs())
            && self.amount_max.is_none_or(|max| magnitude <= max.unsigned_abs())
    }
}

/// What the first rule whose conditions hold sets, or `None` when no rule applies.
pub fn sort<'r, A: PartialEq, C, P>(
    rules: &'r [Rule<A, C, P>],
    transaction: &Transaction<'_, A>,
    text: &PayeeText,
) -> Option<Outcome<'r, C, P>> {
    let upper = transaction.description.to_uppercase();
    let (index, rule) = rules
        .iter()
        .enumerate()
        .find(|(_, rule)| rule.holds(transaction, &upper))?;
    let payee = match &rule.payee {
        PayeeAction::None => None,
        PayeeAction::Fixed(payee) => Some(Payee::Fixed(payee)),
        PayeeAction::FromText => rule.contains.as_ref().map(|channel| {
            Payee::Named(
                text.name(transaction.description, channel)
                    .unwrap_or_else(|| channel.clone()),
            )
        }),
    };
    Some(Outcome {
        rule: index,
        category: rule.category.as_ref(),
        payee,
    })
}

/// Why a rule cannot be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleError {
    EmptyText,
    NoCondition,
    NegativeAmount,
    MinAboveMax,
    NothingToSet,
    PayeeTextMissing,
}

impl RuleError {
    /// A stable name for it, for clients that show their own text.
    pub fn code(self) -> &'static str {
        match self {
            RuleError::EmptyText => "rule_empty_text",
            RuleError::NoCondition => "rule_no_condition",
            RuleError::NegativeAmount => "rule_negative_amount",
            RuleError::MinAboveMax => "rule_min_above_max",
            RuleError::NothingToSet => "rule_nothing_to_set",
            RuleError::PayeeTextMissing => "rule_payee_text_missing",
        }
    }
}

impl std::fmt::Display for RuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            RuleError::EmptyText => "the text to look for cannot be empty",
            RuleError::NoCondition => "a rule needs at least one condition",
            RuleError::NegativeAmount => "amounts in a rule are magnitudes, zero or more",
            RuleError::MinAboveMax => "the smallest amount is above the largest",
            RuleError::NothingToSet => "a rule needs a category or a payee to set",
            RuleError::PayeeTextMissing => "a payee read from the text needs the text the rule looks for",
        })
    }
}

impl From<RuleError> for String {
    fn from(error: RuleError) -> String {
        error.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type R = Rule<&'static str, &'static str, &'static str>;

    fn rule(contains: &str, direction: Option<Direction>, category: &'static str) -> R {
        Rule::new(
            Some(contains.to_owned()),
            direction,
            None,
            None,
            None,
            Some(category),
            PayeeAction::None,
        )
        .unwrap()
    }

    fn category(rules: &[R], description: &str, amount_minor: i64) -> Option<&'static str> {
        let text = PayeeText::builtin("es-PA").unwrap();
        let transaction = Transaction {
            description,
            amount_minor,
            account: None,
        };
        sort(rules, &transaction, &text).and_then(|o| o.category.copied())
    }

    #[test]
    fn the_first_rule_that_holds_wins() {
        let rules = [
            rule("fuel station", Some(Direction::Out), "Fuel"),
            rule("station", None, "Travel"),
            rule("salary", Some(Direction::In), "Income"),
        ];
        assert_eq!(category(&rules, "CITY FUEL STATION 12", -4000), Some("Fuel"));
        assert_eq!(category(&rules, "Train station", -900), Some("Travel"));
        // Out of direction: the next rule that holds.
        assert_eq!(category(&rules, "fuel station refund", 4000), Some("Travel"));
        assert_eq!(category(&rules, "SALARY JUNE", 0), Some("Income"));
        assert_eq!(category(&rules, "SALARY REVERSAL", -1), None);
    }

    #[test]
    fn amount_and_account_narrow_a_rule() {
        let text = PayeeText::builtin("es-PA").unwrap();
        let rules: [R; 1] = [Rule::new(
            None,
            None,
            Some("card"),
            Some(1000),
            Some(5000),
            Some("Small"),
            PayeeAction::None,
        )
        .unwrap()];
        let at = |amount_minor, account| {
            let transaction = Transaction {
                description: "anything",
                amount_minor,
                account,
            };
            sort(&rules, &transaction, &text).map(|o| o.rule)
        };
        assert_eq!(at(-1000, Some(&"card")), Some(0));
        assert_eq!(at(5000, Some(&"card")), Some(0));
        assert_eq!(at(-999, Some(&"card")), None);
        assert_eq!(at(-5001, Some(&"card")), None);
        assert_eq!(at(-2000, Some(&"cash")), None);
        assert_eq!(at(-2000, None), None);
    }

    #[test]
    fn refuses_rules_that_cannot_work() {
        let none = || PayeeAction::<&str>::None;
        let bad: [Result<R, RuleError>; 5] = [
            Rule::new(Some(String::new()), None, None, None, None, Some("C"), none()),
            Rule::new(None, None, None, None, None, Some("C"), none()),
            Rule::new(None, None, None, Some(5), Some(1), Some("C"), none()),
            Rule::new(Some("x".to_owned()), None, None, None, None, None, none()),
            Rule::new(
                None,
                Some(Direction::Out),
                None,
                None,
                None,
                None,
                PayeeAction::FromText,
            ),
        ];
        let why = bad.map(Result::err);
        let want = [
            RuleError::EmptyText,
            RuleError::NoCondition,
            RuleError::MinAboveMax,
            RuleError::NothingToSet,
            RuleError::PayeeTextMissing,
        ];
        assert_eq!(why, want.map(Some));
    }
}
