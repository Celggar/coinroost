//! What each employer still owes, fortnight by fortnight.

use axum::Json;
use axum::extract::{Query, State};
use coinroost_payroll::{Fortnight, Status};
use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Session;
use crate::employers::{self, Owing, Paid};
use crate::error::{ApiResult, Problem};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(payroll))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AsOf {
    /// The day to measure at (today in UTC when absent): send the person's today.
    as_of: Option<Date>,
}

/// A salary fortnight: the 1st to the 15th (half 1) or the 16th to the end of the month (half 2).
#[derive(Serialize, ToSchema)]
pub struct FortnightView {
    year: i32,
    month: u8,
    half: u8,
    first_day: Date,
    last_day: Date,
    /// The day after it ends.
    due_on: Date,
}

impl From<Fortnight> for FortnightView {
    fn from(f: Fortnight) -> FortnightView {
        FortnightView {
            year: f.year,
            month: f.month,
            half: f.half,
            first_day: f.first_day(),
            last_day: f.last_day(),
            due_on: f.due_on(),
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FortnightStatus {
    /// Due on or before the account's last data and not paid.
    Owed,
    /// Due after the account's last data: not known yet whether it was paid.
    NoData,
    Late,
    OnTime,
    /// Paid before it was due.
    Ahead,
}

impl From<Status> for FortnightStatus {
    fn from(s: Status) -> FortnightStatus {
        match s {
            Status::Owed => FortnightStatus::Owed,
            Status::NoData => FortnightStatus::NoData,
            Status::Late => FortnightStatus::Late,
            Status::OnTime => FortnightStatus::OnTime,
            Status::Ahead => FortnightStatus::Ahead,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct Payment {
    transaction_id: Uuid,
    date: Date,
    amount_minor: i64,
    description: String,
}

impl From<&Paid> for Payment {
    fn from(p: &Paid) -> Payment {
        Payment {
            transaction_id: p.transaction_id,
            date: p.date,
            amount_minor: p.amount_minor,
            description: p.description.clone(),
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct FortnightLine {
    fortnight: FortnightView,
    status: FortnightStatus,
    /// Days late (zero or below: on time); for an owed fortnight, days due at the last data.
    days: Option<i64>,
    payment: Option<Payment>,
    /// The fortnight the bank's description named, when it named one.
    label: Option<FortnightView>,
}

#[derive(Serialize, ToSchema)]
pub struct EmployerPayroll {
    employer_id: Uuid,
    name: String,
    account_id: Uuid,
    currency: String,
    /// What a fortnight is worth today.
    amount_minor: i64,
    /// The first day of the first fortnight the employer pays.
    start: Date,
    /// When the deposits could not be read (no deposit counted since `start`), why; the other fields then stay
    /// empty.
    error: Option<String>,
    /// Due and unpaid, oldest first.
    owed: Vec<FortnightView>,
    /// `amount_minor` times the owed fortnights.
    owed_total_minor: i64,
    ahead: Vec<FortnightView>,
    /// Due after the last data of the account.
    unknown: Vec<FortnightView>,
    /// Deposits that counted as a fortnight's pay.
    paid: usize,
    next_due: Option<FortnightView>,
    /// The date "owed" is measured at: `as_of`, or the account's last data when earlier.
    data_until: Option<Date>,
    /// From the first fortnight to the last one due or paid.
    history: Vec<FortnightLine>,
}

#[derive(Serialize, ToSchema)]
pub struct Payroll {
    as_of: Date,
    employers: Vec<EmployerPayroll>,
}

fn view(owing: Owing) -> EmployerPayroll {
    let e = owing.employer;
    let start = Fortnight::containing(e.start_month).first_day();
    let mut out = EmployerPayroll {
        employer_id: e.id,
        name: e.name,
        account_id: e.account_id,
        currency: e.currency,
        amount_minor: e.amount_minor,
        start,
        error: None,
        owed: Vec::new(),
        owed_total_minor: 0,
        ahead: Vec::new(),
        unknown: Vec::new(),
        paid: 0,
        next_due: None,
        data_until: None,
        history: Vec::new(),
    };
    let Ok((report, paid)) = owing.outcome else {
        out.error = Some(format!(
            "no deposit counted since {start}: check the match text, the account and the minimum amount"
        ));
        return out;
    };
    let owed_count = i64::try_from(report.owed.len()).unwrap_or(i64::MAX);
    out.owed_total_minor = e.amount_minor.saturating_mul(owed_count);
    out.owed = report.owed.into_iter().map(FortnightView::from).collect();
    out.ahead = report.ahead.into_iter().map(FortnightView::from).collect();
    out.unknown = report.unknown.into_iter().map(FortnightView::from).collect();
    out.paid = report.paid;
    out.next_due = Some(report.next_due.into());
    out.data_until = Some(report.data_until);
    out.history = report
        .history
        .into_iter()
        .map(|s| FortnightLine {
            fortnight: s.fortnight.into(),
            status: s.status.into(),
            days: s.days,
            payment: s.deposit.map(|i| Payment::from(&paid[i])),
            label: s.label.map(FortnightView::from),
        })
        .collect();
    out
}

/// What each employer owes at a date: one report per employer, or the reason it has none.
#[utoipa::path(get, path = "/api/v1/payroll", tag = "payroll", params(AsOf),
    responses(
        (status = OK, description = "Each employer's fortnights", body = Payroll),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem)))]
async fn payroll(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<AsOf>,
) -> ApiResult<Json<Payroll>> {
    let as_of = query.as_of.unwrap_or_else(|| OffsetDateTime::now_utc().date());
    let owing = employers::owing(&state.db, session.household_id, as_of).await?;
    Ok(Json(Payroll {
        as_of,
        employers: owing.into_iter().map(view).collect(),
    }))
}
