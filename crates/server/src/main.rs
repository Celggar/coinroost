//! `coinroost-server serve`: applies the schema and serves the API on `COINROOST_BIND` (default `127.0.0.1:8080`)
//! with the database at `DATABASE_URL`.
//! `coinroost-server migrate`: applies the schema and exits.
//! `coinroost-server openapi`: prints the API contract (`OpenAPI` 3.1, JSON).

use std::process::ExitCode;

use coinroost_server::{AppState, MIGRATOR, openapi, router};
use sqlx::postgres::PgPoolOptions;

const USAGE: &str = "usage: coinroost-server serve | migrate | openapi";

#[tokio::main]
async fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_default();
    let outcome = match command.as_str() {
        "openapi" => print_contract(),
        "migrate" => connect().await.map(|_| ()),
        "serve" => serve().await,
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

/// A pool on `DATABASE_URL` with the schema applied.
async fn connect() -> Result<sqlx::PgPool, String> {
    let url = std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is not set".to_owned())?;
    let db = PgPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await
        .map_err(|e| format!("database: {e}"))?;
    MIGRATOR.run(&db).await.map_err(|e| format!("schema: {e}"))?;
    Ok(db)
}

async fn serve() -> Result<(), String> {
    let db = connect().await?;
    let bind = std::env::var("COINROOST_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_owned());
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .map_err(|e| format!("{bind}: {e}"))?;
    eprintln!("serving on {bind}");
    axum::serve(listener, router(AppState { db }))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|e| e.to_string())
}
