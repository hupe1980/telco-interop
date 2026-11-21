//! Personal data stays out of logs: `Debug` and errors.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use spri::*;

const SECRETS: [&str; 12] = [
    "Erika",
    "Mustermann",
    "Beispiel",
    "1234567",
    "7654321",
    "1234568",
    "9876543",
    "example.de",
    "Hauptstraße",
    "Beispiel GmbH",
    "Vormann",
    "synthetic test document",
];

fn assert_clean(what: &str, text: &str) {
    for secret in SECRETS {
        assert!(!text.contains(secret), "{what} leaks {secret:?}: {text}");
    }
    // Attachment bytes do not appear as a byte list either ("%PDF" = 37, 80, 68, 70).
    assert!(!text.contains("37, 80, 68, 70"), "{what} leaks attachment bytes");
}

// spri-model/SR-002: Debug of every order, message and receipt shows no names, numbers,
// e-mail address, street or attachment content.
#[test]
fn debug_of_orders_and_messages_is_redacted() {
    for (name, payload) in common::all_payloads() {
        assert_clean(name, &format!("{payload:?}"));
        assert_clean(name, &format!("{payload:#?}"));
    }
}

// spri-model/SR-002: an order with end customer, contact person and attachment.
#[test]
fn an_order_with_customer_contact_and_attachment_is_redacted() {
    let auftrag = common::neu();
    let Geschaeftsfall::Neu(b) = &auftrag.geschaeftsfall else { panic!() };
    let text = format!("{auftrag:?}");
    assert_clean("NEU", &text);
    assert!(text.contains(&format!("<{} bytes>", b.anlage.as_ref().unwrap().inhalt.len())), "{text}");
    assert_clean("EST", &format!("{:?}", common::est()));
}

// spri-model/SR-002: construction errors carry no field values.
#[test]
fn value_errors_carry_no_values() {
    let err = Nachname::new(format!("Mustermann{}", "x".repeat(30))).unwrap_err();
    assert_clean("ValueError", &err.to_string());
    assert_clean("ValueError", &format!("{err:?}"));
    let err = Emailadresse::new("erika.mustermann.example.de").unwrap_err();
    assert_clean("ValueError", &err.to_string());
}
