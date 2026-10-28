use time::{Date, Month};

use super::*;

fn day(d: u8) -> Date {
    Date::from_calendar_date(2026, Month::May, d).unwrap()
}

fn leg(id: &str, d: u8, amount_minor: i64, account: &str) -> Leg<String, String> {
    Leg {
        id: id.to_owned(),
        date: day(d),
        amount_minor,
        currency: "USD".to_owned(),
        account: account.to_owned(),
        text: String::new(),
    }
}

fn paired_ids(legs: &[Leg<String, String>]) -> Vec<(String, String, Pass)> {
    let mut ids: Vec<_> = plan(legs, &[])
        .pairs
        .iter()
        .map(|p| (legs[p.outflow].id.clone(), legs[p.inflow].id.clone(), p.pass))
        .collect();
    ids.sort();
    ids
}

#[test]
fn business_days_skip_the_weekend() {
    // 2026-05-01 is a Friday.
    assert_eq!(business_days(day(1), day(4)), 1, "Friday to Monday");
    assert_eq!(business_days(day(2), day(4)), 1, "Saturday to Monday");
    assert_eq!(business_days(day(4), day(1)), 1, "either way round");
    assert_eq!(business_days(day(1), day(5)), 2, "Friday to Tuesday");
    assert_eq!(business_days(day(4), day(4)), 0);
    assert_eq!(calendar_days(day(1), day(4)), 3);
}

#[test]
fn a_different_currency_never_pairs() {
    let mut legs = vec![leg("1", 4, -5000, "checking"), leg("2", 4, 5000, "savings")];
    legs[1].currency = "EUR".to_owned();
    let found = plan(&legs, &[]);
    assert_eq!(found.pairs, []);
    assert_eq!(found.orphans.len(), 2);
}

#[test]
fn the_input_order_does_not_change_the_pairs() {
    let legs = vec![
        leg("1", 4, -5000, "checking"),
        leg("2", 5, 5000, "savings"),
        leg("3", 4, -7000, "checking"),
        leg("4", 4, 7000, "card"),
        leg("5", 6, 7000, "savings"),
        leg("6", 1, -9000, "checking"),
        leg("7", 4, 9000, "card"),
    ];
    let forward = paired_ids(&legs);
    let mut backward = legs.clone();
    backward.reverse();
    assert_eq!(paired_ids(&backward), forward);
    // 3 has two inflows in reach, so it waits; 6 → 7 is Friday to Monday.
    assert_eq!(
        forward,
        [
            ("1".into(), "2".into(), Pass::Unique),
            ("6".into(), "7".into(), Pass::BusinessDays)
        ]
    );
}
