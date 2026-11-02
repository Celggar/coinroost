//! Pages of the transaction list: the rows a filter matches, counted for `total`, and the cursor. A cursor holds
//! where a page ended in the list's order (date, then the time the row was recorded, both newest first, then id),
//! so the next page starts right after it: rows that share a date and an import time keep their places, and a
//! row deleted between pages does not shift the rest.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use time::macros::format_description;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use super::Transaction;
use crate::error::ApiError;

/// The rows that match a `Filter`: `$1` the household, `$2`–`$7` the filters, in [`bind_filter`]'s order.
macro_rules! matching {
    () => {
        " from transactions t
         where t.household_id = $1
           and ($2::uuid is null or exists (select 1 from legs l where l.transaction_id = t.id and l.account_id = $2))
           and ($3::date is null or t.date >= $3) and ($4::date is null or t.date <= $4)
           and (not $5 or t.category_id is null)
           and (not $6 or (select count(*) from legs l where l.transaction_id = t.id) = 2)
           and ($7::text is null or strpos(lower(t.description), lower($7)) > 0)"
    };
}

/// Binds `$1`–`$7` of [`matching`].
macro_rules! bind_filter {
    ($query:expr, $household:expr, $filter:expr, $text:expr) => {
        $query
            .bind($household)
            .bind($filter.account_id)
            .bind($filter.from)
            .bind($filter.to)
            .bind($filter.uncategorized)
            .bind($filter.transfers)
            .bind($text)
    };
}

pub struct Cursor {
    pub date: Date,
    pub created_at: OffsetDateTime,
    pub id: Uuid,
}

impl Cursor {
    /// The text handed out as `next_cursor`; opaque to clients.
    pub fn encode(&self) -> String {
        let micros = self.created_at.unix_timestamp_nanos() / 1000;
        URL_SAFE_NO_PAD.encode(format!("{}.{micros}.{}", self.date, self.id))
    }

    /// A cursor this API gave; anything else is refused, never read as the first page.
    pub fn decode(text: &str) -> Result<Cursor, ApiError> {
        let invalid = || {
            ApiError::invalid(
                "cursor_invalid",
                "the cursor was not given by this API; pass next_cursor as it came",
            )
        };
        let plain = URL_SAFE_NO_PAD.decode(text).map_err(|_| invalid())?;
        let plain = String::from_utf8(plain).map_err(|_| invalid())?;
        let mut parts = plain.split('.');
        let (Some(date), Some(micros), Some(id), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(invalid());
        };
        let date = Date::parse(date, format_description!("[year]-[month]-[day]")).map_err(|_| invalid())?;
        let nanos = micros
            .parse::<i128>()
            .ok()
            .and_then(|m| m.checked_mul(1000))
            .ok_or_else(invalid)?;
        let created_at = OffsetDateTime::from_unix_timestamp_nanos(nanos).map_err(|_| invalid())?;
        let id = Uuid::parse_str(id).map_err(|_| invalid())?;
        Ok(Cursor { date, created_at, id })
    }
}

/// Takes the look-ahead row off a page fetched with one row more than `limit`: the cursor of the page's last row when
/// more follow, `None` when the page is the end.
pub fn cut(items: &mut Vec<Transaction>, limit: i64) -> Option<String> {
    let page = usize::try_from(limit).unwrap_or(usize::MAX);
    if items.len() <= page {
        return None;
    }
    items.truncate(page);
    items.last().map(|t| {
        Cursor {
            date: t.date,
            created_at: t.created_at,
            id: t.id,
        }
        .encode()
    })
}
