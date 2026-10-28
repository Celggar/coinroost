//! `coinroost pair [--evidence <transfers.jsonl>]`: reads single-leg transactions between own accounts as JSON lines
//! on standard input (`{"id", "date": "YYYY-MM-DD", "amount_minor", "currency", "account", "text"}`, the currency
//! optional and USD by default) and prints the pairing plan as JSON: the pairs with the pass that found them, the
//! legs with no counterpart, and the ambiguous ones with their candidates. The evidence file holds transfers already
//! made, one `{"text", "account"}` per line: the outflow's text and the account the money went to.

use std::io::BufRead;

use coinroost_transfers::{Leg, plan};
use serde::Deserialize;
use serde_json::json;
use time::{Date, Month};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    id: String,
    date: String,
    amount_minor: i64,
    currency: Option<String>,
    account: String,
    text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    text: String,
    account: String,
}

fn date(text: &str) -> Option<Date> {
    let mut parts = text.splitn(3, '-').map(str::parse::<u16>);
    let (Some(Ok(year)), Some(Ok(month)), Some(Ok(day))) = (parts.next(), parts.next(), parts.next()) else {
        return None;
    };
    let month = Month::try_from(u8::try_from(month).ok()?).ok()?;
    Date::from_calendar_date(i32::from(year), month, u8::try_from(day).ok()?).ok()
}

fn lines<T: for<'de> Deserialize<'de>>(reader: impl BufRead, source: &str) -> Result<Vec<T>, String> {
    reader
        .lines()
        .enumerate()
        .map(|(number, line)| {
            let line = line.map_err(|e| format!("{source}: {e}"))?;
            serde_json::from_str(&line).map_err(|e| format!("{source} line {}: {e}", number + 1))
        })
        .collect()
}

pub fn run(args: &[String]) -> Result<(), String> {
    let evidence: Vec<Evidence> = match args {
        [] => Vec::new(),
        [flag, path] if flag == "--evidence" => {
            let file = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
            lines(std::io::BufReader::new(file), path)?
        }
        _ => return Err(crate::USAGE.to_owned()),
    };
    let inputs: Vec<Input> = lines(std::io::stdin().lock(), "standard input")?;
    let legs = inputs
        .into_iter()
        .map(|input| {
            Ok(Leg {
                date: date(&input.date)
                    .ok_or_else(|| format!("leg {}: the date is not YYYY-MM-DD", input.id))?,
                id: input.id,
                amount_minor: input.amount_minor,
                currency: input.currency.unwrap_or_else(|| "USD".to_owned()),
                account: input.account,
                text: input.text,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let known: Vec<(&str, String)> = evidence
        .iter()
        .map(|e| (e.text.as_str(), e.account.clone()))
        .collect();
    let found = plan(&legs, &known);
    let id = |i: usize| legs[i].id.as_str();
    let output = json!({
        "pairs": found.pairs.iter().map(|p| json!({
            "outflow": id(p.outflow), "inflow": id(p.inflow), "pass": p.pass as u8,
        })).collect::<Vec<_>>(),
        "orphans": found.orphans.iter().map(|&i| id(i)).collect::<Vec<_>>(),
        "ambiguous": found.ambiguous.iter().map(|(leg, candidates)| json!({
            "leg": id(*leg), "candidates": candidates.iter().map(|&i| id(i)).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    });
    println!("{output}");
    Ok(())
}
