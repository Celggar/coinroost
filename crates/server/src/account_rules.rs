//! Loads statement detection onto a household's existing accounts, by namespace, from an account rules file
//! (`accounts.toml`, the file `coinroost detect --accounts` reads). Accounts made by a Firefly migration have a
//! profile but no detection, so two accounts that share an export format (two cards, two savings accounts) come
//! back ambiguous when a file is dropped. Values are checked as `POST /api/v1/accounts` checks them.

use std::collections::HashSet;

use coinroost_normalizer::detect::AccountRules;
use sqlx::PgPool;
use uuid::Uuid;

use crate::routes::accounts::{Detection, check_profile};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct LoadReport {
    pub changed: usize,
    pub unchanged: usize,
}

/// Sets each rule's profile and detection on the household's account with that namespace. All or nothing: an
/// unknown namespace, a namespace listed twice, another profile already on the account or a value the API would
/// refuse leaves every account as it was. Loading the same file again changes nothing.
pub async fn load(db: &PgPool, household_id: Uuid, rules: &AccountRules) -> Result<LoadReport, String> {
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    let mut report = LoadReport::default();
    let mut seen = HashSet::new();
    for rule in &rules.account {
        let fail = |detail: String| format!("account {}: {detail}", rule.namespace);
        if !seen.insert(rule.namespace.as_str()) {
            return Err(fail("listed twice".to_owned()));
        }
        let account: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
            "select id, currency, profile from accounts where household_id = $1 and namespace = $2",
        )
        .bind(household_id)
        .bind(&rule.namespace)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        let Some((id, currency, profile)) = account else {
            return Err(fail("no account with this namespace in the household".to_owned()));
        };
        if let Some(current) = profile.filter(|p| *p != rule.profile) {
            return Err(fail(format!(
                "it reads with profile {current}, the file says {}",
                rule.profile
            )));
        }
        check_profile(&rule.profile, &currency).map_err(|e| fail(e.to_string()))?;
        let (markers, card_prefixes, label, number_hash) = Detection::from_rule(rule)
            .map_err(|e| fail(e.to_string()))?
            .into_columns();
        let updated = sqlx::query(
            "update accounts set profile = $2, detect_markers = $3, detect_card_prefixes = $4, detect_label = $5,
                                 detect_number_hash = $6
             where id = $1
               and (profile, detect_markers, detect_card_prefixes, detect_label, detect_number_hash)
                   is distinct from ($2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(&rule.profile)
        .bind(markers)
        .bind(card_prefixes)
        .bind(label)
        .bind(number_hash)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        if updated.rows_affected() == 1 {
            report.changed += 1;
        } else {
            report.unchanged += 1;
        }
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(report)
}
