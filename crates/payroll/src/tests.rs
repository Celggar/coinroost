use time::{Date, Month};

use super::*;

fn day(year: i32, month: Month, d: u8) -> Date {
    Date::from_calendar_date(year, month, d).unwrap()
}

#[test]
fn the_builtin_words_read_a_bank_label() {
    let labels = Labels::builtin("es-PA").expect("the es-PA words load");
    let paid = day(2025, Month::October, 2);
    assert_eq!(
        labels.parse("PAGO II Qna. de Septiembre 2025", paid),
        Some(Fortnight::new(2025, 9, 2))
    );
    assert_eq!(
        labels.parse("XIII MES AGOSTO", paid),
        None,
        "the x and the i are one word"
    );
}

#[test]
fn locale_words_must_be_written_as_they_are_matched() {
    let words = "joiners = [\"de\"]\nyear_joiners = [\"de\"]\n[halves]\ni = 1\n[months]\n";
    assert!(Labels::from_toml(&format!("{words}enero = 1\n")).is_ok());
    assert!(
        Labels::from_toml(&format!("{words}\"Enero\" = 1\n")).is_err(),
        "upper case never matches"
    );
    assert!(
        Labels::from_toml(&format!("{words}\"marzó\" = 3\n")).is_err(),
        "accents are dropped first"
    );
    assert!(Labels::from_toml(&format!("{words}enero = 13\n")).is_err());
}
