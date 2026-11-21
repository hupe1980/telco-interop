//! Answers built from read requests carry every reference and validate clean.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use wbci::*;

fn read_anfrage(name: &str) -> Anfrage {
    let xml = std::fs::read(format!("{}/tests/fixtures/{name}.xml", env!("CARGO_MANIFEST_DIR"))).unwrap();
    match Payload::from_xml(&xml).unwrap() {
        Payload::AnfrageRequest { anfrage, .. } => anfrage,
        other => panic!("{other:?}"),
    }
}

#[test]
fn ruem_va_from_a_read_request() {
    let Anfrage::Vorabstimmung(va) = read_anfrage("va-kue-orn") else { panic!() };
    let later = common::date("2027-01-22");
    let ruem = RuemVa::zustimmung(&va, later);
    assert_eq!(ruem.kopf.absender, va.ekp.abg);
    assert_eq!(ruem.kopf.vorabstimmungs_id, va.id);
    assert_eq!(ruem.kopf.geschaeftsfall, Geschaeftsfall::VaKueOrn);
    assert_eq!(ruem.positionen[0].position.code, Meldungscode::Nat);
    assert_eq!(Meldung::from(ruem).issues(Some(&va.into())), vec![]);
}

#[test]
fn abbm_and_erlm_for_every_request() {
    for name in [
        "va-kue-mrn",
        "va-kue-orn",
        "va-rrnp",
        "str-auf-ekpauf",
        "str-auf-ekpabg",
        "str-aen-ekpauf-original",
        "str-aen-ekpabg",
        "tvs-va",
    ] {
        let anfrage = read_anfrage(name);
        let code = match anfrage.geschaeftsfall() {
            Geschaeftsfall::StrAuf | Geschaeftsfall::StrAen => Meldungscode::StornoAbgelehnt,
            Geschaeftsfall::TvsVa => Meldungscode::TerminverschiebungAbgelehnt,
            _ => Meldungscode::Kni,
        };
        let mut abbm = Abbm::ablehnung(&anfrage, code);
        if anfrage.geschaeftsfall() == Geschaeftsfall::TvsVa {
            abbm.wechseltermin = Some(common::date("2027-01-15"));
        }
        assert_eq!(abbm.kopf.absender, *anfrage.ekp().of(anfrage.sender().other()), "{name}");
        assert!(Meldung::from(abbm).issues(Some(&anfrage)).is_empty(), "{name}");
        let teq = Teq::for_anfrage(&anfrage, Meldungscode::Ok);
        assert!(teq.is_ok());
        assert_eq!(teq.absender, *anfrage.ekp().of(anfrage.sender().other()));
        let erlm = match &anfrage {
            Anfrage::Storno(s) => Erlm::storno_ok(s),
            Anfrage::Terminverschiebung(t) => Erlm::terminverschiebung_ok(t),
            Anfrage::Vorabstimmung(_) => continue,
        };
        assert!(Meldung::from(erlm).issues(Some(&anfrage)).is_empty(), "{name}");
    }
}

#[test]
fn takeover_and_its_refusal() {
    let ruem = common::ruem_va();
    let akm = AkmTr::uebernahme(&ruem, vec![UebernahmeLeitung::LineId("DEU.EKP2.1754639821".parse().unwrap())], true);
    assert_eq!(akm.kopf.absender, ruem.kopf.ekp.auf);
    assert!(akm.ressourcenuebernahme);
    let va: Anfrage = common::va_kue_mrn().into();
    assert_eq!(Meldung::from(akm.clone()).issues(Some(&va)), vec![]);
    let abbm_tr = AbbmTr::ablehnung(&akm, Meldungscode::LineIdFremd);
    assert_eq!(abbm_tr.kopf.absender, ruem.kopf.ekp.abg);
    assert_eq!(Meldung::from(abbm_tr).issues(Some(&va)), vec![]);
}

#[test]
fn receipt_for_unreadable_input() {
    let err = Payload::from_xml(b"<nonsense/>").unwrap_err();
    let teq = Teq::fehler(common::cc("DEU.EKP2"), common::va_id(), err.teq_code(), None);
    assert_eq!(teq.positionen[0].code, Meldungscode::DatenstrukturUngueltig);
    assert!(!teq.is_ok());
    let xml = Payload::AnfrageResponse { control: common::control("2026-11-23T09:30:00+01:00"), quittung: teq }
        .to_xml()
        .unwrap();
    assert!(xml.contains("<Meldungscode>0995</Meldungscode>"));
}

#[test]
fn teq_for_a_message_goes_back_to_its_sender() {
    let m: Meldung = common::ruem_va().into();
    let teq = Teq::for_meldung(&m, Meldungscode::Ok);
    assert_eq!(teq.absender, common::cc("DEU.EKP1"));
}
