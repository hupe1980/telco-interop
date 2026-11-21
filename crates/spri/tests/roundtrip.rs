//! Every order and message kind round-trips: value → XML → value is equal, and the golden
//! fixtures read and write back byte for byte. Regenerate the fixtures with `SPRI_BLESS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::path::PathBuf;

use spri::*;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(format!("{name}.xml"))
}

#[test]
fn every_order_and_message_kind_has_a_fixture() {
    let payloads = common::all_payloads();
    assert_eq!(common::orders().len(), 8);
    assert_eq!(common::messages().len(), 20);
    let mut typen: Vec<Meldungstyp> = common::messages().iter().map(|(_, m)| m.typ()).collect();
    typen.sort_by_key(|t| t.as_str());
    typen.dedup();
    assert_eq!(typen.len(), 20, "every message type but TEQ");
    assert_eq!(payloads.len(), 30);
}

#[test]
fn golden_fixtures_are_current_and_round_trip() {
    let bless = std::env::var_os("SPRI_BLESS").is_some();
    for (name, payload) in common::all_payloads() {
        let xml = payload.to_xml().unwrap();
        let path = fixture(name);
        if bless {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &xml).unwrap();
        }
        let golden =
            std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing fixture {name}; run with SPRI_BLESS=1"));
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

// spri-model/SR-001: markup characters in text are escaped and come back unchanged.
#[test]
fn markup_in_text_is_escaped() {
    let mut auftrag = common::est();
    let text = Freitext::new(r#"<b>O'Neil</b> & "Bau""#).unwrap();
    if let Geschaeftsfall::Est(e) = &mut auftrag.geschaeftsfall {
        e.stoerungsbeschreibung = text.clone();
    }
    let payload = Payload::AuftragRequest { control: common::c(), auftrag };
    let xml = payload.to_xml().unwrap();
    assert!(xml.contains("&lt;b&gt;O&apos;Neil&lt;/b&gt; &amp; &quot;Bau&quot;"));
    assert_eq!(Payload::from_xml(xml.as_bytes()).unwrap(), payload);
}

#[test]
fn a_wbci_vorabstimmungs_id_goes_into_a_pv_order() {
    let from_wbci: VorabstimmungsId = VorabstimmungsId::allocate(&"DEU.EKP1".parse().unwrap(), 13).unwrap();
    let mut auftrag = common::pv();
    if let Geschaeftsfall::Pv(p) = &mut auftrag.geschaeftsfall {
        p.vorabstimmungs_id = from_wbci.clone();
    }
    assert_eq!(auftrag.geschaeftsfall.code(), GeschaeftsfallCode::Pv);
    let xml = Payload::AuftragRequest { control: common::c(), auftrag }.to_xml().unwrap();
    assert!(xml.contains(r#"<geschaeftsfall xsi:type="order:ProviderwechselType">"#));
    assert!(xml.contains("<vorabstimmungId>DEU.EKP1.V000000013</vorabstimmungId>"));
    let Payload::AuftragRequest { auftrag, .. } = Payload::from_xml(xml.as_bytes()).unwrap() else { panic!() };
    let Geschaeftsfall::Pv(p) = auftrag.geschaeftsfall else { panic!() };
    assert_eq!(p.vorabstimmungs_id, from_wbci);
}

#[test]
fn abm_gives_date_line_and_positions() {
    let xml = std::fs::read_to_string(fixture("meldung-abm")).unwrap();
    let Payload::MeldungRequest { meldung, .. } = Payload::from_xml(xml.as_bytes()).unwrap() else { panic!() };
    let Meldungsinhalt::Abm(abm) = meldung.inhalt else { panic!("not an ABM") };
    assert_eq!(abm.verbindlicher_liefertermin.datum, common::date("2027-01-15"));
    assert_eq!(abm.anschluss.line_id.as_str(), "DEU.NB01.1754639821");
    assert_eq!(abm.positionen.len(), 2);
    assert_eq!(abm.positionen[0].position.position.code, Meldungscode::C0000);
    assert!(abm.positionen[0].ansprechpartner_leistungserbringer.is_some());
}

#[test]
fn utc_timestamps_other_prefixes_and_short_releases_are_read() {
    let xml = std::fs::read_to_string(fixture("teq-auftrag"))
        .unwrap()
        .replace("env:", "e:")
        .replace("xmlns:env", "xmlns:e")
        .replace("+01:00", "Z")
        .replace("<majorRelease>04</majorRelease>", "<majorRelease>4</majorRelease>");
    let Payload::AuftragResponse { control, .. } = Payload::from_xml(xml.as_bytes()).unwrap() else { panic!() };
    assert_eq!(control.zeitstempel.timestamp, common::ts("2026-11-23T09:30:00Z"));
    assert_eq!(control.release, Release::SPRI_4_4);
}

#[test]
fn unknown_codes_are_read_and_kept() {
    let xml = std::fs::read_to_string(fixture("meldung-abbm")).unwrap().replace(">1001<", ">4242<");
    let Payload::MeldungRequest { meldung, .. } = Payload::from_xml(xml.as_bytes()).unwrap() else { panic!() };
    assert_eq!(meldung.inhalt.codes(), [&Meldungscode::Other("4242".into())]);
}

#[test]
fn a_message_without_positions_is_refused_when_writing() {
    let mut abbm = common::abbm();
    abbm.positionen.clear();
    let meldung = common::meldung(GeschaeftsfallCode::Neu, GeschaeftsfallArt::Bereitstellung, abbm);
    let err = Payload::MeldungRequest { control: common::c(), meldung }.to_xml().unwrap_err();
    assert_eq!(err.kind, xml::WriteErrorKind::Missing { element: "position" });
    assert!(err.path.ends_with("meldungstyp/meldungspositionen"), "{}", err.path);
}

#[test]
fn eleven_numbers_are_refused_when_writing() {
    let mut auftrag = common::neu();
    if let Geschaeftsfall::Neu(b) = &mut auftrag.geschaeftsfall {
        let one = OnkzRufnummer { onkz: Onkz::new("30").unwrap(), rufnummer: Rufnummer::new("1").unwrap() };
        b.fttx.wita.rufnummern_portierung.as_mut().unwrap().anschluss = PortierungAnschluss::Rufnummern(vec![one; 11]);
    }
    let err = Payload::AuftragRequest { control: common::c(), auftrag }.to_xml().unwrap_err();
    assert_eq!(err.kind, xml::WriteErrorKind::TooMany { element: "zuPortierendeOnkzRnr", max: 10, actual: 11 });
}
