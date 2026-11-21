//! Deadlines against the worked examples of Anl. 1 V5.4.1 §2.5 and one case per rule.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use wbci::calendar::Closed;
use wbci::deadline::{self, DeadlineKind};
use wbci::{Calendar, Date, Geschaeftsfall, Role, Timestamp};

fn berlin(date: &str, time: &str) -> Timestamp {
    let dt: jiff::civil::DateTime = format!("{date}T{time}").parse().unwrap();
    dt.to_zoned_datetime(wbci::calendar::berlin()).unwrap().timestamp()
}

fn d(s: &str) -> Date {
    s.parse().unwrap()
}

/// Example 3: answer to a VA acknowledged on Monday is due by Thursday 23:59 (X+3 AT).
#[test]
fn example_3_forward_three_working_days() {
    let dl = deadline::va_answer(berlin("2026-11-23", "10:15"), &Calendar::wbci());
    assert_eq!(dl.due, berlin("2026-11-27", "00:00"));
    assert_eq!(dl.party, Role::EkpAbg);
    assert_eq!(dl.to_string().split(" — ").next().unwrap(), "EKPabg: by Do 26.11.2026 23:59");
    assert_eq!(dl.explanation.counted, vec![d("2026-11-24"), d("2026-11-25"), d("2026-11-26")]);
}

/// Example 1: switch date Friday, X−3 AT is Tuesday, so the request is due by Monday 23:59.
#[test]
fn example_1_backward_three_working_days() {
    let dl = deadline::terminverschiebung_latest(d("2026-11-27"), false, &Calendar::wbci());
    assert_eq!(dl.due, berlin("2026-11-24", "00:00"));
    assert!(dl.to_string().starts_with("EKPauf: by Mo 23.11.2026 23:59"), "{dl}");
}

/// Example 2: switch date Monday, X−3 AT is Wednesday, so the request is due by Tuesday 23:59.
#[test]
fn example_2_backward_over_a_weekend() {
    let dl = deadline::terminverschiebung_latest(d("2026-11-30"), false, &Calendar::wbci());
    assert_eq!(dl.due, berlin("2026-11-25", "00:00"));
    assert_eq!(dl.explanation.skipped, vec![(d("2026-11-29"), Closed::Weekend), (d("2026-11-28"), Closed::Weekend)]);
    // The count starts on the switch date itself, not „23:59 the day before".
    assert!(
        dl.explanation.to_string().starts_with("agreed switch date Mo 30.11.2026; backwards"),
        "{}",
        dl.explanation
    );
}

#[test]
fn holidays_are_skipped_and_named() {
    // TEQ on Wed 23.12.2026: Thu 24 counts, 25/26 are holidays, 27 Sunday, Mon 28, Tue 29.
    let dl = deadline::va_answer(berlin("2026-12-23", "16:00"), &Calendar::wbci());
    assert_eq!(dl.due, berlin("2026-12-30", "00:00"));
    let text = dl.explanation.to_string();
    assert!(text.contains("Fr 25.12.2026 (1. Weihnachtstag)"), "{text}");
    assert!(text.contains("Sa 26.12.2026 (weekend)"), "{text}");
    // The literal Mon–Fri reading counts the holiday.
    assert_eq!(
        deadline::va_answer(berlin("2026-12-23", "16:00"), &Calendar::mon_fri()).due,
        berlin("2026-12-29", "00:00")
    );
}

#[test]
fn lead_times_for_a_vorabstimmung() {
    let cal = Calendar::wbci();
    let sent = berlin("2026-11-23", "09:00");
    let kue = deadline::earliest_kundenwunschtermin(Geschaeftsfall::VaKueMrn, sent, &cal).unwrap();
    assert_eq!(kue.date, d("2026-12-21"));
    let rrnp = deadline::earliest_kundenwunschtermin(Geschaeftsfall::VaRrnp, sent, &cal).unwrap();
    assert_eq!(rrnp.date, d("2026-12-02"));
    assert!(deadline::earliest_kundenwunschtermin(Geschaeftsfall::TvsVa, sent, &cal).is_none());
    // 15 AT from Mon 23.11. is Mon 14.12.; a later request wins.
    assert_eq!(deadline::earliest_wechseltermin(sent, d("2026-12-01"), &cal).date, d("2026-12-14"));
    assert_eq!(deadline::earliest_wechseltermin(sent, d("2027-01-15"), &cal).date, d("2027-01-15"));
}

#[test]
fn answer_within_24_hours_on_working_days() {
    let cal = Calendar::wbci();
    // Friday 15:00 → Monday 15:00.
    let dl = deadline::answer_24h(berlin("2026-11-27", "15:00"), Role::EkpAbg, &cal);
    assert_eq!(dl.due, berlin("2026-11-30", "15:00"));
    assert_eq!(dl.kind, DeadlineKind::Answer24h);
    // Saturday counts from Monday 0:00.
    assert_eq!(
        deadline::answer_24h(berlin("2026-11-28", "11:00"), Role::EkpAuf, &cal).due,
        berlin("2026-12-01", "00:00")
    );
    // Over Easter 2027: Thursday 25.03. 12:00 → Tuesday 30.03. 12:00 (Good Friday, weekend, Easter Monday).
    assert_eq!(
        deadline::answer_24h(berlin("2027-03-25", "12:00"), Role::EkpAbg, &cal).due,
        berlin("2027-03-30", "12:00")
    );
}

#[test]
fn twenty_four_hour_lead_for_a_shared_network_operator() {
    // Switch on Monday: 24 working hours before Monday 0:00 is Friday 0:00.
    let dl = deadline::terminverschiebung_latest(d("2026-11-30"), true, &Calendar::wbci());
    assert_eq!(dl.due, berlin("2026-11-27", "00:00"));
}

#[test]
fn every_lead_rule() {
    let cal = Calendar::wbci();
    let x = d("2027-01-15"); // Friday
    assert_eq!(deadline::storno_aufhebung_latest(x, Role::EkpAbg, &cal).due, berlin("2027-01-12", "00:00"));
    assert_eq!(deadline::storno_aenderung_latest(x, Role::EkpAuf, &cal).due, berlin("2027-01-05", "00:00"));
    assert_eq!(deadline::akm_tr_reminder(x, &cal).due, berlin("2026-12-31", "00:00"));
    assert_eq!(deadline::akm_tr_escalation(x, &cal).due, berlin("2027-01-11", "00:00"));
    assert_eq!(deadline::new_vorabstimmung(x, &cal).due, berlin("2027-01-06", "00:00"));
}

#[test]
fn reminder_skips_new_year() {
    // 10 AT before Fri 15.01.2027: 14,13,12,11,8,7,6,5,4 and — Fri 01.01. is Neujahr — Thu 31.12.
    let dl = deadline::akm_tr_reminder(d("2027-01-15"), &Calendar::wbci());
    assert_eq!(dl.explanation.counted.last(), Some(&d("2026-12-31")));
    assert!(dl.explanation.skipped.contains(&(d("2027-01-01"), Closed::Holiday("Neujahr"))));
}

#[test]
fn teq_escalation_after_two_hours() {
    let sent = berlin("2026-11-23", "23:30");
    let dl = deadline::teq_escalation(sent, Role::EkpAuf);
    assert_eq!(dl.due, berlin("2026-11-24", "01:30"));
    assert!(!dl.is_overdue(berlin("2026-11-24", "01:29")));
    assert!(dl.is_overdue(berlin("2026-11-24", "01:30")));
}

#[test]
fn later_porting_window() {
    let dl = deadline::later_porting(d("2026-11-02"), &Calendar::wbci());
    assert_eq!(dl.explanation.counted.len(), 65);
    assert!(dl.explanation.skipped.iter().any(|(_, why)| matches!(why, Closed::Holiday(_))));
}

#[test]
fn daylight_saving_change_is_handled() {
    // 24 working hours from Friday 27.03.2026 12:00 cross the switch to summer time on Sunday.
    let dl = deadline::answer_24h(berlin("2026-03-27", "12:00"), Role::EkpAbg, &Calendar::wbci());
    assert_eq!(dl.due, berlin("2026-03-30", "12:00"));
}
