//! `coinroost normalize --profile <id> --namespace <account> <file>`: prints the normalization as JSON.
//! `coinroost profiles`: lists the built-in profiles.

use std::process::ExitCode;

use coinroost_normalizer::{Profile, normalize};
use serde_json::{Value, json};

const USAGE: &str =
    "usage: coinroost normalize --profile <id> --namespace <account> <file>\n       coinroost profiles";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("profiles") => {
            for profile in Profile::builtin() {
                println!("{}\t{}", profile.id, profile.name);
            }
            ExitCode::SUCCESS
        }
        Some("normalize") => match run_normalize(&args[1..]) {
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

fn run_normalize(args: &[String]) -> Result<Value, String> {
    let (mut profile, mut namespace, mut file) = (None, None, None);
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--profile" => profile = rest.next(),
            "--namespace" => namespace = rest.next(),
            _ if file.is_none() => file = Some(arg),
            _ => return Err(USAGE.to_owned()),
        }
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
