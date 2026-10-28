//! A test Firefly III: serves `spec/fixtures/firefly` the way Firefly's API v1 does (JSON:API items,
//! `meta.pagination`, `links.next`, 4 items a page whatever `limit` asks), and records every request it gets.

use std::collections::HashMap;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use serde_json::{Value, json};

pub const TOKEN: &str = "synthetic-token";
const PER_PAGE: usize = 4;
const BASE: &str = "http://firefly.example/api/v1";

pub fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/fixtures/firefly")
}

fn load(name: &str) -> Value {
    let text = std::fs::read_to_string(fixture().join(name)).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// A new empty folder for one test.
pub fn scratch() -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A token file readable by its owner alone (`mode` lets a test make it readable by others).
pub fn token_file(dir: &Path, token: &str, mode: u32) -> PathBuf {
    let path = dir.join("token");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&path)
        .unwrap();
    std::io::Write::write_all(&mut file, token.as_bytes()).unwrap();
    path
}

type Seen = Arc<Mutex<Vec<(Method, String)>>>;

pub struct TestFirefly {
    pub url: String,
    seen: Seen,
}

impl TestFirefly {
    pub async fn start() -> TestFirefly {
        let seen: Seen = Arc::default();
        let app = Router::new().fallback(any(answer)).with_state(seen.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        TestFirefly { url, seen }
    }

    /// Every request so far: its method and its path with the query.
    pub fn requests(&self) -> Vec<(Method, String)> {
        self.seen.lock().unwrap().clone()
    }
}

/// One page of `items` with Firefly's envelope.
fn page(items: &[Value], query: &HashMap<String, String>, path: &str) -> Value {
    let page: usize = query.get("page").and_then(|p| p.parse().ok()).unwrap_or(1).max(1);
    let total_pages = items.len().div_ceil(PER_PAGE).max(1);
    let data: Vec<Value> = items
        .iter()
        .skip((page - 1) * PER_PAGE)
        .take(PER_PAGE)
        .cloned()
        .collect();
    let mut links = json!({ "self": format!("{BASE}{path}?page={page}"), "first": format!("{BASE}{path}?page=1"),
                            "last": format!("{BASE}{path}?page={total_pages}") });
    if page < total_pages {
        links["next"] = json!(format!("{BASE}{path}?page={}", page + 1));
    }
    json!({ "data": data, "links": links, "meta": { "pagination": { "total": items.len(), "count": data.len(),
            "per_page": PER_PAGE, "current_page": page, "total_pages": total_pages } } })
}

fn group_type(group: &Value) -> &str {
    group["attributes"]["transactions"][0]["type"]
        .as_str()
        .unwrap_or_default()
}

/// Firefly's type filters: `all` leaves out liability credits, and no type means withdrawals, deposits and
/// transfers only (`TransactionFilter.php`, 6.6.3).
fn transactions(query: &HashMap<String, String>) -> Vec<Value> {
    let wanted: &[&str] = match query.get("type").map(String::as_str) {
        Some("all") => &[
            "withdrawal",
            "deposit",
            "transfer",
            "opening balance",
            "reconciliation",
        ],
        _ => &["withdrawal", "deposit", "transfer"],
    };
    let groups = load("groups.json");
    groups
        .as_array()
        .unwrap()
        .iter()
        .filter(|g| wanted.contains(&group_type(g)))
        .cloned()
        .collect()
}

fn accounts(query: &HashMap<String, String>) -> Vec<Value> {
    let all = load("accounts.json");
    let normal = ["asset", "liabilities", "expense", "revenue"];
    let keep = |a: &&Value| {
        query.get("type").is_some_and(|t| t == "all")
            || normal.contains(&a["attributes"]["type"].as_str().unwrap())
    };
    all.as_array().unwrap().iter().filter(keep).cloned().collect()
}

/// An account as Firefly shows it at the end of `date`, from the hand-worked balances; a date nobody worked out
/// is an error, never computed.
fn balance(id: &str, query: &HashMap<String, String>) -> Response {
    let Some(date) = query.get("date") else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let balances = load("balances.json");
    let Some(amount) = balances[id][date].as_str() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("no balance worked out for {id} at {date}"),
        )
            .into_response();
    };
    let all = load("accounts.json");
    let mut account = all
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == id)
        .unwrap()
        .clone();
    account["attributes"]["current_balance"] = json!(amount);
    account["attributes"]["current_balance_date"] = json!(format!("{date}T23:59:59-05:00"));
    axum::Json(json!({ "data": account })).into_response()
}

fn route(path: &str, query: &HashMap<String, String>) -> Response {
    let rest = path.trim_start_matches("/api/v1/");
    let found = match rest.split('/').collect::<Vec<_>>().as_slice() {
        ["about"] => load("about.json"),
        ["accounts"] => page(&accounts(query), query, path),
        ["accounts", id] => return balance(id, query),
        ["transactions"] => page(&transactions(query), query, path),
        ["transactions", id] => {
            let groups = load("groups.json");
            match groups.as_array().unwrap().iter().find(|g| g["id"] == *id) {
                Some(group) => json!({ "data": group }),
                None => {
                    return (
                        StatusCode::NOT_FOUND,
                        axum::Json(json!({ "message": "Resource not found" })),
                    )
                        .into_response();
                }
            }
        }
        [kind] if fixture().join(format!("{kind}.json")).exists() => {
            page(load(&format!("{kind}.json")).as_array().unwrap(), query, path)
        }
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    axum::Json(found).into_response()
}

async fn answer(State(seen): State<Seen>, request: Request) -> Response {
    let target = request
        .uri()
        .path_and_query()
        .map_or_else(String::new, ToString::to_string);
    seen.lock().unwrap().push((request.method().clone(), target));
    if request.method() != Method::GET {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let headers = request.headers();
    if headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) != Some(&format!("Bearer {TOKEN}")) {
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(json!({ "message": "Unauthenticated." })),
        )
            .into_response();
    }
    let accepts_json = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|a| a.contains("json"));
    if !accepts_json {
        return (StatusCode::FOUND, [(header::LOCATION, "/login")]).into_response();
    }
    let query: HashMap<String, String> =
        serde_urlencoded::from_str(request.uri().query().unwrap_or("")).unwrap();
    route(request.uri().path(), &query)
}
