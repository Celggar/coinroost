//! Pages of the rule list: the rules whose name or text contains `q`, in their order, counted for `total`. A cursor
//! holds the last rule a page showed, and the next page starts right after wherever that rule is now. Adding,
//! deleting or moving another rule between two pages shifts the positions, yet the next page still holds the rules
//! that followed (a rule moved across that last one shows up on the side it moved to). Moving the last rule itself
//! moves where the next page starts; deleting it starts the next page after the position it had.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use sqlx::PgPool;
use utoipa::IntoParams;
use uuid::Uuid;

use super::{Rule, Rules};
use crate::error::{ApiError, ApiResult};
use crate::validate;

const MAX_LIMIT: i64 = 200;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct RuleFilter {
    /// Only rules whose name, or the text they look for, contains this text, upper or lower case alike.
    q: Option<String>,
    /// At most this many, first one first (1 to 200); every rule that matches when absent.
    limit: Option<i64>,
    /// The `next_cursor` of the previous page, with the same `q`: the rules that follow it.
    cursor: Option<String>,
}

/// The rules that match: `$1` the household, `$2` the text.
macro_rules! matching {
    () => {
        " from rules r
         where r.household_id = $1
           and ($2::text is null or strpos(lower(r.name), lower($2)) > 0
                or strpos(lower(coalesce(r.description_contains, '')), lower($2)) > 0)"
    };
}

struct Cursor {
    position: i32,
    id: Uuid,
}

impl Cursor {
    /// The text handed out as `next_cursor`; opaque to clients.
    fn encode(rule: &Rule) -> String {
        URL_SAFE_NO_PAD.encode(format!("rule.{}.{}", rule.position, rule.id))
    }

    /// A cursor this API gave for rules; anything else is refused, never read as the first page.
    fn decode(text: &str) -> ApiResult<Cursor> {
        let invalid = || {
            ApiError::invalid(
                "cursor_invalid",
                "the cursor was not given by this API for rules; pass next_cursor as it came",
            )
        };
        let plain = URL_SAFE_NO_PAD.decode(text).map_err(|_| invalid())?;
        let plain = String::from_utf8(plain).map_err(|_| invalid())?;
        let mut parts = plain.split('.');
        let (Some("rule"), Some(position), Some(id), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(invalid());
        };
        let position = position
            .parse::<i32>()
            .ok()
            .filter(|p| *p >= 1)
            .ok_or_else(invalid)?;
        let id = Uuid::parse_str(id).map_err(|_| invalid())?;
        Ok(Cursor { position, id })
    }
}

/// The rules `filter` asks for, with how many match and the cursor of the rest.
pub(super) async fn list(db: &PgPool, household_id: Uuid, filter: &RuleFilter) -> ApiResult<Rules> {
    let text = validate::search(filter.q.as_deref())?;
    let limit = filter.limit.map(|l| validate::limit(l, MAX_LIMIT)).transpose()?;
    let after = filter.cursor.as_deref().map(Cursor::decode).transpose()?;
    let total: i64 = sqlx::query_scalar(concat!("select count(*)", matching!()))
        .bind(household_id)
        .bind(text)
        .fetch_one(db)
        .await?;
    // One rule past the page says whether another page follows; no limit is every rule.
    let mut items: Vec<Rule> = sqlx::query_as(concat!(
        "select ",
        columns!(),
        matching!(),
        "
           and ($3::integer is null or r.position > coalesce(
                (select a.position from rules a where a.id = $4 and a.household_id = $1), $3))
         order by r.position limit $5"
    ))
    .bind(household_id)
    .bind(text)
    .bind(after.as_ref().map(|c| c.position))
    .bind(after.as_ref().map(|c| c.id))
    .bind(limit.map(|l| l + 1))
    .fetch_all(db)
    .await?;
    let page = limit.and_then(|l| usize::try_from(l).ok()).unwrap_or(usize::MAX);
    let next_cursor = if items.len() > page {
        items.truncate(page);
        items.last().map(Cursor::encode)
    } else {
        None
    };
    Ok(Rules {
        items,
        total,
        next_cursor,
    })
}
