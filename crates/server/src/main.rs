//! `coinroost-server serve`: applies the schema and serves the API on `COINROOST_BIND` (default `127.0.0.1:8080`)
//! with the database at `DATABASE_URL`, and the web app built in `COINROOST_WEB_DIR` when it is set.
//! `coinroost-server migrate`: creates the database if it does not exist, applies the schema and exits.
//! `coinroost-server user add <email> [--household <id>]`: adds a user, to a new household unless one is given.
//! The password is read from the terminal (or from the first line of standard input when it is not a terminal),
//! never from the command line, where other users could see it.
//! `coinroost-server openapi`: prints the API contract (`OpenAPI` 3.1, JSON).

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use coinroost_server::{AppState, MIGRATOR, app, auth, openapi};
use sqlx::migrate::MigrateDatabase;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

const USAGE: &str = "usage: coinroost-server serve | migrate | user add <email> [--household <id>] | openapi";

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
            if !dir.join("index.html").is_file() {
                return Err(format!("COINROOST_WEB_DIR: no index.html in {}", dir.display()));
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
    eprintln!("serving on {address}");
    axum::serve(listener, app(AppState { db }, web.as_deref()))
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
