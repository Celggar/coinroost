//! A fetched Firefly ledger, read back from its folder:
//!
//! - `manifest.json`, the Firefly version and what was fetched;
//! - `pages/<kind>/<n>.json`, every list page as Firefly sent it;
//! - `groups/<id>.json`, transaction groups no list returns (a liability's opening credit), fetched one by one;
//! - `balances/<account id>/<YYYY-MM-DD>.json`, an account as Firefly showed it at the end of that day.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;
use time::Date;
use time::macros::format_description;

use super::KINDS;

pub struct Account {
    pub id: String,
    pub name: String,
    /// asset, liabilities, expense, revenue, cash, initial-balance, reconciliation…
    pub kind: String,
    pub currency: Option<String>,
    pub digits: Option<u32>,
    pub active: bool,
}

/// One journal (a split) of a transaction group, with the fields a migration reads.
#[derive(Deserialize)]
pub struct Journal {
    #[serde(skip)]
    pub group_id: String,
    #[serde(rename = "transaction_journal_id")]
    pub id: String,
    /// withdrawal, deposit, transfer, opening balance, reconciliation, liability credit.
    #[serde(rename = "type")]
    pub kind: String,
    /// As Firefly wrote it, with the time and zone: `2026-03-31T21:30:00-05:00`.
    pub date: String,
    /// Always positive: the direction is the source and the destination.
    pub amount: String,
    pub currency_code: String,
    pub currency_decimal_places: u32,
    pub foreign_amount: Option<String>,
    pub foreign_currency_code: Option<String>,
    pub foreign_currency_decimal_places: Option<u32>,
    pub description: String,
    pub source_id: String,
    pub source_name: String,
    pub source_type: String,
    pub destination_id: String,
    pub destination_name: String,
    pub destination_type: String,
    pub category_name: Option<String>,
    pub notes: Option<String>,
    pub external_id: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub budget_id: Option<String>,
    pub bill_id: Option<String>,
}

impl Journal {
    /// The day the journal belongs to, as written, in the zone Firefly wrote it in: a purchase at 21:30 on the
    /// last day of the month stays in that month.
    pub fn day(&self) -> Result<Date, String> {
        let written = self.date.get(..10).unwrap_or_default();
        Date::parse(written, format_description!("[year]-[month]-[day]"))
            .map_err(|_| format!("journal {}: no date in {:?}", self.id, self.date))
    }
}

/// Firefly's balance of an account at the end of a day.
pub struct Balance {
    pub account_id: String,
    pub date: Date,
    pub amount: String,
}

pub struct Snapshot {
    pub version: String,
    pub accounts: Vec<Account>,
    pub journals: Vec<Journal>,
    pub categories: Vec<String>,
    /// How many of each entity type Firefly reported, by `Kind::name`.
    pub counts: BTreeMap<String, usize>,
    pub balances: Vec<Balance>,
}

fn read(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Every item of a kind's pages, checked against the total Firefly announced.
fn items(dir: &Path, kind: &str) -> Result<Vec<Value>, String> {
    let mut all = Vec::new();
    let mut total = None;
    for page in 1.. {
        let path = dir.join("pages").join(kind).join(format!("{page}.json"));
        if !path.exists() {
            break;
        }
        let body = read(&path)?;
        total = total.or_else(|| body["meta"]["pagination"]["total"].as_u64());
        all.extend(body["data"].as_array().cloned().unwrap_or_default());
    }
    let announced = total.ok_or_else(|| format!("the snapshot has no pages of {kind}"))?;
    if u64::try_from(all.len()).ok() != Some(announced) {
        return Err(format!(
            "{kind}: Firefly announced {announced}, the snapshot holds {}",
            all.len()
        ));
    }
    Ok(all)
}

fn account(item: &Value) -> Result<Account, String> {
    let a = &item["attributes"];
    let text = |key: &str| a[key].as_str().map(str::to_owned);
    Ok(Account {
        id: item["id"].as_str().ok_or("an account without id")?.to_owned(),
        name: text("name").ok_or("an account without name")?,
        kind: text("type").ok_or("an account without type")?,
        currency: text("currency_code"),
        digits: a["currency_decimal_places"]
            .as_u64()
            .and_then(|d| u32::try_from(d).ok()),
        active: a["active"].as_bool().unwrap_or(true),
    })
}

fn journals(group: &Value) -> Result<Vec<Journal>, String> {
    let group_id = group["id"].as_str().ok_or("a transaction group without id")?;
    let splits = group["attributes"]["transactions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    splits
        .into_iter()
        .map(|split| {
            let mut journal: Journal =
                serde_json::from_value(split).map_err(|e| format!("transaction group {group_id}: {e}"))?;
            group_id.clone_into(&mut journal.group_id);
            Ok(journal)
        })
        .collect()
}

fn balances(dir: &Path) -> Result<Vec<Balance>, String> {
    let mut found = Vec::new();
    let Ok(accounts) = std::fs::read_dir(dir.join("balances")) else {
        return Ok(found);
    };
    for account in accounts.flatten() {
        for day in std::fs::read_dir(account.path())
            .map_err(|e| e.to_string())?
            .flatten()
        {
            let path = day.path();
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            let date = Date::parse(stem, format_description!("[year]-[month]-[day]"))
                .map_err(|_| format!("{}: not a date", path.display()))?;
            let body = read(&path)?;
            let amount = body["data"]["attributes"]["current_balance"].as_str();
            found.push(Balance {
                account_id: account.file_name().to_string_lossy().into_owned(),
                date,
                amount: amount
                    .ok_or_else(|| format!("{}: no current_balance", path.display()))?
                    .to_owned(),
            });
        }
    }
    found.sort_by(|a, b| (&a.account_id, a.date).cmp(&(&b.account_id, b.date)));
    Ok(found)
}

impl Snapshot {
    pub fn load(dir: &Path) -> Result<Snapshot, String> {
        let manifest = read(&dir.join("manifest.json"))?;
        let mut counts = BTreeMap::new();
        let mut lists = BTreeMap::new();
        for kind in &KINDS {
            let found = items(dir, kind.name)?;
            counts.insert(kind.name.to_owned(), found.len());
            lists.insert(kind.name, found);
        }
        let mut groups = lists.remove("transactions").unwrap_or_default();
        if let Ok(extra) = std::fs::read_dir(dir.join("groups")) {
            for file in extra.flatten() {
                groups.push(read(&file.path())?["data"].clone());
            }
        }
        *counts.entry("transactions".to_owned()).or_default() = groups.len();
        let mut journals = Vec::new();
        for group in &groups {
            journals.extend(self::journals(group)?);
        }
        let categories = lists["categories"]
            .iter()
            .filter_map(|c| c["attributes"]["name"].as_str().map(str::to_owned))
            .collect();
        Ok(Snapshot {
            version: manifest["firefly_version"]
                .as_str()
                .unwrap_or("unknown")
                .to_owned(),
            accounts: lists["accounts"].iter().map(account).collect::<Result<_, _>>()?,
            journals,
            categories,
            counts,
            balances: balances(dir)?,
        })
    }
}
