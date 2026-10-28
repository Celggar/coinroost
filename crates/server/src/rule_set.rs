//! Loading a rule set file (`coinroost_rules::RuleSet`) into a household: its categories, payees and rules, by
//! name. Loading the same set again changes nothing: a category or rule that already has the name is updated only
//! where it differs, and rules not there yet are added after the household's last rule, in the file's order.
//! Transactions are not touched; applying the rules to them is the preview and apply of the API.

use std::collections::HashMap;

use coinroost_rules::{Direction, NewRule, RuleSet};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

/// What a load changed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct LoadReport {
    pub categories_added: usize,
    pub categories_changed: usize,
    pub payees_added: usize,
    pub rules_added: usize,
    pub rules_changed: usize,
}

/// A rule's conditions and actions as stored.
#[derive(PartialEq, Eq, sqlx::FromRow)]
struct Stored {
    description_contains: Option<String>,
    direction: Option<String>,
    account_id: Option<Uuid>,
    amount_min: Option<i64>,
    amount_max: Option<i64>,
    category_id: Option<Uuid>,
    payee_id: Option<Uuid>,
    payee_from_text: bool,
}

#[derive(sqlx::FromRow)]
struct Existing {
    id: Uuid,
    #[sqlx(flatten)]
    stored: Stored,
}

pub async fn load(db: &PgPool, household_id: Uuid, set: &RuleSet) -> Result<LoadReport, String> {
    set.rules()?;
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    let report = load_in(&mut tx, household_id, set)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(report)
}

/// The household's categories, accounts and payees by name.
struct Maps {
    categories: HashMap<String, Uuid>,
    accounts: HashMap<String, Uuid>,
    payees: HashMap<String, Uuid>,
}

/// The rule as it should be stored, its names resolved.
fn wanted(rule: &NewRule, maps: &Maps) -> Result<Stored, sqlx::Error> {
    let Maps {
        categories,
        accounts,
        payees,
    } = maps;
    let when = &rule.when;
    let account = match &when.account {
        Some(namespace) => Some(*accounts.get(namespace).ok_or_else(|| {
            sqlx::Error::Protocol(format!(
                "rule «{}»: no account with namespace {namespace}",
                rule.name
            ))
        })?),
        None => None,
    };
    Ok(Stored {
        description_contains: when.contains.clone(),
        direction: when.direction.map(|d| match d {
            Direction::In => "in".to_owned(),
            Direction::Out => "out".to_owned(),
        }),
        account_id: account,
        amount_min: when.amount_min,
        amount_max: when.amount_max,
        category_id: rule
            .then
            .category
            .as_ref()
            .and_then(|c| categories.get(c).copied()),
        payee_id: rule
            .then
            .payee
            .as_ref()
            .and_then(|p| payees.get(p.trim()).copied()),
        payee_from_text: rule.then.payee_from_text,
    })
}

/// Adds the rule, or updates the one with its name where it differs.
async fn load_rule(
    db: &mut PgConnection,
    household_id: Uuid,
    rule: &NewRule,
    maps: &Maps,
    report: &mut LoadReport,
) -> Result<(), sqlx::Error> {
    let wanted = wanted(rule, maps)?;
    let existing: Option<Existing> = sqlx::query_as(
        "select id, description_contains, direction, account_id, amount_min, amount_max, category_id,
                payee_id, payee_from_text
         from rules where household_id = $1 and name = $2 order by position limit 1",
    )
    .bind(household_id)
    .bind(&rule.name)
    .fetch_optional(&mut *db)
    .await?;
    let id = match existing {
        Some(existing) if existing.stored == wanted => return Ok(()),
        Some(existing) => {
            report.rules_changed += 1;
            Some(existing.id)
        }
        None => {
            report.rules_added += 1;
            None
        }
    };
    let statement = if id.is_some() {
        "update rules set description_contains = $3, direction = $4, account_id = $5, amount_min = $6,
                          amount_max = $7, category_id = $8, payee_id = $9, payee_from_text = $10
         where id = $1 and household_id = $2"
    } else {
        "insert into rules (id, household_id, name, position, description_contains, direction, account_id,
                            amount_min, amount_max, category_id, payee_id, payee_from_text)
         values (gen_random_uuid(), $2, $11,
                 (select coalesce(max(position), 0) + 1 from rules where household_id = $2),
                 $3, $4, $5, $6, $7, $8, $9, $10)"
    };
    sqlx::query(statement)
        .bind(id)
        .bind(household_id)
        .bind(wanted.description_contains)
        .bind(wanted.direction)
        .bind(wanted.account_id)
        .bind(wanted.amount_min)
        .bind(wanted.amount_max)
        .bind(wanted.category_id)
        .bind(wanted.payee_id)
        .bind(wanted.payee_from_text)
        .bind(&rule.name)
        .execute(&mut *db)
        .await?;
    Ok(())
}

/// Adds the set's categories, or changes the kind of one that has the name.
async fn load_categories(
    db: &mut PgConnection,
    household_id: Uuid,
    set: &RuleSet,
    report: &mut LoadReport,
) -> Result<(), sqlx::Error> {
    for category in &set.categories {
        let kind = match category.kind {
            coinroost_rules::CategoryKind::Expense => "expense",
            coinroost_rules::CategoryKind::Income => "income",
            coinroost_rules::CategoryKind::Transfer => "transfer",
        };
        let row: Option<(bool,)> = sqlx::query_as(
            "insert into categories (household_id, name, kind) values ($1, $2, $3)
             on conflict (household_id, name) do update set kind = excluded.kind
                 where categories.kind is distinct from excluded.kind
             returning xmax = 0",
        )
        .bind(household_id)
        .bind(&category.name)
        .bind(kind)
        .fetch_optional(&mut *db)
        .await?;
        match row {
            Some((true,)) => report.categories_added += 1,
            Some((false,)) => report.categories_changed += 1,
            None => {}
        }
    }
    Ok(())
}

/// Adds the payees the set's rules name and the household does not have.
async fn load_payees(
    db: &mut PgConnection,
    household_id: Uuid,
    set: &RuleSet,
    report: &mut LoadReport,
) -> Result<(), sqlx::Error> {
    for name in set.rules.iter().filter_map(|r| r.then.payee.as_deref()) {
        let added: Option<(Uuid,)> = sqlx::query_as(
            "insert into payees (household_id, name) values ($1, $2) on conflict (household_id, name) do nothing
             returning id",
        )
        .bind(household_id)
        .bind(name.trim())
        .fetch_optional(&mut *db)
        .await?;
        report.payees_added += usize::from(added.is_some());
    }
    Ok(())
}

async fn load_in(
    db: &mut PgConnection,
    household_id: Uuid,
    set: &RuleSet,
) -> Result<LoadReport, sqlx::Error> {
    let mut report = LoadReport::default();
    sqlx::query("select id from households where id = $1 for update")
        .bind(household_id)
        .execute(&mut *db)
        .await?;
    load_categories(db, household_id, set, &mut report).await?;
    load_payees(db, household_id, set, &mut report).await?;
    let maps = Maps {
        categories: ids(
            db,
            "select name, id from categories where household_id = $1",
            household_id,
        )
        .await?,
        accounts: ids(
            db,
            "select namespace, id from accounts where household_id = $1",
            household_id,
        )
        .await?,
        payees: ids(
            db,
            "select name, id from payees where household_id = $1",
            household_id,
        )
        .await?,
    };
    sqlx::query("set constraints all deferred")
        .execute(&mut *db)
        .await?;
    for rule in &set.rules {
        load_rule(db, household_id, rule, &maps, &mut report).await?;
    }
    Ok(report)
}

async fn ids(
    db: &mut PgConnection,
    statement: &'static str,
    household_id: Uuid,
) -> Result<HashMap<String, Uuid>, sqlx::Error> {
    Ok(sqlx::query_as::<_, (String, Uuid)>(statement)
        .bind(household_id)
        .fetch_all(db)
        .await?
        .into_iter()
        .collect())
}
