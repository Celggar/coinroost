//! `coinroost payroll --employer <employer.json> --as-of <date> [--data-until <date>] [--words <locale.toml>]`:
//! reads deposits as JSON lines on standard input (`{"on": "YYYY-MM-DD", "amount_minor", "description", "account"}`)
//! and prints what the employer owes as JSON. The employer file is `{"name", "match", "account",
//! "min_amount_minor", "amount_minor", "start": "YYYY-MM"}`.

use coinroost_payroll::{Deposit, Employer, Fortnight, Labels, NoDeposits, Status, report};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::pair::{date, lines};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmployerFile {
    name: String,
    #[serde(rename = "match")]
    match_text: String,
    account: String,
    min_amount_minor: i64,
    amount_minor: i64,
    start: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    on: String,
    amount_minor: i64,
    description: String,
    account: String,
}

fn start(text: &str) -> Option<Fortnight> {
    let first = date(&format!("{text}-01"))?;
    Some(Fortnight::containing(first))
}

fn name(f: Fortnight) -> String {
    format!("{}-{:02}-{}", f.year, f.month, f.half)
}

fn status(s: Status) -> &'static str {
    match s {
        Status::Owed => "owed",
        Status::NoData => "no_data",
        Status::Late => "late",
        Status::OnTime => "on_time",
        Status::Ahead => "ahead",
    }
}

fn employer(path: &str) -> Result<Employer<String>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let file: EmployerFile = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    Ok(Employer {
        name: file.name,
        match_text: file.match_text,
        account: file.account,
        min_amount_minor: file.min_amount_minor,
        amount_minor: file.amount_minor,
        start: start(&file.start).ok_or_else(|| format!("{path}: start is YYYY-MM"))?,
    })
}

fn labels(words: Option<&String>) -> Result<Labels, String> {
    match words {
        Some(path) => {
            let toml = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            Labels::from_toml(&toml).map_err(|e| format!("{path}: {e}"))
        }
        None => Labels::builtin("es-PA").ok_or_else(|| "the built-in payroll words do not load".to_owned()),
    }
}

fn deposits(inputs: Vec<Input>) -> Result<Vec<Deposit<String>>, String> {
    inputs
        .into_iter()
        .map(|d| {
            Ok(Deposit {
                on: date(&d.on).ok_or_else(|| format!("deposit {}: the date is not YYYY-MM-DD", d.on))?,
                amount_minor: d.amount_minor,
                description: d.description,
                account: d.account,
            })
        })
        .collect()
}

fn output(result: Result<coinroost_payroll::Report, NoDeposits>) -> Value {
    match result {
        Err(NoDeposits) => json!({ "error": "no deposit counted" }),
        Ok(r) => json!({
            "owed": r.owed.iter().copied().map(name).collect::<Vec<_>>(),
            "ahead": r.ahead.iter().copied().map(name).collect::<Vec<_>>(),
            "unknown": r.unknown.iter().copied().map(name).collect::<Vec<_>>(),
            "paid": r.paid,
            "next_due": name(r.next_due),
            "data_until": r.data_until.to_string(),
            "history": r.history.iter().map(|s| json!({
                "fortnight": name(s.fortnight),
                "deposit": s.deposit,
                "status": status(s.status),
                "days": s.days,
                "label": s.label.map(name),
            })).collect::<Vec<_>>(),
        }),
    }
}

pub fn run(args: &[String]) -> Result<(), String> {
    let (mut employer_path, mut as_of, mut data_until, mut words) = (None, None, None, None);
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--employer" => employer_path = rest.next(),
            "--as-of" => as_of = rest.next(),
            "--data-until" => data_until = rest.next(),
            "--words" => words = rest.next(),
            _ => return Err(crate::USAGE.to_owned()),
        }
    }
    let (Some(path), Some(as_of)) = (employer_path, as_of) else {
        return Err(crate::USAGE.to_owned());
    };
    let employer = employer(path)?;
    let as_of = date(as_of).ok_or("--as-of is YYYY-MM-DD")?;
    let data_until = match data_until {
        Some(d) => Some(date(d).ok_or("--data-until is YYYY-MM-DD")?),
        None => None,
    };
    let labels = labels(words)?;
    let deposits = deposits(lines(std::io::stdin().lock(), "standard input")?)?;
    println!(
        "{}",
        output(report(&employer, &deposits, as_of, data_until, &labels))
    );
    Ok(())
}
