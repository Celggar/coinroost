//! `coinroost normalize --profile <id> --namespace <account> <file>`: prints the normalization as JSON.
//! `coinroost detect --accounts <rules.toml> <file>`: prints which account a file belongs to, as JSON.
//! `coinroost tables <file.pdf>`: prints the ruled tables of a PDF, page by page, as JSON.
//! `coinroost profiles`: lists the built-in profiles.
//! `coinroost categorize --rules <set.json> [--payee-text <locale.toml>]`: sorts transactions read as JSON lines.
//! `coinroost pair [--evidence <transfers.jsonl>]`: pairs the two legs of transfers read as JSON lines.
//! `coinroost payroll --employer <file> --as-of <date> [--data-until <date>]`: what an employer owes.

mod categorize;
mod pair;
mod payroll;

use std::process::ExitCode;

use coinroost_normalizer::detect::{AccountRules, Detection, detect};
use coinroost_normalizer::{Profile, normalize};
use serde_json::{Value, json};

pub(crate) const USAGE: &str = "usage: coinroost normalize --profile <id> --namespace <account> <file>
       coinroost detect --accounts <rules.toml> <file>
       coinroost tables <file.pdf>
       coinroost profiles
       coinroost categorize --rules <set.json> [--payee-text <locale.toml>] < transactions.jsonl
       coinroost pair [--evidence <transfers.jsonl>] < legs.jsonl
       coinroost payroll --employer <employer.json> --as-of <date> [--data-until <date>] [--words <locale.toml>] < deposits.jsonl";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("profiles") => {
            for profile in Profile::builtin() {
                println!("{}\t{}", profile.id, profile.name);
            }
            ExitCode::SUCCESS
        }
        Some(command @ ("categorize" | "pair" | "payroll")) => match match command {
            "pair" => pair::run(&args[1..]),
            "payroll" => payroll::run(&args[1..]),
            _ => categorize::run(&args[1..]),
        } {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("{message}");
                ExitCode::from(2)
            }
        },
        Some(command @ ("normalize" | "detect" | "tables")) => match run(command, &args[1..]) {
            Ok(output) => {
                println!("{output:#}");
                ExitCode::SUCCESS
            }
            Err(message) => {
                eprintln!("{message}");
                ExitCode::from(2)
            }
        },
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run(command: &str, args: &[String]) -> Result<Value, String> {
    let (mut profile, mut namespace, mut accounts, mut file) = (None, None, None, None);
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--profile" => profile = rest.next(),
            "--namespace" => namespace = rest.next(),
            "--accounts" => accounts = rest.next(),
            _ if file.is_none() => file = Some(arg),
            _ => return Err(USAGE.to_owned()),
        }
    }
    if command == "tables" {
        let (None, None, None, Some(file)) = (profile, namespace, accounts, file) else {
            return Err(USAGE.to_owned());
        };
        let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
        return Ok(match coinroost_normalizer::pdf::read_tables(&bytes) {
            Ok(tables) => json!(tables),
            Err(error) => json!({ "fatal": error }),
        });
    }
    if command == "detect" {
        let (Some(accounts), Some(file)) = (accounts, file) else {
            return Err(USAGE.to_owned());
        };
        return run_detect(accounts, file);
    }
    let (Some(profile), Some(namespace), Some(file)) = (profile, namespace, file) else {
        return Err(USAGE.to_owned());
    };
    let profile = Profile::find(profile).ok_or_else(|| format!("unknown profile: {profile}"))?;
    let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
    Ok(match normalize(&bytes, &profile, namespace) {
        Ok(result) => result.to_json(),
        Err(error) => json!({ "fatal": error.to_string() }),
    })
}

fn run_detect(accounts: &str, file: &str) -> Result<Value, String> {
    let rules = std::fs::read_to_string(accounts).map_err(|e| format!("{accounts}: {e}"))?;
    let rules = AccountRules::from_toml(&rules).map_err(|e| format!("{accounts}: {e}"))?;
    let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
    let name = std::path::Path::new(file)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    Ok(match detect(&bytes, name, &Profile::builtin(), &rules.account) {
        Detection::Found(namespace) => json!({ "result": "found", "namespace": namespace }),
        Detection::Ambiguous(candidates) => json!({ "result": "ambiguous", "candidates": candidates }),
        Detection::NotFound => json!({ "result": "not_found" }),
    })
}
