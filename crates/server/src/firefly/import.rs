//! `firefly import`: a snapshot into a household, in one database transaction. It writes like `rules load` and
//! `payroll load`, through the database rather than the API: an offline command run on the server, whose values
//! are checked here before anything is written.
//!
//! Running it again adds nothing: a migrated journal is known by its Firefly id, and a leg whose `external_id` the
//! account already has (a statement imported first) is not added twice. A journal with only some legs present gets
//! the missing ones as a transaction of their own.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::KINDS;
use super::plan::{self, Leg, Map, Mapped, Planned};
use super::snapshot::{Account, Snapshot};

const KINDS_OF_ACCOUNT: [&str; 6] = ["checking", "savings", "credit_card", "cash", "loan", "investment"];

#[derive(Default)]
struct Tally {
    added: usize,
    partly: usize,
    present: usize,
    migrated: usize,
}

/// Coinroost's account for each mapped Firefly account, found by namespace or created.
async fn accounts(
    db: &mut PgConnection,
    household: Uuid,
    snapshot: &Snapshot,
    map: &Map,
) -> Result<(HashMap<String, (Uuid, String)>, usize), String> {
    let mut found = HashMap::new();
    let mut created = 0;
    for (id, mapped) in &map.accounts {
        let firefly = snapshot
            .accounts
            .iter()
            .find(|a| &a.id == id)
            .ok_or("an account left the snapshot")?;
        let currency = firefly
            .currency
            .clone()
            .ok_or_else(|| format!("account {id} has no currency"))?;
        if !KINDS_OF_ACCOUNT.contains(&mapped.kind.as_str()) {
            return Err(format!(
                "account {id}: kind {:?} is not one of {KINDS_OF_ACCOUNT:?}",
                mapped.kind
            ));
        }
        let existing: Option<(Uuid, String)> = sqlx::query_as(
            "select id, currency::text from accounts where household_id = $1 and namespace = $2",
        )
        .bind(household)
        .bind(&mapped.namespace)
        .fetch_optional(&mut *db)
        .await
        .map_err(|e| e.to_string())?;
        let account = match existing {
            Some((uuid, held)) if held == currency => uuid,
            Some((_, held)) => {
                return Err(format!(
                    "namespace {} is a {held} account, Firefly's is {currency}",
                    mapped.namespace
                ));
            }
            None => {
                created += 1;
                create_account(db, household, firefly, mapped, &currency).await?
            }
        };
        found.insert(id.clone(), (account, currency));
    }
    Ok((found, created))
}

async fn create_account(
    db: &mut PgConnection,
    household: Uuid,
    firefly: &Account,
    mapped: &Mapped,
    currency: &str,
) -> Result<Uuid, String> {
    let (uuid,): (Uuid,) = sqlx::query_as(
        "insert into accounts (household_id, name, kind, currency, namespace, profile)
         values ($1, $2, $3, $4, $5, $6) returning id",
    )
    .bind(household)
    .bind(&firefly.name)
    .bind(&mapped.kind)
    .bind(currency)
    .bind(&mapped.namespace)
    .bind(&mapped.profile)
    .fetch_one(&mut *db)
    .await
    .map_err(|e| format!("account {}: {e}", firefly.name))?;
    Ok(uuid)
}

/// Where names live: categories (with a kind) or payees.
#[derive(Clone, Copy)]
enum Table {
    Categories,
    Payees,
}

impl Table {
    fn insert(self) -> &'static str {
        match self {
            Table::Categories => {
                "insert into categories (household_id, name, kind) values ($1, $2, $3) on conflict do nothing"
            }
            Table::Payees => "insert into payees (household_id, name) values ($1, $2) on conflict do nothing",
        }
    }

    fn select(self) -> &'static str {
        match self {
            Table::Categories => "select id from categories where household_id = $1 and name = $2",
            Table::Payees => "select id from payees where household_id = $1 and name = $2",
        }
    }
}

/// The id of each name, created when missing (a category with its kind); how many were created.
async fn named(
    db: &mut PgConnection,
    household: Uuid,
    table: Table,
    names: &BTreeMap<String, Option<String>>,
) -> Result<(HashMap<String, Uuid>, usize), String> {
    let (mut ids, mut created) = (HashMap::new(), 0);
    for (name, kind) in names {
        let mut query = sqlx::query(table.insert()).bind(household).bind(name);
        if let (Table::Categories, Some(kind)) = (table, kind) {
            query = query.bind(kind);
        }
        let done = query
            .execute(&mut *db)
            .await
            .map_err(|e| format!("{name}: {e}"))?;
        created += usize::try_from(done.rows_affected()).unwrap_or(0);
        let (id,): (Uuid,) = sqlx::query_as(table.select())
            .bind(household)
            .bind(name)
            .fetch_one(&mut *db)
            .await
            .map_err(|e| e.to_string())?;
        ids.insert(name.clone(), id);
    }
    Ok((ids, created))
}

struct Ids {
    accounts: HashMap<String, (Uuid, String)>,
    categories: HashMap<String, Uuid>,
    payees: HashMap<String, Uuid>,
}

/// One planned journal: skipped when migrated before or when every leg is already there, otherwise written with
/// the legs that are missing.
async fn write(
    db: &mut PgConnection,
    household: Uuid,
    p: &Planned,
    ids: &Ids,
    tally: &mut Tally,
) -> Result<(), String> {
    let migrated: Option<(Uuid,)> = sqlx::query_as(
        "select transaction_id from transaction_origins where household_id = $1 and source = 'firefly' and source_id = $2",
    )
    .bind(household)
    .bind(&p.journal_id)
    .fetch_optional(&mut *db)
    .await
    .map_err(|e| e.to_string())?;
    if migrated.is_some() {
        tally.migrated += 1;
        return Ok(());
    }
    let missing = missing_legs(db, p, ids).await?;
    match (missing.len(), p.legs.len()) {
        (0, _) => {
            tally.present += 1;
            return Ok(());
        }
        (m, all) if m < all => tally.partly += 1,
        _ => tally.added += 1,
    }
    insert(db, household, p, ids, missing).await
}

/// A planned leg with its Coinroost account and currency.
type Placed<'a> = (&'a Uuid, &'a String, &'a Leg);

/// The legs whose `external_id` their account does not hold yet (a statement may have brought it first).
async fn missing_legs<'a>(
    db: &mut PgConnection,
    p: &'a Planned,
    ids: &'a Ids,
) -> Result<Vec<Placed<'a>>, String> {
    let mut missing = Vec::new();
    for leg in &p.legs {
        let (account, currency) = &ids.accounts[&leg.firefly_account];
        let held: Option<(i32,)> = match &leg.external_id {
            Some(external_id) => {
                sqlx::query_as("select 1 from legs where account_id = $1 and external_id = $2")
                    .bind(account)
                    .bind(external_id)
                    .fetch_optional(&mut *db)
                    .await
                    .map_err(|e| e.to_string())?
            }
            None => None,
        };
        if held.is_none() {
            missing.push((account, currency, leg));
        }
    }
    Ok(missing)
}

async fn insert(
    db: &mut PgConnection,
    household: Uuid,
    p: &Planned,
    ids: &Ids,
    missing: Vec<Placed<'_>>,
) -> Result<(), String> {
    let category = p.category.as_ref().map(|c| ids.categories[c]);
    let payee = p.payee.as_ref().map(|n| ids.payees[n]);
    let (transaction,): (Uuid,) = sqlx::query_as(
        "insert into transactions (household_id, date, description, category_id, category_set_by, payee_id, payee_set_by)
         values ($1, $2, $3, $4, $5, $6, $7) returning id",
    )
    .bind(household)
    .bind(p.date)
    .bind(&p.description)
    .bind(category)
    .bind(category.map(|_| "manual"))
    .bind(payee)
    .bind(payee.map(|_| "manual"))
    .fetch_one(&mut *db)
    .await
    .map_err(|e| format!("journal {}: {e}", p.journal_id))?;
    for (account, currency, leg) in missing {
        sqlx::query(
            "insert into legs (transaction_id, household_id, account_id, amount_minor, currency, posted_on,
                               statement_text, external_id) values ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(transaction)
        .bind(household)
        .bind(account)
        .bind(leg.minor)
        .bind(currency)
        .bind(p.date)
        .bind(&p.description)
        .bind(&leg.external_id)
        .execute(&mut *db)
        .await
        .map_err(|e| format!("journal {}: {e}", p.journal_id))?;
    }
    sqlx::query("insert into transaction_origins (transaction_id, household_id, source, source_id) values ($1, $2, 'firefly', $3)")
        .bind(transaction)
        .bind(household)
        .bind(&p.journal_id)
        .execute(&mut *db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Every journal planned, or every reason one cannot be (the first twenty).
fn planned(snapshot: &Snapshot, map: &Map) -> Result<Vec<Planned>, String> {
    let accounts: HashMap<&str, &Account> = snapshot.accounts.iter().map(|a| (a.id.as_str(), a)).collect();
    let (mut planned, mut errors) = (Vec::new(), Vec::new());
    for journal in &snapshot.journals {
        match plan::plan(journal, &accounts, map) {
            Ok(p) if p.description.chars().count() > 1000 => errors.push(format!(
                "journal {}: description over 1000 characters",
                p.journal_id
            )),
            Ok(p) => planned.push(p),
            Err(e) => errors.push(e),
        }
    }
    if !errors.is_empty() {
        let shown: Vec<String> = errors.iter().take(20).cloned().collect();
        return Err(format!(
            "{} journals cannot be migrated:\n  {}",
            errors.len(),
            shown.join("\n  ")
        ));
    }
    planned.sort_by(|a, b| (a.date, &a.journal_id).cmp(&(b.date, &b.journal_id)));
    Ok(planned)
}

fn report(snapshot: &Snapshot, created: [usize; 3], ids: &Ids, tally: &Tally) -> String {
    let dropped = |test: &dyn Fn(&super::snapshot::Journal) -> bool| {
        snapshot.journals.iter().filter(|j| test(j)).count()
    };
    let mut lines = vec![
        format!("Firefly {} migrated:", snapshot.version),
        format!(
            "  accounts {} ({} created) · categories {} ({} created) · payees {} ({} created)",
            ids.accounts.len(),
            created[0],
            ids.categories.len(),
            created[1],
            ids.payees.len(),
            created[2]
        ),
        format!(
            "  journals {}: {} added, {} partly present (missing legs added), {} already from a statement, {} migrated before",
            snapshot.journals.len(),
            tally.added,
            tally.partly,
            tally.present,
            tally.migrated
        ),
        "  in Firefly (count · where it lives now):".to_owned(),
    ];
    for kind in &KINDS {
        let count = snapshot.counts.get(kind.name).copied().unwrap_or(0);
        lines.push(format!(
            "    {:<18} {count:>6} · {}",
            kind.name,
            kind.home.unwrap_or("GAP: not migrated")
        ));
    }
    lines.push(format!(
        "  on journals, not migrated: tags on {}, budgets on {}, bills on {}, notes on {}",
        dropped(&|j| !j.tags.is_empty()),
        dropped(&|j| j.budget_id.is_some()),
        dropped(&|j| j.bill_id.is_some()),
        dropped(&|j| j.notes.as_deref().is_some_and(|n| !n.trim().is_empty()))
    ));
    lines.join("\n")
}

pub async fn import(db: &PgPool, household: Uuid, snapshot: &Snapshot, map: &Map) -> Result<String, String> {
    plan::check_map(snapshot, map)?;
    let planned = planned(snapshot, map)?;
    let categories: BTreeMap<String, Option<String>> = snapshot
        .categories
        .iter()
        .chain(planned.iter().filter_map(|p| p.category.as_ref()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|name| (name.clone(), Some(plan::category_kind(name, &planned, map))))
        .collect();
    let payees: BTreeMap<String, Option<String>> = planned
        .iter()
        .filter_map(|p| p.payee.clone())
        .map(|n| (n, None))
        .collect();
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    let (accounts, new_accounts) = self::accounts(&mut tx, household, snapshot, map).await?;
    let (categories, new_categories) = named(&mut tx, household, Table::Categories, &categories).await?;
    let (payees, new_payees) = named(&mut tx, household, Table::Payees, &payees).await?;
    let ids = Ids {
        accounts,
        categories,
        payees,
    };
    let mut tally = Tally::default();
    for p in &planned {
        write(&mut tx, household, p, &ids, &mut tally).await?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(report(
        snapshot,
        [new_accounts, new_categories, new_payees],
        &ids,
        &tally,
    ))
}
