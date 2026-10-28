//! `coinroost categorize --rules <set.json> [--payee-text <locale.toml>]`: reads transactions as JSON lines on
//! standard input (`{"description": …, "amount_minor": …, "account": …}`, the account being optional) and prints,
//! for each, the category and payee the rule set gives, as JSON lines (`null` when no rule applies).

use std::io::{BufRead, Write};

use coinroost_rules::{Payee, PayeeText, RuleSet, Transaction, sort};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    description: String,
    amount_minor: i64,
    account: Option<String>,
}

pub fn run(args: &[String]) -> Result<(), String> {
    let (mut rules, mut payee_text) = (None, None);
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--rules" => rules = rest.next(),
            "--payee-text" => payee_text = rest.next(),
            _ => return Err(crate::USAGE.to_owned()),
        }
    }
    let rules = rules.ok_or_else(|| crate::USAGE.to_owned())?;
    let set = std::fs::read_to_string(rules).map_err(|e| format!("{rules}: {e}"))?;
    let rules = RuleSet::from_json(&set)
        .and_then(|set| set.rules())
        .map_err(|e| format!("{rules}: {e}"))?;
    let text = match payee_text {
        Some(path) => {
            let toml = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            PayeeText::from_toml(&toml).map_err(|e| format!("{path}: {e}"))?
        }
        None => PayeeText::builtin("es-PA").ok_or("the built-in payee text does not load")?,
    };
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for (number, line) in std::io::stdin().lock().lines().enumerate() {
        let line = line.map_err(|e| e.to_string())?;
        let input: Input = serde_json::from_str(&line).map_err(|e| format!("line {}: {e}", number + 1))?;
        let transaction = Transaction {
            description: &input.description,
            amount_minor: input.amount_minor,
            account: input.account.as_ref(),
        };
        let result = match sort(&rules, &transaction, &text) {
            Some(outcome) => json!({
                "rule": outcome.rule,
                "category": outcome.category,
                "payee": outcome.payee.map(|p| match p {
                    Payee::Fixed(name) => name.clone(),
                    Payee::Named(name) => name,
                }),
            }),
            None => json!(null),
        };
        writeln!(out, "{result}").map_err(|e| e.to_string())?;
    }
    Ok(())
}
