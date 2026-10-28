//! The household's rules, as stored, run through `coinroost-rules`: when a file is imported, and when a person
//! asks to apply them again. Applying never touches a category or payee a person set, and only transactions with
//! one leg are sorted (two-leg transfers are paired, not categorized).

use std::collections::HashMap;
use std::fmt::Write;
use std::sync::LazyLock;

use coinroost_rules::{Direction, Payee, PayeeAction, PayeeText, Rule, Transaction, sort};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgExecutor};
use time::Date;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};

static PAYEE_TEXT: LazyLock<Option<PayeeText>> = LazyLock::new(|| PayeeText::builtin("es-PA"));

/// The household's enabled rules, in order.
pub struct Sorter {
    rules: Vec<Rule<Uuid, Uuid, Uuid>>,
    ids: Vec<Uuid>,
}

/// The payee a rule gives: one the household has, or a name read from the text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PayeeChoice {
    Id(Uuid),
    Name(String),
}

/// What the first matching rule sets.
pub struct Decision {
    pub rule_id: Uuid,
    pub category_id: Option<Uuid>,
    pub payee: Option<PayeeChoice>,
}

type RuleRow = (
    Uuid,
    Option<String>,
    Option<String>,
    Option<Uuid>,
    Option<i64>,
    Option<i64>,
    Option<Uuid>,
    Option<Uuid>,
    bool,
);

impl Sorter {
    pub async fn load(db: impl PgExecutor<'_>, household_id: Uuid) -> ApiResult<Sorter> {
        let rows: Vec<RuleRow> = sqlx::query_as(
            "select id, description_contains, direction, account_id, amount_min, amount_max, category_id, payee_id,
                    payee_from_text
             from rules where household_id = $1 and enabled order by position",
        )
        .bind(household_id)
        .fetch_all(db)
        .await?;
        let mut sorter = Sorter {
            rules: Vec::with_capacity(rows.len()),
            ids: Vec::with_capacity(rows.len()),
        };
        for (id, contains, direction, account, min, max, category, payee, from_text) in rows {
            let direction = match direction.as_deref() {
                Some("in") => Some(Direction::In),
                Some("out") => Some(Direction::Out),
                _ => None,
            };
            let payee = match (payee, from_text) {
                (_, true) => PayeeAction::FromText,
                (Some(payee), false) => PayeeAction::Fixed(payee),
                (None, false) => PayeeAction::None,
            };
            let rule = Rule::new(contains, direction, account, min, max, category, payee)
                .map_err(|e| ApiError::Internal(format!("stored rule {id}: {e}")))?;
            sorter.rules.push(rule);
            sorter.ids.push(id);
        }
        Ok(sorter)
    }

    pub fn decide(
        &self,
        description: &str,
        amount_minor: i64,
        account_id: Uuid,
    ) -> ApiResult<Option<Decision>> {
        let text = PAYEE_TEXT
            .as_ref()
            .ok_or_else(|| ApiError::Internal("the payee locale does not load".to_owned()))?;
        let transaction = Transaction {
            description,
            amount_minor,
            account: Some(&account_id),
        };
        Ok(sort(&self.rules, &transaction, text).map(|outcome| Decision {
            rule_id: self.ids[outcome.rule],
            category_id: outcome.category.copied(),
            payee: outcome.payee.map(|payee| match payee {
                Payee::Fixed(id) => PayeeChoice::Id(*id),
                Payee::Named(name) => PayeeChoice::Name(name),
            }),
        }))
    }
}

/// The household's payee with this name, created if it does not exist yet.
pub async fn payee_named(db: &mut PgConnection, household_id: Uuid, name: &str) -> ApiResult<Uuid> {
    let (id,): (Uuid,) = sqlx::query_as(
        "insert into payees (household_id, name) values ($1, $2)
         on conflict (household_id, name) do update set name = excluded.name returning id",
    )
    .bind(household_id)
    .bind(name)
    .fetch_one(db)
    .await?;
    Ok(id)
}

/// A transaction's category, payee and the rule that set them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sorted {
    pub category_id: Option<Uuid>,
    pub payee: Option<PayeeChoice>,
    pub rule_id: Option<Uuid>,
}

/// What applying the rules again would change in one transaction.
pub struct Change {
    pub transaction_id: Uuid,
    pub date: Date,
    pub description: String,
    pub amount_minor: i64,
    pub before: Sorted,
    pub after: Sorted,
    /// Whether a person set the category or the payee: those stay.
    pub category_manual: bool,
    pub payee_manual: bool,
}

type Candidate = (
    Uuid,
    Date,
    String,
    Option<Uuid>,
    Option<String>,
    Option<Uuid>,
    Option<String>,
    Option<Uuid>,
    i64,
    Uuid,
);

/// Every change applying the rules would make now, oldest transaction first.
pub async fn changes(db: &mut PgConnection, household_id: Uuid) -> ApiResult<Vec<Change>> {
    let sorter = Sorter::load(&mut *db, household_id).await?;
    let names: HashMap<String, Uuid> =
        sqlx::query_as::<_, (String, Uuid)>("select name, id from payees where household_id = $1")
            .bind(household_id)
            .fetch_all(&mut *db)
            .await?
            .into_iter()
            .collect();
    let candidates: Vec<Candidate> = sqlx::query_as(
        "select t.id, t.date, t.description, t.category_id, t.category_set_by, t.payee_id, t.payee_set_by,
                t.rule_id, l.amount_minor, l.account_id
         from transactions t join legs l on l.transaction_id = t.id
         where t.household_id = $1
           and (t.category_set_by is distinct from 'manual' or t.payee_set_by is distinct from 'manual')
           and (select count(*) from legs o where o.transaction_id = t.id) = 1
         order by t.date, t.id",
    )
    .bind(household_id)
    .fetch_all(&mut *db)
    .await?;
    let mut changes = Vec::new();
    for (id, date, description, category, category_by, payee, payee_by, rule, amount, account) in candidates {
        let category_manual = category_by.as_deref() == Some("manual");
        let payee_manual = payee_by.as_deref() == Some("manual");
        let decision = sorter.decide(&description, amount, account)?;
        let rule_category = decision.as_ref().and_then(|d| d.category_id);
        let rule_payee = decision.as_ref().and_then(|d| d.payee.clone()).map(|p| match p {
            PayeeChoice::Name(name) => names
                .get(&name)
                .map_or(PayeeChoice::Name(name), |id| PayeeChoice::Id(*id)),
            known @ PayeeChoice::Id(_) => known,
        });
        let after_category = if category_manual { category } else { rule_category };
        let after_payee = if payee_manual {
            payee.map(PayeeChoice::Id)
        } else {
            rule_payee
        };
        let ruled =
            (!category_manual && after_category.is_some()) || (!payee_manual && after_payee.is_some());
        let after = Sorted {
            category_id: after_category,
            payee: after_payee,
            rule_id: decision.filter(|_| ruled).map(|d| d.rule_id),
        };
        let before = Sorted {
            category_id: category,
            payee: payee.map(PayeeChoice::Id),
            rule_id: rule,
        };
        if before != after {
            changes.push(Change {
                transaction_id: id,
                date,
                description,
                amount_minor: amount,
                before,
                after,
                category_manual,
                payee_manual,
            });
        }
    }
    Ok(changes)
}

/// A strong validator (RFC 9110 § 8.8.3) of a change set: equal sets give equal tags.
pub fn entity_tag(changes: &[Change]) -> String {
    let mut hash = Sha256::new();
    for change in changes {
        let payee = match &change.after.payee {
            Some(PayeeChoice::Id(id)) => format!("id:{id}"),
            Some(PayeeChoice::Name(name)) => format!("name:{name}"),
            None => String::new(),
        };
        let category = change
            .after
            .category_id
            .map(|id| id.to_string())
            .unwrap_or_default();
        let rule = change.after.rule_id.map(|id| id.to_string()).unwrap_or_default();
        hash.update(format!("{}|{category}|{payee}|{rule}\n", change.transaction_id));
    }
    let digest = hash.finalize();
    let hex = digest
        .iter()
        .take(16)
        .fold(String::with_capacity(32), |mut hex, b| {
            let _ = write!(hex, "{b:02x}");
            hex
        });
    format!("\"{hex}\"")
}

/// Writes one change.
pub async fn write(db: &mut PgConnection, household_id: Uuid, change: &Change) -> ApiResult<()> {
    let payee_id = match &change.after.payee {
        Some(PayeeChoice::Id(id)) => Some(*id),
        Some(PayeeChoice::Name(name)) => Some(payee_named(&mut *db, household_id, name).await?),
        None => None,
    };
    let by = |manual: bool, set: bool| match (manual, set) {
        (true, _) => Some("manual"),
        (false, true) => Some("rule"),
        (false, false) => None,
    };
    sqlx::query(
        "update transactions set category_id = $3, category_set_by = $4, payee_id = $5, payee_set_by = $6,
                                 rule_id = $7
         where id = $1 and household_id = $2",
    )
    .bind(change.transaction_id)
    .bind(household_id)
    .bind(change.after.category_id)
    .bind(by(change.category_manual, change.after.category_id.is_some()))
    .bind(payee_id)
    .bind(by(change.payee_manual, payee_id.is_some()))
    .bind(change.after.rule_id)
    .execute(db)
    .await?;
    Ok(())
}
