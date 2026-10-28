//! `firefly fetch`: a snapshot of a Firefly ledger, read with GET requests only. The folder holds the owner's real
//! financial data (and webhook secrets), so it is created readable by the owner alone.

use std::collections::BTreeSet;
use std::fs::{DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::Path;

use hyper::StatusCode;
use hyper::body::Bytes;
use serde_json::{Value, json};
use time::macros::format_description;
use time::{Date, Duration, OffsetDateTime};

use super::KINDS;
use super::http::Firefly;

/// Past the highest group id a list returned, groups are probed one by one until this many in a row are missing:
/// the newest group may be one no list returns.
const MISSING_IN_A_ROW: u64 = 50;
const PAGE_SIZE: u32 = 100;

/// The token, from a file only its owner can read.
pub fn token(path: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.mode() & 0o077 != 0 {
        return Err(format!(
            "{} must be readable by you alone (chmod 600)",
            path.display()
        ));
    }
    let token = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if token.trim().is_empty() {
        return Err(format!("{} is empty", path.display()));
    }
    Ok(token)
}

fn private_dir(path: &Path) -> Result<(), String> {
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn save(path: &Path, body: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        private_dir(parent)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    file.write_all(body)
        .map_err(|e| format!("{}: {e}", path.display()))
}

async fn ok_json(firefly: &Firefly, path: &str) -> Result<(Bytes, Value), String> {
    let (status, body) = firefly.get(path).await?;
    match status {
        StatusCode::OK => {}
        StatusCode::UNAUTHORIZED => return Err("Firefly refused the token".to_owned()),
        other => return Err(format!("{path}: Firefly answered {other}")),
    }
    let value = serde_json::from_slice(&body).map_err(|e| format!("{path}: {e}"))?;
    Ok((body, value))
}

/// Every page of every kind, as Firefly sent them; the number of items Firefly announced for each.
async fn pages(firefly: &Firefly, out: &Path) -> Result<(Value, Vec<Value>), String> {
    let mut counts = serde_json::Map::new();
    let mut groups = Vec::new();
    for kind in &KINDS {
        let mut page = 1;
        loop {
            let query = [kind.query, &format!("limit={PAGE_SIZE}&page={page}")].join("&");
            let path = format!("{}?{}", kind.path, query.trim_start_matches('&'));
            let (body, value) = ok_json(firefly, &path).await?;
            save(
                &out.join("pages").join(kind.name).join(format!("{page}.json")),
                &body,
            )?;
            let pagination = &value["meta"]["pagination"];
            counts.insert(kind.name.to_owned(), pagination["total"].clone());
            if kind.name == "transactions" {
                groups.extend(value["data"].as_array().cloned().unwrap_or_default());
            }
            if pagination["total_pages"].as_u64().unwrap_or(1) <= page {
                break;
            }
            page += 1;
        }
    }
    Ok((Value::Object(counts), groups))
}

/// Groups no list returns (a liability's opening credit is in none of Firefly's type filters), fetched one by one
/// through the ids the lists skipped. Returns how many were found and how many are deleted.
async fn probe(
    firefly: &Firefly,
    out: &Path,
    listed: &BTreeSet<u64>,
    found: &mut Vec<Value>,
) -> Result<(u64, u64), String> {
    let (mut kept, mut deleted, mut missing_in_a_row) = (0, 0, 0);
    let highest = listed.last().copied().unwrap_or(0);
    let mut id = 0;
    while id < highest || missing_in_a_row < MISSING_IN_A_ROW {
        id += 1;
        if listed.contains(&id) {
            continue;
        }
        let path = format!("/api/v1/transactions/{id}");
        let (status, body) = firefly.get(&path).await?;
        match status {
            StatusCode::OK => {
                let value: Value = serde_json::from_slice(&body).map_err(|e| format!("{path}: {e}"))?;
                save(&out.join("groups").join(format!("{id}.json")), &body)?;
                found.push(value["data"].clone());
                kept += 1;
                missing_in_a_row = 0;
            }
            StatusCode::NOT_FOUND => {
                deleted += u64::from(id <= highest);
                missing_in_a_row += u64::from(id > highest);
            }
            other => return Err(format!("{path}: Firefly answered {other}")),
        }
    }
    Ok((kept, deleted))
}

/// The last day of every month from the first journal's month to the last one's.
fn month_ends(groups: &[Value]) -> Vec<Date> {
    let days: BTreeSet<Date> = groups
        .iter()
        .flat_map(|g| {
            g["attributes"]["transactions"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|s| {
            let date = s["date"].as_str()?.get(..10)?.to_owned();
            Date::parse(&date, format_description!("[year]-[month]-[day]")).ok()
        })
        .collect();
    let (Some(first), Some(last)) = (days.first(), days.last()) else {
        return Vec::new();
    };
    let mut ends = Vec::new();
    let mut month = first.replace_day(1).unwrap_or(*first);
    while month <= *last {
        let next = month.replace_day(28).unwrap_or(month) + Duration::days(4);
        let next = next.replace_day(1).unwrap_or(next);
        ends.push(next - Duration::days(1));
        month = next;
    }
    ends
}

/// Each own account (asset or liability) as Firefly shows it at the end of every month.
async fn balances(firefly: &Firefly, out: &Path, ends: &[Date]) -> Result<u64, String> {
    let mut saved = 0;
    for page in std::fs::read_dir(out.join("pages").join("accounts"))
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let accounts: Value = serde_json::from_slice(&std::fs::read(page.path()).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        for account in accounts["data"].as_array().cloned().unwrap_or_default() {
            let kind = account["attributes"]["type"].as_str().unwrap_or_default();
            let Some(id) = account["id"]
                .as_str()
                .filter(|_| matches!(kind, "asset" | "liabilities"))
            else {
                continue;
            };
            for end in ends {
                let (body, _) = ok_json(firefly, &format!("/api/v1/accounts/{id}?date={end}")).await?;
                save(&out.join("balances").join(id).join(format!("{end}.json")), &body)?;
                saved += 1;
            }
        }
    }
    Ok(saved)
}

/// Fetches the whole ledger into `out`, which must not exist yet; returns a summary.
pub async fn fetch(url: &str, token: &str, out: &Path) -> Result<String, String> {
    if out.exists() {
        return Err(format!(
            "{} already exists: fetch into a new folder",
            out.display()
        ));
    }
    let firefly = Firefly::new(url, token)?;
    private_dir(out)?;
    let (about, about_value) = ok_json(&firefly, "/api/v1/about").await?;
    save(&out.join("about.json"), &about)?;
    let (counts, mut groups) = pages(&firefly, out).await?;
    let listed: BTreeSet<u64> = groups
        .iter()
        .filter_map(|g| g["id"].as_str()?.parse().ok())
        .collect();
    let (outside, deleted) = probe(&firefly, out, &listed, &mut groups).await?;
    let ends = month_ends(&groups);
    let saved = balances(&firefly, out, &ends).await?;
    let manifest = json!({
        "firefly_version": about_value["data"]["version"],
        "api_version": about_value["data"]["api_version"],
        "fetched_from": firefly.base(),
        "fetched_at": OffsetDateTime::now_utc().to_string(),
        "announced": counts,
        "groups_outside_lists": outside,
        "groups_deleted": deleted,
        "month_end_balances": saved,
    });
    let text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    save(&out.join("manifest.json"), text.as_bytes())?;
    Ok(format!(
        "Firefly {}: {} transaction groups ({outside} outside the lists, {deleted} deleted ids), {saved} month-end \
         balances, written to {}",
        about_value["data"]["version"].as_str().unwrap_or("?"),
        groups.len(),
        out.display()
    ))
}
