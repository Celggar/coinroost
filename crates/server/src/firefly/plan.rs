//! What each Firefly journal becomes, decided before anything is written: the legs come from its source and
//! destination accounts against the map, never from Firefly's type (a card payment can be a withdrawal).

use std::collections::{BTreeMap, HashMap};

use serde::Deserialize;
use time::Date;

use super::amount::minor_units;
use super::snapshot::{Account, Journal, Snapshot};

/// The person's map: which Coinroost account each Firefly asset or liability account becomes, and the kind of
/// categories whose kind cannot be told from their use.
#[derive(Deserialize)]
pub struct Map {
    pub accounts: BTreeMap<String, Mapped>,
    #[serde(default)]
    pub categories: BTreeMap<String, String>,
}

#[derive(Deserialize)]
pub struct Mapped {
    /// The account's name in Firefly, to catch a map written for another ledger.
    pub name: String,
    /// `checking`, `savings`, `credit_card`, `cash`, `loan` or `investment`.
    pub kind: String,
    /// The namespace its statements are imported with, so their `external_id`s match.
    pub namespace: String,
    pub profile: Option<String>,
}

pub struct Leg {
    pub firefly_account: String,
    pub minor: i64,
    pub external_id: Option<String>,
}

pub struct Planned {
    pub journal_id: String,
    pub date: Date,
    pub description: String,
    pub category: Option<String>,
    pub payee: Option<String>,
    pub legs: Vec<Leg>,
}

/// Counterparts that are a payee: who was paid or who paid.
const PAYEES: [&str; 4] = [
    "Expense account",
    "Revenue account",
    "Beneficiary account",
    "Cash account",
];

/// Every asset and liability account of the snapshot is in the map under its name, and the map names nothing else.
pub fn check_map(snapshot: &Snapshot, map: &Map) -> Result<(), String> {
    let own: Vec<&Account> = snapshot
        .accounts
        .iter()
        .filter(|a| matches!(a.kind.as_str(), "asset" | "liabilities"))
        .collect();
    let missing: Vec<String> = own
        .iter()
        .filter(|a| !map.accounts.contains_key(&a.id))
        .map(|a| format!("{} ({})", a.id, a.name))
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "the map does not name these Firefly accounts: {}",
            missing.join(", ")
        ));
    }
    for (id, mapped) in &map.accounts {
        match own.iter().find(|a| &a.id == id) {
            None => {
                return Err(format!(
                    "the map names account {id}, which is no asset or liability in Firefly"
                ));
            }
            Some(a) if a.name != mapped.name => {
                return Err(format!(
                    "account {id} is {:?} in Firefly, not {:?}",
                    a.name, mapped.name
                ));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// The `external_id`s written in a journal's notes (`ext=…`), in order.
fn noted_ids(notes: &str) -> impl Iterator<Item = &str> {
    notes
        .split("ext=")
        .skip(1)
        .filter_map(|rest| rest.split_whitespace().next())
}

/// The amount of a journal in an account's currency: the amount, or the foreign amount when that is the one in the
/// account's currency.
fn amount_in(journal: &Journal, account: &Account) -> Result<i64, String> {
    let currency = account.currency.as_deref().unwrap_or_default();
    let digits = account.digits.unwrap_or(journal.currency_decimal_places);
    if journal.currency_code == currency {
        return minor_units(&journal.amount, digits).map_err(|e| format!("journal {}: {e}", journal.id));
    }
    match (&journal.foreign_currency_code, &journal.foreign_amount) {
        (Some(code), Some(amount)) if code == currency => {
            minor_units(amount, digits).map_err(|e| format!("journal {}: {e}", journal.id))
        }
        _ => Err(format!(
            "journal {}: no amount in {currency}, the currency of account {}",
            journal.id, account.name
        )),
    }
}

pub(super) fn plan(
    journal: &Journal,
    accounts: &HashMap<&str, &Account>,
    map: &Map,
) -> Result<Planned, String> {
    let own = |id: &str| {
        map.accounts
            .contains_key(id)
            .then(|| accounts.get(id).copied())
            .flatten()
    };
    let (source, destination) = (own(&journal.source_id), own(&journal.destination_id));
    let field_id = journal.external_id.clone().filter(|id| !id.trim().is_empty());
    // A transfer the hub merged keeps the outflow's id in the field and both in the notes: the other is the inflow's.
    // Without the field, which noted id is whose cannot be told, so none is taken.
    let inflow_id = field_id.as_deref().and_then(|outflow| {
        let noted = noted_ids(journal.notes.as_deref()?).find(|id| *id != outflow);
        noted.map(str::to_owned)
    });
    let mut legs = Vec::new();
    if let Some(account) = source {
        let external_id = field_id.clone();
        legs.push(Leg {
            firefly_account: account.id.clone(),
            minor: -amount_in(journal, account)?,
            external_id,
        });
    }
    if let Some(account) = destination {
        let external_id = if source.is_some() { inflow_id } else { field_id };
        legs.push(Leg {
            firefly_account: account.id.clone(),
            minor: amount_in(journal, account)?,
            external_id,
        });
    }
    let payee = match (source, destination) {
        (Some(_), None) => PAYEES
            .contains(&journal.destination_type.as_str())
            .then(|| journal.destination_name.clone()),
        (None, Some(_)) => PAYEES
            .contains(&journal.source_type.as_str())
            .then(|| journal.source_name.clone()),
        (None, None) => {
            return Err(format!(
                "journal {}: neither of its accounts is in the map",
                journal.id
            ));
        }
        (Some(_), Some(_)) => None,
    };
    Ok(Planned {
        journal_id: journal.id.clone(),
        date: journal.day()?,
        description: journal.description.clone(),
        category: journal.category_name.clone().filter(|c| !c.trim().is_empty()),
        payee,
        legs,
    })
}

/// A category's kind: the map's, else from its use (only between own accounts: transfer; only money in: income),
/// else expense.
pub fn category_kind(name: &str, planned: &[Planned], map: &Map) -> String {
    if let Some(kind) = map.categories.get(name) {
        return kind.clone();
    }
    let used: Vec<&Planned> = planned
        .iter()
        .filter(|p| p.category.as_deref() == Some(name))
        .collect();
    if !used.is_empty() && used.iter().all(|p| p.legs.len() == 2) {
        return "transfer".to_owned();
    }
    if !used.is_empty() && used.iter().all(|p| p.legs.iter().all(|l| l.minor > 0)) {
        return "income".to_owned();
    }
    "expense".to_owned()
}
