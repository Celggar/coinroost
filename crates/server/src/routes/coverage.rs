//! Coverage: for each account, up to when it has data and which stretches were imported, to answer «what do I
//! still have to download from the bank?» before uploading anything.
//!
//! The thresholds are a visual aid, not a verdict: a statement cycle is about 30 days, so more than 40 days
//! without a single transaction almost always means a statement was not imported, while 15 is the early warning.
//! A quiet account (savings that only gets a monthly deposit) can be legitimately still.

use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Session;
use crate::error::{ApiResult, Problem};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(coverage))
}

/// More days than this without data is worth a look.
const WARNING_DAYS: i64 = 15;
/// More days than this without data almost always means a missing statement.
const LATE_DAYS: i64 = 40;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct Today {
    /// The caller's date; the server's UTC date when absent.
    today: Option<Date>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Delay {
    /// More than 40 days.
    Late,
    /// More than 15 days.
    Warning,
    /// The account has no transactions.
    NoData,
    UpToDate,
}

fn delay(days: Option<i64>) -> Delay {
    match days {
        None => Delay::NoData,
        Some(d) if d > LATE_DAYS => Delay::Late,
        Some(d) if d > WARNING_DAYS => Delay::Warning,
        Some(_) => Delay::UpToDate,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
pub struct DateRange {
    from: Date,
    to: Date,
}

/// Days with no imported file between two imported ranges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
pub struct Gap {
    /// First day without data.
    from: Date,
    /// Last day without data.
    to: Date,
    days: i64,
    delay: Delay,
}

#[derive(Serialize, ToSchema)]
pub struct AccountCoverage {
    account_id: Uuid,
    name: String,
    /// The date of its latest transaction, imported or not.
    latest: Option<Date>,
    /// Days from `latest` to today.
    days_since: Option<i64>,
    delay: Delay,
    /// What the imported files cover, oldest first; files less than 16 days apart count as one range.
    ranges: Vec<DateRange>,
    /// The holes between those ranges.
    gaps: Vec<Gap>,
}

#[derive(Serialize, ToSchema)]
pub struct Coverage {
    today: Date,
    /// Every account, the most behind first.
    items: Vec<AccountCoverage>,
}

/// Up to when each account has data, what its imported files cover and where the holes are.
#[utoipa::path(get, path = "/api/v1/coverage", tag = "imports", params(Today),
    responses(
        (status = OK, description = "Coverage of every account", body = Coverage),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem)))]
async fn coverage(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<Today>,
) -> ApiResult<Json<Coverage>> {
    let today = query.today.unwrap_or_else(|| OffsetDateTime::now_utc().date());
    let accounts: Vec<(Uuid, String, Option<Date>)> = sqlx::query_as(
        "select a.id, a.name, (select max(l.posted_on) from legs l where l.account_id = a.id)
         from accounts a where a.household_id = $1 order by a.name",
    )
    .bind(session.household_id)
    .fetch_all(&state.db)
    .await?;
    let batches: Vec<(Uuid, Date, Date)> = sqlx::query_as(
        "select account_id, first_date, last_date from import_batches
         where household_id = $1 and first_date is not null order by account_id, first_date",
    )
    .bind(session.household_id)
    .fetch_all(&state.db)
    .await?;
    let mut items: Vec<AccountCoverage> = accounts
        .into_iter()
        .map(|(account_id, name, latest)| {
            let spans = batches
                .iter()
                .filter(|b| b.0 == account_id)
                .map(|b| DateRange { from: b.1, to: b.2 });
            let (ranges, gaps) = merge(spans);
            let days_since = latest.map(|d| (today - d).whole_days());
            AccountCoverage {
                account_id,
                name,
                latest,
                days_since,
                delay: delay(days_since),
                ranges,
                gaps,
            }
        })
        .collect();
    items.sort_by(|a, b| {
        (a.delay, std::cmp::Reverse(a.days_since)).cmp(&(b.delay, std::cmp::Reverse(b.days_since)))
    });
    Ok(Json(Coverage { today, items }))
}

/// Merges imported ranges whose hole is at most [`WARNING_DAYS`] (statements of one account rarely end and start
/// on consecutive days), and lists the longer holes as gaps.
fn merge(spans: impl Iterator<Item = DateRange>) -> (Vec<DateRange>, Vec<Gap>) {
    let mut spans: Vec<DateRange> = spans.collect();
    spans.sort_by_key(|s| (s.from, s.to));
    let mut ranges: Vec<DateRange> = Vec::new();
    let mut gaps = Vec::new();
    for span in spans {
        match ranges.last_mut() {
            Some(last) => {
                let hole = (span.from - last.to).whole_days() - 1;
                if hole <= WARNING_DAYS {
                    last.to = last.to.max(span.to);
                } else {
                    gaps.push(Gap {
                        from: last.to.next_day().unwrap_or(last.to),
                        to: span.from.previous_day().unwrap_or(span.from),
                        days: hole,
                        delay: delay(Some(hole)),
                    });
                    ranges.push(span);
                }
            }
            None => ranges.push(span),
        }
    }
    (ranges, gaps)
}

#[cfg(test)]
mod tests {
    use time::macros::date;

    use super::*;

    fn range(from: Date, to: Date) -> DateRange {
        DateRange { from, to }
    }

    #[test]
    fn coverage_merges_ranges_and_reports_gaps() {
        let spans = [
            range(date!(2026 - 03 - 02), date!(2026 - 03 - 30)),
            // Overlaps the first: one range.
            range(date!(2026 - 03 - 20), date!(2026 - 04 - 10)),
            // 15 days without data in between: still one range.
            range(date!(2026 - 04 - 26), date!(2026 - 05 - 28)),
            // June is missing: 34 days without data.
            range(date!(2026 - 07 - 02), date!(2026 - 07 - 29)),
            // Months later: a late gap.
            range(date!(2026 - 10 - 01), date!(2026 - 10 - 20)),
        ];
        let (ranges, gaps) = merge(spans.into_iter().rev());
        assert_eq!(
            ranges,
            [
                range(date!(2026 - 03 - 02), date!(2026 - 05 - 28)),
                range(date!(2026 - 07 - 02), date!(2026 - 07 - 29)),
                range(date!(2026 - 10 - 01), date!(2026 - 10 - 20)),
            ]
        );
        assert_eq!(
            gaps,
            [
                Gap {
                    from: date!(2026 - 05 - 29),
                    to: date!(2026 - 07 - 01),
                    days: 34,
                    delay: Delay::Warning
                },
                Gap {
                    from: date!(2026 - 07 - 30),
                    to: date!(2026 - 09 - 30),
                    days: 63,
                    delay: Delay::Late
                },
            ]
        );
        // A 16-day hole is a gap.
        let (_, gaps) = merge(
            [
                range(date!(2026 - 01 - 01), date!(2026 - 01 - 10)),
                range(date!(2026 - 01 - 27), date!(2026 - 01 - 31)),
            ]
            .into_iter(),
        );
        assert_eq!(gaps.iter().map(|g| g.days).collect::<Vec<_>>(), [16]);
    }

    #[test]
    fn delay_follows_the_thresholds() {
        let cases = [
            (None, Delay::NoData),
            (Some(0), Delay::UpToDate),
            (Some(15), Delay::UpToDate),
            (Some(16), Delay::Warning),
            (Some(40), Delay::Warning),
            (Some(41), Delay::Late),
        ];
        for (days, expected) in cases {
            assert_eq!(delay(days), expected, "{days:?}");
        }
    }
}
