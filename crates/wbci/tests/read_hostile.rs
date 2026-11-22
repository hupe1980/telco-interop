//! Untrusted input: every refusal names a reason and maps to the receipt code to answer with.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use wbci::xml::{Limits, ReadErrorKind};
use wbci::{Meldungscode, Payload};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/fixtures/{name}.xml", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn refused(xml: &str) -> wbci::xml::ReadError {
    Payload::from_xml(xml.as_bytes()).expect_err("must be refused")
}

// wbci-switch/SR-001: a DTD is refused before any entity can be declared or expanded.
#[test]
fn doctype_with_entities_is_refused() {
    let bomb = format!(
        "<?xml version=\"1.0\"?>\n<!DOCTYPE lol [<!ENTITY lol \"lol\"><!ENTITY lol2 \"&lol;&lol;&lol;\">]>\n{}",
        fixture("teq-anfrage").replace("4711", "&lol2;")
    );
    let err = refused(&bomb);
    assert_eq!(err.kind, ReadErrorKind::Doctype);
    assert_eq!(err.teq_code(), Meldungscode::DatenstrukturUngueltig);
}

// wbci-switch/SR-001: an external entity cannot be declared either.
#[test]
fn external_entity_is_refused() {
    let xxe = format!("<!DOCTYPE x [<!ENTITY xxe SYSTEM \"file:///etc/passwd\">]>{}", fixture("teq-anfrage"));
    assert_eq!(refused(&xxe).kind, ReadErrorKind::Doctype);
}

// wbci-switch/SR-001: only the five predefined entities resolve.
#[test]
fn undeclared_entity_is_refused() {
    let err = refused(&fixture("teq-anfrage").replace("4711", "&nbsp;4711"));
    assert_eq!(err.kind, ReadErrorKind::Entity);
    let ok = fixture("teq-anfrage").replace("4711", "&#52;711");
    assert!(Payload::from_xml(ok.as_bytes()).is_ok());
}

// wbci-switch/SR-002: oversized input is refused before parsing.
#[test]
fn oversized_input_is_refused() {
    let limits = Limits { max_bytes: 1024, ..Limits::default() };
    let err = Payload::from_xml_with(fixture("va-kue-mrn").as_bytes(), &limits).unwrap_err();
    assert!(matches!(err.kind, ReadErrorKind::TooLarge { limit: 1024, .. }));
}

// wbci-switch/SR-002: nesting depth is bounded.
#[test]
fn deep_nesting_is_refused() {
    let deep = format!("{}{}", "<a>".repeat(40), "</a>".repeat(40));
    assert_eq!(refused(&deep).kind, ReadErrorKind::TooDeep { limit: 32 });
}

// wbci-switch/SR-002: repetitions beyond the schema maximum are refused (99 positions).
#[test]
fn hundred_positions_are_refused() {
    let xml = fixture("abbm-tr");
    let start = xml.find("    <Position").unwrap();
    let end = xml.find("</Position>\n").unwrap() + "</Position>\n".len();
    let position = &xml[start..end];
    let many = xml.replacen(position, &position.repeat(100), 1);
    let err = refused(&many);
    assert_eq!(err.kind, ReadErrorKind::TooMany { element: "Position", max: 99 });
    assert!(err.path.ends_with("Meldung"));
}

// wbci-switch/SR-002: text length is bounded.
#[test]
fn long_text_is_refused() {
    let err = refused(&fixture("teq-anfrage").replace("4711", &"9".repeat(5000)));
    assert_eq!(err.kind, ReadErrorKind::TextTooLong { limit: 4096 });
}

#[test]
fn eleven_numbers_are_refused() {
    let xml = fixture("va-kue-mrn");
    let start = xml.find("          <zuPortierendeOnkzRnr>").unwrap();
    let end = xml.find("</zuPortierendeOnkzRnr>\n").unwrap() + "</zuPortierendeOnkzRnr>\n".len();
    let one = &xml[start..end];
    let err = refused(&xml.replacen(one, &one.repeat(11), 1));
    assert_eq!(err.kind, ReadErrorKind::TooMany { element: "zuPortierendeOnkzRnr", max: 10 });
}

#[test]
fn wrong_major_release_answers_0993() {
    let err =
        refused(&fixture("va-kue-mrn").replace("<majorRelease>02</majorRelease>", "<majorRelease>03</majorRelease>"));
    assert_eq!(err.kind, ReadErrorKind::Version("03".into()));
    assert_eq!(err.teq_code(), Meldungscode::VersionFehlerhaft);
}

#[test]
fn newer_minor_release_is_accepted() {
    let xml = fixture("va-kue-mrn").replace("<minorRelease>00</minorRelease>", "<minorRelease>07</minorRelease>");
    let payload = Payload::from_xml(xml.as_bytes()).unwrap();
    assert_eq!(payload.control().release.minor, 7);
}

#[test]
fn timestamp_without_offset_is_refused() {
    let err = refused(&fixture("teq-anfrage").replace("2026-11-23T09:30:00+01:00", "2026-11-23T09:30:00"));
    assert!(matches!(err.kind, ReadErrorKind::Invalid(_)), "{err}");
    assert!(err.path.ends_with("Control/zeitstempel"));
}

#[test]
fn structural_errors_name_the_path() {
    let wrong_ns = fixture("va-kue-mrn").replace("http://www.vatm.de/wbci/v1/request", "urn:other");
    assert!(matches!(refused(&wrong_ns).kind, ReadErrorKind::XsiType(_)));

    let unknown_type = fixture("va-kue-mrn").replace("KuendigungMitRNPGeschaeftsfallType", "Fantasie");
    assert!(matches!(refused(&unknown_type).kind, ReadErrorKind::XsiType(_)));

    let missing = fixture("va-kue-mrn").replace("<Kundenwunschtermin>2027-01-15</Kundenwunschtermin>", "");
    let err = refused(&missing);
    assert_eq!(err.kind, ReadErrorKind::Missing("Kundenwunschtermin"));
    assert_eq!(err.path, "annehmenAnfrageRequest/Anfrage");

    let bad_value =
        fixture("va-kue-mrn").replace("<postleitzahl>10115</postleitzahl>", "<postleitzahl>1011</postleitzahl>");
    let err = refused(&bad_value);
    assert!(matches!(err.kind, ReadErrorKind::Value(_)));
    assert_eq!(err.path, "annehmenAnfrageRequest/Anfrage/Standort/postleitzahl");
    assert_eq!(
        err.to_string(),
        "annehmenAnfrageRequest/Anfrage/Standort/postleitzahl: Postleitzahl: length must be 5, is 4"
    );

    let swapped = fixture("va-kue-mrn").replace("<Kundenwunschtermin>", "<X>").replace("</Kundenwunschtermin>", "</X>");
    assert!(matches!(refused(&swapped).kind, ReadErrorKind::Missing("Kundenwunschtermin")));
}

#[test]
fn request_sent_by_the_wrong_side_is_refused() {
    let xml =
        fixture("va-kue-mrn").replacen("<CarrierCode>DEU.EKP1</CarrierCode>", "<CarrierCode>DEU.EKP2</CarrierCode>", 1);
    let err = refused(&xml);
    assert_eq!(err.kind, ReadErrorKind::Absender("EKPauf"));
    assert_eq!(err.teq_code(), Meldungscode::DatenstrukturUngueltig);
}

#[test]
fn garbage_is_refused_without_panic() {
    for input in ["", "<", "not xml", "<a></b>", "<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><a/>", "\u{FEFF}<a/>"]
    {
        assert!(Payload::from_xml(input.as_bytes()).is_err(), "{input:?}");
    }
    assert!(Payload::from_xml(&[0xff, 0xfe, 0x00]).is_err());
}

#[test]
fn errors_never_echo_personal_data() {
    // wbci-switch/SR-005
    let err = refused(
        &fixture("va-kue-mrn").replace("<nachname>Mustermann</nachname>", "<nachname>Muster\u{7}mann</nachname>"),
    );
    let text = err.to_string();
    assert!(!text.contains("Muster"), "{text}");
}

#[test]
fn a_malformed_document_names_the_problem_once() {
    let err = wbci::Payload::from_xml(b"<a><b></a>").unwrap_err();
    assert_eq!(err.to_string().matches("malformed XML").count(), 1, "{err}");
}
