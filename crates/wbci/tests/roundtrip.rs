//! Every request and message kind round-trips: value → XML → value is equal, and the golden
//! fixtures read and write back byte for byte. Regenerate the fixtures with `WBCI_BLESS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::path::PathBuf;

use wbci::*;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(format!("{name}.xml"))
}

#[test]
fn golden_fixtures_are_current_and_round_trip() {
    let bless = std::env::var_os("WBCI_BLESS").is_some();
    for (name, payload) in common::all_payloads() {
        let xml = payload.to_xml().unwrap();
        let path = fixture(name);
        if bless {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &xml).unwrap();
        }
        let golden =
            std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing fixture {name}; run with WBCI_BLESS=1"));
        assert_eq!(xml, golden, "{name}: writer output differs from the fixture");
        let read = Payload::from_xml(golden.as_bytes()).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(read, payload, "{name}: read value differs");
        assert_eq!(read.to_xml().unwrap(), golden, "{name}: not byte-stable");
    }
}

#[test]
fn soap_envelope_round_trips() {
    for (name, payload) in common::all_payloads() {
        let soap = payload.to_soap().unwrap();
        assert!(soap.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert_eq!(Payload::from_xml(soap.as_bytes()).unwrap(), payload, "{name}");
    }
}

// wbci-switch/SR-004: markup characters in text are escaped and come back unchanged.
#[test]
fn markup_in_text_is_escaped() {
    let name = Nachname::new(r#"O'Neil <&> "Bau""#).unwrap();
    let mut va = common::va_kue_mrn();
    va.endkunde = Kunde::person(Anrede::Herr, None, name.clone());
    let payload = Payload::AnfrageRequest { control: common::control("2026-11-23T09:30:00Z"), anfrage: va.into() };
    let xml = payload.to_xml().unwrap();
    assert!(xml.contains("<nachname>O&apos;Neil &lt;&amp;&gt; &quot;Bau&quot;</nachname>"));
    let Payload::AnfrageRequest { anfrage: Anfrage::Vorabstimmung(back), .. } =
        Payload::from_xml(xml.as_bytes()).unwrap()
    else {
        panic!("wrong payload")
    };
    let Kunde::Person(p) = back.endkunde else { panic!("wrong kunde") };
    assert_eq!(p.nachname, name);
}

#[test]
fn line_breaks_in_free_text_survive() {
    let payload = &common::all_payloads()[4].1;
    let xml = payload.to_xml().unwrap();
    assert!(xml.contains("EKPauf&#13;\nNachweis"));
    assert_eq!(&Payload::from_xml(xml.as_bytes()).unwrap(), payload);
}

#[test]
fn utc_timestamps_and_other_prefixes_are_read() {
    let xml = std::fs::read_to_string(fixture("teq-anfrage"))
        .unwrap()
        .replace("env:", "e:")
        .replace("xmlns:env", "xmlns:e")
        .replace("+01:00", "Z");
    let Payload::AnfrageResponse { control, .. } = Payload::from_xml(xml.as_bytes()).unwrap() else { panic!() };
    assert_eq!(control.zeitstempel.timestamp, common::ts("2026-11-23T09:30:00Z"));
}

#[test]
fn too_many_positions_are_refused_when_writing() {
    let mut abbm = common::abbm();
    abbm.positionen = (0..100).map(|_| Position::new(Meldungscode::Sonstiges).into()).collect();
    let err = Payload::MeldungRequest { control: common::control("2026-11-23T09:30:00Z"), meldung: abbm.into() }
        .to_xml()
        .unwrap_err();
    assert_eq!(err.kind, xml::WriteErrorKind::TooMany { element: "Position", max: 99, actual: 100 });
    assert!(err.path.ends_with("Meldung"), "{}", err.path);
}
