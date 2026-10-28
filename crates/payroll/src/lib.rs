//! Which salary fortnights an employer still owes. A fortnight is the 1st to the 15th (I) or the 16th to the end of
//! the month (II), and it is due the day after it ends. Each salary deposit settles one fortnight:
//! - a deposit whose description names its fortnight ("II Qna de Septiembre 2025") settles that one, the first by
//!   date when two name the same;
//! - every other deposit settles the newest fortnight still free from the one it is nearest to, going back, so the
//!   oldest stay unpaid last; when everything back to the start is paid, it is paid ahead and settles the next free
//!   one.
//!
//! Only deposits from the employer, into its account, of at least the minimum amount count (a 13th-month bonus is
//! smaller and settles nothing). A fortnight due after the last data of the account is unknown, never owed: its
//! payment may be in a statement not imported yet. An employer with no deposit at all is an error, not "owes
//! everything".
//!
//! Pure: no clock and no database. `as_of` and the account's last data date come in.

mod label;

use std::collections::{BTreeMap, BTreeSet};

pub use label::{Labels, normalize};

use time::{Date, Duration, Month};

/// A salary fortnight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fortnight {
    pub year: i32,
    pub month: u8,
    /// 1 for the 1st to the 15th, 2 for the 16th to the end of the month.
    pub half: u8,
}

fn date(year: i32, month: u8, day: u8) -> Date {
    let month = Month::try_from(month).expect("months are 1 to 12");
    Date::from_calendar_date(year, month, day).expect("a valid day")
}

impl Fortnight {
    /// # Panics
    /// When the month is not 1 to 12 or the half not 1 or 2.
    pub fn new(year: i32, month: u8, half: u8) -> Fortnight {
        assert!(
            (1..=12).contains(&month) && (1..=2).contains(&half),
            "a month 1–12 and a half 1–2"
        );
        Fortnight { year, month, half }
    }

    pub fn containing(day: Date) -> Fortnight {
        Fortnight::new(
            day.year(),
            u8::from(day.month()),
            if day.day() <= 15 { 1 } else { 2 },
        )
    }

    pub fn first_day(self) -> Date {
        date(self.year, self.month, if self.half == 1 { 1 } else { 16 })
    }

    /// # Panics
    /// Never for a fortnight made with [`Fortnight::new`].
    pub fn last_day(self) -> Date {
        if self.half == 1 {
            date(self.year, self.month, 15)
        } else {
            let month = Month::try_from(self.month).expect("months are 1 to 12");
            date(self.year, self.month, month.length(self.year))
        }
    }

    /// The day after it ends.
    pub fn due_on(self) -> Date {
        self.last_day() + Duration::days(1)
    }

    #[must_use]
    pub fn previous(self) -> Fortnight {
        match (self.half, self.month) {
            (2, _) => Fortnight::new(self.year, self.month, 1),
            (_, 1) => Fortnight::new(self.year - 1, 12, 2),
            _ => Fortnight::new(self.year, self.month - 1, 2),
        }
    }

    #[must_use]
    pub fn next(self) -> Fortnight {
        match (self.half, self.month) {
            (1, _) => Fortnight::new(self.year, self.month, 2),
            (_, 12) => Fortnight::new(self.year + 1, 1, 1),
            _ => Fortnight::new(self.year, self.month + 1, 1),
        }
    }

    /// The fortnight a payment on that day is nearest to: the one whose end is closest. A payment on 5 March is
    /// February's II four days late, not March's I ten days early.
    pub fn reference(paid_on: Date) -> Fortnight {
        let current = Fortnight::containing(paid_on);
        let before = current.previous();
        if (current.last_day() - paid_on) <= (paid_on - before.last_day()) {
            current
        } else {
            before
        }
    }
}

/// A deposit into an own account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deposit<A> {
    pub on: Date,
    pub amount_minor: i64,
    pub description: String,
    pub account: A,
}

/// An employer paid by the fortnight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Employer<A> {
    pub name: String,
    /// Text its deposits carry in their description (compared without case).
    pub match_text: String,
    /// The account its deposits arrive in.
    pub account: A,
    /// Below this a deposit is not a fortnight's pay.
    pub min_amount_minor: i64,
    /// What a fortnight is worth today.
    pub amount_minor: i64,
    /// The first fortnight counted.
    pub start: Fortnight,
}

/// Where a fortnight stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Due on or before the account's last data and not paid.
    Owed,
    /// Due after the account's last data: whether it was paid is not known yet.
    NoData,
    Late,
    OnTime,
    /// Paid before it was due.
    Ahead,
}

/// One fortnight of the history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settlement {
    pub fortnight: Fortnight,
    /// The deposit that settled it, as a position in the input.
    pub deposit: Option<usize>,
    pub status: Status,
    /// How late the payment was (zero or below: on time); for an owed fortnight, days it has been due at the last
    /// data; none when nothing is known.
    pub days: Option<i64>,
    /// The fortnight the deposit's description named, if it named one.
    pub label: Option<Fortnight>,
}

/// What an employer owes at a date.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// Due and unpaid, oldest first.
    pub owed: Vec<Fortnight>,
    pub ahead: Vec<Fortnight>,
    pub unknown: Vec<Fortnight>,
    /// Deposits that counted as a fortnight's pay.
    pub paid: usize,
    /// The fortnight in course at `as_of`.
    pub next_due: Fortnight,
    /// From the first fortnight to the last due or paid one.
    pub history: Vec<Settlement>,
    /// The date "owed" is measured at: `as_of`, or the account's last data when earlier.
    pub data_until: Date,
}

/// No deposit counted: most likely the match text, account or minimum is wrong.
#[derive(Debug, PartialEq, Eq)]
pub struct NoDeposits;

/// The deposits that count as this employer's fortnight pay, as positions in `deposits`.
pub fn eligible<A: Eq>(employer: &Employer<A>, deposits: &[Deposit<A>]) -> Vec<usize> {
    let needle = employer.match_text.to_lowercase();
    let first = employer.start.first_day();
    (0..deposits.len())
        .filter(|&i| {
            let d = &deposits[i];
            d.description.to_lowercase().contains(&needle)
                && d.account == employer.account
                && d.amount_minor >= employer.min_amount_minor
                && d.on >= first
        })
        .collect()
}

/// Which fortnight each deposit settles (newest free first; labels win).
pub fn assign<A>(
    deposits: &[Deposit<A>],
    counted: &[usize],
    start: Fortnight,
    labels: &Labels,
) -> BTreeMap<Fortnight, usize> {
    let mut order = counted.to_vec();
    order.sort_by_key(|&i| deposits[i].on);
    let mut covered = BTreeMap::new();
    let mut unlabeled = Vec::new();
    for i in order {
        match labels.parse(&deposits[i].description, deposits[i].on) {
            Some(f) if f >= start && !covered.contains_key(&f) => {
                covered.insert(f, i);
            }
            _ => unlabeled.push(i),
        }
    }
    for i in unlabeled {
        let reference = Fortnight::reference(deposits[i].on);
        let mut target = reference;
        while target >= start && covered.contains_key(&target) {
            target = target.previous();
        }
        if target < start {
            target = reference.next();
            while covered.contains_key(&target) {
                target = target.next();
            }
        }
        covered.insert(target, i);
    }
    covered
}

/// The fortnights from `start` already due at `as_of`.
pub fn due(start: Fortnight, as_of: Date) -> Vec<Fortnight> {
    let mut found = Vec::new();
    let mut fortnight = start;
    while fortnight.due_on() <= as_of {
        found.push(fortnight);
        fortnight = fortnight.next();
    }
    found
}

/// How `fortnight` stands; `paid` is the deposit that settled it, with its position in the input.
fn settle<A>(
    fortnight: Fortnight,
    paid: Option<(usize, &Deposit<A>)>,
    as_of: Date,
    data_until: Date,
    labels: &Labels,
) -> Settlement {
    let deposit = paid.map(|(i, _)| i);
    let Some((_, d)) = paid else {
        let (status, days) = if fortnight.due_on() > data_until {
            (Status::NoData, None)
        } else {
            (Status::Owed, Some((data_until - fortnight.due_on()).whole_days()))
        };
        return Settlement {
            fortnight,
            deposit,
            status,
            days,
            label: None,
        };
    };
    let days = (d.on - fortnight.due_on()).whole_days();
    let status = if fortnight.due_on() > as_of {
        Status::Ahead
    } else if days > 0 {
        Status::Late
    } else {
        Status::OnTime
    };
    Settlement {
        fortnight,
        deposit,
        status,
        days: Some(days),
        label: labels.parse(&d.description, d.on),
    }
}

/// What `employer` owes at `as_of`, from `deposits` (any account; only its own count). `data_until` is the last date
/// with data for its account: "owed" is only said up to there.
pub fn report<A: Eq>(
    employer: &Employer<A>,
    deposits: &[Deposit<A>],
    as_of: Date,
    data_until: Option<Date>,
    labels: &Labels,
) -> Result<Report, NoDeposits> {
    let counted = eligible(employer, deposits);
    if counted.is_empty() {
        return Err(NoDeposits);
    }
    let cutoff = data_until.map_or(as_of, |d| d.min(as_of));
    let covered = assign(deposits, &counted, employer.start, labels);
    let fortnights: BTreeSet<Fortnight> = due(employer.start, as_of)
        .into_iter()
        .chain(covered.keys().copied())
        .collect();
    let history: Vec<Settlement> = fortnights
        .into_iter()
        .map(|f| {
            let paid = covered.get(&f).map(|&i| (i, &deposits[i]));
            settle(f, paid, as_of, cutoff, labels)
        })
        .collect();
    let with = |status: Status| {
        history
            .iter()
            .filter(|s| s.status == status)
            .map(|s| s.fortnight)
            .collect()
    };
    Ok(Report {
        owed: with(Status::Owed),
        ahead: with(Status::Ahead),
        unknown: with(Status::NoData),
        paid: counted.len(),
        next_due: Fortnight::containing(as_of),
        history,
        data_until: cutoff,
    })
}

#[cfg(test)]
mod tests;
