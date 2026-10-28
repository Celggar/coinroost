//! A rule set as a file: categories, and rules in order, naming categories, payees and accounts. It is how a
//! household's rules are loaded at once (`coinroost-server rules load`) and how they are tried out from the command
//! line (`coinroost categorize`).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{Direction, PayeeAction, Rule};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuleSet {
    #[serde(default)]
    pub categories: Vec<NewCategory>,
    /// In order: the first rule that holds wins.
    pub rules: Vec<NewRule>,
}

/// What a category holds: spending, income, or money moved between the household's own accounts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CategoryKind {
    Expense,
    Income,
    Transfer,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NewCategory {
    pub name: String,
    pub kind: CategoryKind,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NewRule {
    /// Unique in the set; loading the set again updates the rule with this name.
    pub name: String,
    pub when: When,
    pub then: Then,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct When {
    /// Text the description contains, compared in upper case; kept exactly as written.
    pub contains: Option<String>,
    pub direction: Option<Direction>,
    /// The account's namespace.
    pub account: Option<String>,
    /// Magnitude in minor units, both ends included.
    pub amount_min: Option<i64>,
    pub amount_max: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Then {
    pub category: Option<String>,
    pub payee: Option<String>,
    /// The matched text is a payment channel: the payee is the name after it.
    #[serde(default)]
    pub payee_from_text: bool,
}

impl RuleSet {
    pub fn from_json(text: &str) -> Result<RuleSet, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }

    /// The rules in order, by name, after checking the set: names are unique, every category a rule sets is in
    /// the set, and every rule can work.
    pub fn rules(&self) -> Result<Vec<Rule<String, String, String>>, String> {
        let mut categories = HashSet::new();
        for category in &self.categories {
            if category.name.trim().is_empty() || !categories.insert(category.name.as_str()) {
                return Err(format!("category «{}» is empty or repeated", category.name));
            }
        }
        let mut names = HashSet::new();
        self.rules
            .iter()
            .map(|rule| {
                if rule.name.trim().is_empty() || !names.insert(rule.name.as_str()) {
                    return Err(format!("rule «{}» is empty or repeated", rule.name));
                }
                if let Some(category) = &rule.then.category
                    && !categories.contains(category.as_str())
                {
                    return Err(format!(
                        "rule «{}» sets a category not in the set: {category}",
                        rule.name
                    ));
                }
                rule.compile().map_err(|e| format!("rule «{}»: {e}", rule.name))
            })
            .collect()
    }
}

impl NewRule {
    pub fn compile(&self) -> Result<Rule<String, String, String>, String> {
        let payee = match (&self.then.payee, self.then.payee_from_text) {
            (Some(_), true) => {
                return Err("a payee is either named or read from the text, not both".to_owned());
            }
            (Some(payee), false) => PayeeAction::Fixed(payee.clone()),
            (None, true) => PayeeAction::FromText,
            (None, false) => PayeeAction::None,
        };
        Rule::new(
            self.when.contains.clone(),
            self.when.direction,
            self.when.account.clone(),
            self.when.amount_min,
            self.when.amount_max,
            self.then.category.clone(),
            payee,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_names_and_references() {
        let set = |json: &str| RuleSet::from_json(json).and_then(|s| s.rules());
        assert!(
            set(r#"{"categories":[{"name":"Food","kind":"expense"}],
                    "rules":[{"name":"a","when":{"contains":"MARKET"},"then":{"category":"Food"}}]}"#)
            .is_ok()
        );
        for bad in [
            r#"{"rules":[{"name":"a","when":{"contains":"X"},"then":{"category":"Missing"}}]}"#,
            r#"{"categories":[],"rules":[{"name":"a","when":{"contains":"X"},"then":{"payee":"P"}},
                                         {"name":"a","when":{"contains":"Y"},"then":{"payee":"P"}}]}"#,
            r#"{"rules":[{"name":"a","when":{"contains":"X"},"then":{"payee":"P","payee_from_text":true}}]}"#,
            r#"{"rules":[{"name":"a","when":{"contains":""},"then":{"payee":"P"}}]}"#,
            r#"{"rules":[{"name":"a","when":{"contains":"X"},"then":{"payee":"P"},"extra":1}]}"#,
        ] {
            assert!(set(bad).is_err(), "{bad}");
        }
    }
}
