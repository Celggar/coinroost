//! `firefly check`: every mapped account's balance at each month end against Firefly's own figure. Firefly's
//! `current_balance` at a date is the sum of the account's journal rows in its currency up to the end of that day
//! (the virtual balance is left out), so it compares directly with the sum of the account's legs.

use sqlx::PgPool;
use uuid::Uuid;

use super::amount::minor_units;
use super::plan::Map;
use super::snapshot::Snapshot;

/// The comparison, one line per account and month end, and how many differ.
pub async fn check(
    db: &PgPool,
    household: Uuid,
    snapshot: &Snapshot,
    map: &Map,
) -> Result<(String, usize), String> {
    let mut lines = Vec::new();
    let mut differ = 0;
    for balance in &snapshot.balances {
        let Some(mapped) = map.accounts.get(&balance.account_id) else {
            continue;
        };
        let firefly = snapshot.accounts.iter().find(|a| a.id == balance.account_id);
        let digits = firefly.and_then(|a| a.digits).unwrap_or(2);
        let expected = minor_units(&balance.amount, digits)?;
        let held: Option<(i64,)> = sqlx::query_as(
            "select coalesce(sum(l.amount_minor), 0)::bigint from accounts a
             left join legs l on l.account_id = a.id and l.posted_on <= $3
             where a.household_id = $1 and a.namespace = $2 group by a.id",
        )
        .bind(household)
        .bind(&mapped.namespace)
        .bind(balance.date)
        .fetch_optional(db)
        .await
        .map_err(|e| e.to_string())?;
        let Some((held,)) = held else {
            return Err(format!(
                "{} is not imported yet (namespace {})",
                mapped.name, mapped.namespace
            ));
        };
        let verdict = if held == expected {
            "ok"
        } else {
            differ += 1;
            "DIFFERS"
        };
        lines.push(format!(
            "{:<30} {}  Firefly {:>14}  Coinroost {:>14}  {verdict}",
            mapped.name, balance.date, expected, held
        ));
    }
    if lines.is_empty() {
        return Err("the snapshot has no month-end balance of a mapped account".to_owned());
    }
    lines.push(format!(
        "{} month ends compared, {differ} differ (amounts in minor units)",
        lines.len()
    ));
    Ok((lines.join("\n"), differ))
}
