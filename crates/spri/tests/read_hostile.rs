//! Untrusted input: every refusal names a reason and maps to the receipt code to answer with.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use spri::xml::{Limits, MAX_ANLAGE_BYTES, ReadErrorKind, default_limits};
use spri::{Geschaeftsfall, Meldungscode, Payload};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/fixtures/{name}.xml", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn refused(xml: &str) -> spri::xml::ReadError {
    Payload::from_xml(xml.as_bytes()).expect_err("must be refused")
}

// spri-model/SR-001: a DTD is refused before any entity can be declared or expanded.
#[test]
fn doctype_with_entities_is_refused() {
    let bomb = format!(
        "<?xml version=\"1.0\"?>\n<!DOCTYPE lol [<!ENTITY lol \"lol\"><!ENTITY lol2 \"&lol;&lol;&lol;\">]>\n{}",
        fixture("meldung-abbm").replace("4711", "&lol2;")
    );
    let err = refused(&bomb);
    assert_eq!(err.kind, ReadErrorKind::Doctype);
    assert_eq!(err.teq_code(), Meldungscode::DATENSTRUKTUR_UNZULAESSIG);
}

// spri-model/SR-001: an external entity cannot be declared either, inside SOAP or not.
#[test]
fn external_entity_is_refused() {
    let soap = common::all_payloads()[0].1.to_soap().unwrap();
    let xxe = soap.replacen(
        "<soapenv:Envelope",
        "<!DOCTYPE x [<!ENTITY xxe SYSTEM \"file:///etc/passwd\">]>\n<soapenv:Envelope",
        1,
    );
    assert_eq!(refused(&xxe).kind, ReadErrorKind::Doctype);
}

// spri-model/SR-001: only the five predefined entities resolve.
#[test]
fn undeclared_entity_is_refused() {
    let err = refused(&fixture("teq-auftrag").replace("4711</auftraggebernummer>", "&nbsp;4711</auftraggebernummer>"));
    assert_eq!(err.kind, ReadErrorKind::Entity);
    let ok = fixture("teq-auftrag").replace("4711</auftraggebernummer>", "&#52;711</auftraggebernummer>");
    assert!(Payload::from_xml(ok.as_bytes()).is_ok());
}

// spri-model/SR-001: oversized input is refused before parsing.
#[test]
fn oversized_input_is_refused() {
    let limits = Limits { max_bytes: 1024, ..default_limits() };
    let err = Payload::from_xml_with(fixture("auftrag-neu").as_bytes(), &limits).unwrap_err();
    assert!(matches!(err.kind, ReadErrorKind::TooLarge { limit: 1024, .. }));
    assert_eq!(err.teq_code(), Meldungscode::DATENSTRUKTUR_UNZULAESSIG);
}

// spri-model/SR-001: nesting depth is bounded.
#[test]
fn deep_nesting_is_refused() {
    let deep = format!("{}{}", "<a>".repeat(40), "</a>".repeat(40));
    assert_eq!(refused(&deep).kind, ReadErrorKind::TooDeep { limit: 32 });
}

// spri-model/SR-001: the number of elements is bounded, so unbounded lists are too.
#[test]
fn too_many_elements_are_refused() {
    let xml = fixture("meldung-entm");
    let one = "        <position>\n          <meldungscode>0010</meldungscode>\n          <meldungstext>Auftrag ausgeführt.</meldungstext>\n        </position>\n";
    assert!(xml.contains(one));
    let err = refused(&xml.replacen(one, &one.repeat(4000), 1));
    assert_eq!(err.kind, ReadErrorKind::TooManyElements { limit: 10_000 });
}

// spri-model/SR-001: repetitions beyond the schema maximum are refused (10 numbers).
#[test]
fn eleven_numbers_are_refused() {
    let xml = fixture("auftrag-neu");
    let start = xml.find("              <zuPortierendeOnkzRnr>").unwrap();
    let end = xml.find("</zuPortierendeOnkzRnr>\n").unwrap() + "</zuPortierendeOnkzRnr>\n".len();
    let one = &xml[start..end];
    let err = refused(&xml.replacen(one, &one.repeat(11), 1));
    assert_eq!(err.kind, ReadErrorKind::TooMany { element: "zuPortierendeOnkzRnr", max: 10 });
    assert!(err.path.ends_with("einzelanschluss/rufnummernliste"), "{}", err.path);
}

// spri-model/SR-001: text length is bounded.
#[test]
fn long_text_is_refused() {
    let err = refused(&fixture("teq-auftrag").replace("A-2026-0001", &"9".repeat(5000)));
    assert_eq!(err.kind, ReadErrorKind::TextTooLong { limit: 4096 });
}

fn with_attachment(bytes: usize) -> String {
    let mut a = common::neu();
    if let Geschaeftsfall::Neu(b) = &mut a.geschaeftsfall {
        b.anlage.as_mut().unwrap().inhalt = vec![0x5a; bytes];
    }
    Payload::AuftragRequest { control: common::c(), auftrag: a }.to_xml().unwrap()
}

// spri-model/SR-001: an attachment of the maximum size is read; one byte more is refused.
#[test]
fn attachment_size_is_bounded() {
    let ok = with_attachment(MAX_ANLAGE_BYTES);
    let Payload::AuftragRequest { auftrag, .. } = Payload::from_xml(ok.as_bytes()).unwrap() else { panic!() };
    let Geschaeftsfall::Neu(b) = auftrag.geschaeftsfall else { panic!() };
    assert_eq!(b.anlage.unwrap().inhalt.len(), MAX_ANLAGE_BYTES);

    let err = refused(&with_attachment(MAX_ANLAGE_BYTES + 1));
    assert_eq!(err.kind, ReadErrorKind::AttachmentTooLarge { limit: MAX_ANLAGE_BYTES });
    assert!(err.path.ends_with("anlage/inhalt"), "{}", err.path);

    // A tighter limit, and base64 text far beyond any limit, are refused while parsing.
    let tight = Limits { max_attachment: 1000, ..default_limits() };
    let err = Payload::from_xml_with(with_attachment(10_000).as_bytes(), &tight).unwrap_err();
    assert_eq!(err.kind, ReadErrorKind::AttachmentTooLarge { limit: 1000 });
}

#[test]
fn invalid_base64_is_refused() {
    let xml = fixture("auftrag-neu").replace("JVBERi0x", "JVBER*0x");
    let err = refused(&xml);
    assert_eq!(err.kind, ReadErrorKind::Invalid("base64"));
}

#[test]
fn wrong_major_release_answers_0993() {
    let err =
        refused(&fixture("auftrag-pv").replace("<majorRelease>04</majorRelease>", "<majorRelease>03</majorRelease>"));
    assert_eq!(err.kind, ReadErrorKind::Version("03".into()));
    assert_eq!(err.teq_code(), Meldungscode::VERSION_FEHLERHAFT);
}

#[test]
fn newer_minor_release_is_accepted() {
    let xml = fixture("auftrag-pv").replace("<minorRelease>04</minorRelease>", "<minorRelease>07</minorRelease>");
    assert_eq!(Payload::from_xml(xml.as_bytes()).unwrap().control().release.minor, 7);
}

#[test]
fn timestamp_without_offset_is_refused() {
    let err = refused(&fixture("teq-auftrag").replace("2026-11-23T09:30:00+01:00", "2026-11-23T09:30:00"));
    assert!(matches!(err.kind, ReadErrorKind::Invalid(_)), "{err}");
    assert!(err.path.ends_with("control/zeitstempel"));
}

#[test]
fn structural_errors_name_the_path() {
    let wrong_ns = fixture("auftrag-pv")
        .replace("xmlns:order=\"http://spri.telekom.de/oss/v4/order\"", "xmlns:order=\"urn:other\"");
    assert!(matches!(refused(&wrong_ns).kind, ReadErrorKind::XsiType(_)));

    let unknown_type = fixture("meldung-abm").replace("MeldungstypABMType", "MeldungstypTEQType");
    assert!(matches!(refused(&unknown_type).kind, ReadErrorKind::XsiType(_)));

    let wrong_product = fixture("auftrag-pv").replace("fttx:FTTXProviderwechselType", "fttx:FTTXBereitstellungType");
    assert!(matches!(refused(&wrong_product).kind, ReadErrorKind::XsiType(_)));

    let missing = fixture("auftrag-pv").replace("      <vorabstimmungId>DEU.EKP1.V000000013</vorabstimmungId>\n", "");
    let err = refused(&missing);
    assert_eq!(err.kind, ReadErrorKind::Missing("vorabstimmungId"));
    assert_eq!(err.path, "annehmenAuftragRequest/auftrag/geschaeftsfall");

    let bad_value = fixture("auftrag-pv").replace("<hausnummer>12</hausnummer>", "<hausnummer>12a</hausnummer>");
    let err = refused(&bad_value);
    assert!(matches!(err.kind, ReadErrorKind::Value(_)));
    assert_eq!(
        err.to_string(),
        "annehmenAuftragRequest/auftrag/geschaeftsfall/auftragsposition/geschaeftsfallProdukt/standortA/strasse/hausnummer: Hausnummer: must match \\d{1,4}"
    );

    let extra = fixture("meldung-kdle").replace("</meldungsattribute>", "<x/></meldungsattribute>");
    assert_eq!(refused(&extra).kind, ReadErrorKind::Unexpected("x".into()));

    let bad_enum = fixture("auftrag-lae").replace("<aktionscode>A</aktionscode>", "<aktionscode>Q</aktionscode>");
    assert!(matches!(refused(&bad_enum).kind, ReadErrorKind::Invalid(_)));
}

#[test]
fn garbage_is_refused_without_panic() {
    for input in ["", "<", "not xml", "<a></b>", "<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><a/>", "\u{FEFF}<a/>"]
    {
        assert!(Payload::from_xml(input.as_bytes()).is_err(), "{input:?}");
    }
    assert!(Payload::from_xml(&[0xff, 0xfe, 0x00]).is_err());
    // A WBCI payload is not an S/PRI payload.
    assert!(matches!(
        refused("<e:annehmenMeldungRequest xmlns:e=\"http://www.vatm.de/wbci/v1/envelope\"/>").kind,
        ReadErrorKind::Unexpected(_)
    ));
}

#[test]
fn every_truncation_of_every_fixture_is_refused_without_panic() {
    for (name, payload) in common::all_payloads() {
        let xml = payload.to_xml().unwrap();
        for cut in (0..xml.len()).step_by(97) {
            if xml.is_char_boundary(cut) {
                assert!(Payload::from_xml(&xml.as_bytes()[..cut]).is_err(), "{name} cut at {cut}");
            }
        }
    }
}

// spri-model/SR-002: errors never repeat values.
#[test]
fn errors_never_echo_personal_data() {
    let err = refused(
        &fixture("auftrag-est").replace("<nachname>Mustermann</nachname>", "<nachname>Muster\u{7}mann</nachname>"),
    );
    let text = err.to_string();
    assert!(!text.contains("Muster"), "{text}");
}
