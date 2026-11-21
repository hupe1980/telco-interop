//! Personal data stays out of logs: `Debug`, errors and issues (wbci-switch/SR-005).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use wbci::*;

const SECRETS: [&str; 7] = ["Erika", "Mustermann", "Hauptstraße", "1234567", "Beispiel GmbH", "Widerruf", "Umzug"];

fn assert_clean(what: &str, text: &str) {
    for secret in SECRETS {
        assert!(!text.contains(secret), "{what} leaks {secret:?}: {text}");
    }
}

// wbci-switch/SR-005: Debug of whole requests, messages and payloads shows no personal data.
#[test]
fn debug_of_requests_and_messages_is_redacted() {
    for (name, payload) in common::all_payloads() {
        assert_clean(name, &format!("{payload:?}"));
        assert_clean(name, &format!("{payload:#?}"));
    }
}

// wbci-switch/SR-005: so is the Debug of a process and its history.
#[test]
fn debug_of_a_process_is_redacted() {
    let p =
        Process::new(Role::EkpAuf, common::va_kue_mrn(), Direction::Sent, common::ts("2026-11-23T08:00:00Z")).unwrap();
    assert_clean("process", &format!("{p:?}"));
}

// wbci-switch/SR-005: construction errors and validation issues carry no field values.
#[test]
fn errors_and_issues_carry_no_values() {
    let err = Nachname::new(format!("Mustermann{}", "x".repeat(30))).unwrap_err();
    assert_clean("ValueError", &err.to_string());
    assert_clean("ValueError", &format!("{err:?}"));
    let mut va = common::va_kue_mrn();
    va.ekp.abg = va.ekp.auf.clone();
    va.kundenwunschtermin = common::date("2026-11-24");
    let anfrage = Anfrage::from(va);
    let issues = anfrage.issues_at(common::ts("2026-11-23T08:00:00Z"), &Calendar::wbci());
    assert_ne!(issues, vec![]);
    for issue in issues {
        assert_clean("issue", &issue.to_string());
    }
}
