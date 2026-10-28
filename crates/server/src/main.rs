//! `coinroost-server serve`: applies the schema and serves the API on `COINROOST_BIND` (default `127.0.0.1:8080`)
//! with the database at `DATABASE_URL`, the web app built in `COINROOST_WEB_DIR` when it is set, and `/mcp` for MCP
//! clients, which accepts the `Host` names in `COINROOST_MCP_HOSTS` (comma separated; loopback when unset or empty).
//! `coinroost-server migrate`: creates the database if it does not exist, applies the schema and exits.
//! `coinroost-server user add <email> [--household <id>]`: adds a user, to a new household unless one is given.
//! The password is read from the terminal (or from the first line of standard input when it is not a terminal),
//! never from the command line, where other users could see it.
//! `coinroost-server rules load <set.json> --household <id>`: loads a rule set (categories, payees and rules by
//! name) into a household; loading it again changes nothing.
//! `coinroost-server payroll load <employers.json> --household <id>`: loads employers by name; again, nothing changes.
//! `coinroost-server accounts load <accounts.toml> --household <id>`: sets each listed account's profile and the
//! detection that tells its exports apart, by namespace (accounts made by a migration have none); again, nothing changes.
//! `coinroost-server token add <email> --name <label> [--read-only]`: prints a new API token for the user, once.
//! `coinroost-server token revoke <email> --name <label>`: revokes the user's live token of that name.
//! `coinroost-server firefly fetch --out <dir>`: reads a Firefly III ledger with GET requests only into a new
//! folder only you can read; the address is `FIREFLY_URL` and the token is read from the file `FIREFLY_TOKEN_FILE`.
//! `coinroost-server firefly import|check <dir> --household <id> --accounts <map.json>`: moves a fetched ledger
//! into a household, or compares every mapped account's month-end balance with Firefly's.
//! `coinroost-server openapi`: prints the API contract (`OpenAPI` 3.1, JSON).

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use coinroost_server::tokens::{self, Scope};
use coinroost_server::{
    AppState, MIGRATOR, SHELL, account_rules, app, auth, employers, firefly, mcp, openapi, rule_set,
};
use sqlx::migrate::MigrateDatabase;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

const USAGE: &str = "usage: coinroost-server serve | migrate | user add <email> [--household <id>]
       | rules load <set.json> --household <id> | payroll load <employers.json> --household <id>
       | accounts load <accounts.toml> --household <id>
       | token add <email> --name <label> [--read-only] | token revoke <email> --name <label> | openapi
       | firefly fetch --out <dir> | firefly import <dir> --household <id> --accounts <map.json>
       | firefly check <dir> --household <id> --accounts <map.json>";

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let outcome = match args.as_slice() {
        ["openapi"] => print_contract(),
        ["migrate"] => connect().await.map(|_| ()),
        ["serve"] => serve().await,
        ["user", "add", email] => add_user(email, None).await,
        ["user", "add", email, "--household", id] => match Uuid::parse_str(id) {
            Ok(id) => add_user(email, Some(id)).await,
            Err(e) => Err(format!("--household: {e}")),
        },
        ["rules", "load", file, "--household", id] => match Uuid::parse_str(id) {
            Ok(id) => load_rules(file, id).await,
            Err(e) => Err(format!("--household: {e}")),
        },
        ["payroll", "load", file, "--household", id] => match Uuid::parse_str(id) {
            Ok(id) => load_employers(file, id).await,
            Err(e) => Err(format!("--household: {e}")),
        },
        ["accounts", "load", file, "--household", id] => match Uuid::parse_str(id) {
            Ok(id) => load_accounts(file, id).await,
            Err(e) => Err(format!("--household: {e}")),
        },
        ["token", "add", email, "--name", name] => add_token(email, name, Scope::Write).await,
        ["token", "add", email, "--name", name, "--read-only"] => add_token(email, name, Scope::Read).await,
        ["token", "revoke", email, "--name", name] => revoke_token(email, name).await,
        ["firefly", "fetch", "--out", dir] => firefly_fetch(dir).await,
        [
            "firefly",
            step @ ("import" | "check"),
            dir,
            "--household",
            id,
            "--accounts",
            map,
        ] => match Uuid::parse_str(id) {
            Ok(id) => firefly_offline(step, dir, id, map).await,
            Err(e) => Err(format!("--household: {e}")),
        },
        _ => Err(USAGE.to_owned()),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

fn print_contract() -> Result<(), String> {
    let json = openapi().to_pretty_json().map_err(|e| e.to_string())?;
    println!("{json}");
    Ok(())
}

/// A pool on `DATABASE_URL`, with the database created if missing and the schema applied.
async fn connect() -> Result<sqlx::PgPool, String> {
    let url = std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is not set".to_owned())?;
    let exists = sqlx::Postgres::database_exists(&url)
        .await
        .map_err(|e| format!("database: {e}"))?;
    if !exists {
        sqlx::Postgres::create_database(&url)
            .await
            .map_err(|e| format!("creating the database: {e}"))?;
        eprintln!("created the database");
    }
    let db = PgPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await
        .map_err(|e| format!("database: {e}"))?;
    MIGRATOR.run(&db).await.map_err(|e| format!("schema: {e}"))?;
    Ok(db)
}

async fn serve() -> Result<(), String> {
    let web = match std::env::var_os("COINROOST_WEB_DIR") {
        Some(dir) => {
            let dir = PathBuf::from(dir);
            if !dir.join(SHELL).is_file() {
                return Err(format!("COINROOST_WEB_DIR: no {SHELL} in {}", dir.display()));
            }
            Some(dir)
        }
        None => None,
    };
    let db = connect().await?;
    let bind = std::env::var("COINROOST_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_owned());
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .map_err(|e| format!("{bind}: {e}"))?;
    let address = listener.local_addr().map_err(|e| e.to_string())?;
    let hosts: Vec<String> = std::env::var("COINROOST_MCP_HOSTS")
        .unwrap_or_default()
        .split(',')
        .map(|h| h.trim().to_owned())
        .filter(|h| !h.is_empty())
        .collect();
    eprintln!("serving on {address}");
    let state = AppState { db };
    let routes = mcp::router(state.clone(), &hosts).merge(app(state, web.as_deref()));
    axum::serve(listener, routes)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|e| e.to_string())
}

async fn add_user(email: &str, household: Option<Uuid>) -> Result<(), String> {
    let password = read_password()?;
    let db = connect().await?;
    let enrolled = auth::enroll(&db, email, &password, household)
        .await
        .map_err(|e| e.to_string())?;
    println!("household {}", enrolled.household_id);
    println!("user {}", enrolled.user_id);
    println!("Enroll this in an authenticator app; its 6-digit codes are the second step of every login:");
    println!("{}", enrolled.uri);
    println!("secret {}", enrolled.secret_base32);
    Ok(())
}

async fn load_rules(file: &str, household: Uuid) -> Result<(), String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let set = coinroost_rules::RuleSet::from_json(&text).map_err(|e| format!("{file}: {e}"))?;
    let db = connect().await?;
    let report = rule_set::load(&db, household, &set).await?;
    println!(
        "categories added {}, changed {}; payees added {}; rules added {}, changed {}",
        report.categories_added,
        report.categories_changed,
        report.payees_added,
        report.rules_added,
        report.rules_changed
    );
    Ok(())
}

async fn load_employers(file: &str, household: Uuid) -> Result<(), String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let employers = employers::EmployersFile::from_json(&text).map_err(|e| format!("{file}: {e}"))?;
    let db = connect().await?;
    let report = employers::load(&db, household, &employers).await?;
    println!("employers added {}, changed {}", report.added, report.changed);
    Ok(())
}

async fn load_accounts(file: &str, household: Uuid) -> Result<(), String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let rules =
        coinroost_normalizer::detect::AccountRules::from_toml(&text).map_err(|e| format!("{file}: {e}"))?;
    let db = connect().await?;
    let report = account_rules::load(&db, household, &rules).await?;
    println!(
        "accounts changed {}, unchanged {}",
        report.changed, report.unchanged
    );
    Ok(())
}

fn read_password() -> Result<String, String> {
    if std::io::stdin().is_terminal() {
        let first =
            rpassword::prompt_password("Password (12 characters or more): ").map_err(|e| e.to_string())?;
        let again = rpassword::prompt_password("The same password again: ").map_err(|e| e.to_string())?;
        if first != again {
            return Err("the two passwords differ".to_owned());
        }
        return Ok(first);
    }
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).map_err(|e| e.to_string())?;
    Ok(line.trim_end_matches(['\r', '\n']).to_owned())
}

async fn add_token(email: &str, name: &str, scope: Scope) -> Result<(), String> {
    let db = connect().await?;
    let token = tokens::add(&db, email, name, scope).await?;
    eprintln!("a {scope:?} token for {email}, named {name:?}; it is shown only now:");
    println!("token {token}");
    Ok(())
}

async fn revoke_token(email: &str, name: &str) -> Result<(), String> {
    let db = connect().await?;
    if tokens::revoke(&db, email, name).await? {
        eprintln!("revoked {email}'s token {name:?}");
        Ok(())
    } else {
        Err(format!("{email} has no live token named {name:?}"))
    }
}

async fn firefly_fetch(dir: &str) -> Result<(), String> {
    let url =
        std::env::var("FIREFLY_URL").map_err(|_| "set FIREFLY_URL, such as http://firefly.lan".to_owned())?;
    let token_file = std::env::var("FIREFLY_TOKEN_FILE")
        .map_err(|_| "set FIREFLY_TOKEN_FILE to a file holding the token (chmod 600)".to_owned())?;
    let token = firefly::fetch::token(std::path::Path::new(&token_file))?;
    let summary = firefly::fetch::fetch(&url, &token, std::path::Path::new(dir)).await?;
    eprintln!("{summary}");
    Ok(())
}

async fn firefly_offline(step: &str, dir: &str, household: Uuid, map: &str) -> Result<(), String> {
    let snapshot = firefly::snapshot::Snapshot::load(std::path::Path::new(dir))?;
    let text = std::fs::read_to_string(map).map_err(|e| format!("{map}: {e}"))?;
    let map: firefly::plan::Map = serde_json::from_str(&text).map_err(|e| format!("{map}: {e}"))?;
    let db = connect().await?;
    if step == "import" {
        println!(
            "{}",
            firefly::import::import(&db, household, &snapshot, &map).await?
        );
        return Ok(());
    }
    let (report, differ) = firefly::check::check(&db, household, &snapshot, &map).await?;
    println!("{report}");
    if differ == 0 {
        Ok(())
    } else {
        Err(format!("{differ} month-end balances differ from Firefly's"))
    }
}
