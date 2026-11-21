//! Synthetic sample values shared by the integration tests. Every name, number and address
//! here is invented.
#![allow(
    dead_code,
    unreachable_pub,
    clippy::unwrap_used,
    clippy::missing_panics_doc,
    clippy::panic,
    clippy::indexing_slicing
)]

use wbci::*;

pub fn cc(s: &str) -> CarrierCode {
    s.parse().unwrap()
}

pub fn ekp() -> Ekp {
    Ekp { auf: cc("DEU.EKP1"), abg: cc("DEU.EKP2") }
}

pub fn va_id() -> VorabstimmungsId {
    VorabstimmungsId::allocate(&cc("DEU.EKP1"), 13).unwrap()
}

pub fn ts(s: &str) -> Timestamp {
    s.parse().unwrap()
}

pub fn control(at: &str) -> Control {
    Control::new(
        ts(at),
        SignaturId {
            issuer: SignaturText::new("CN=Test CA, O=Example").unwrap(),
            serial: SignaturText::new("4711").unwrap(),
        },
    )
}

pub fn date(s: &str) -> Date {
    s.parse().unwrap()
}

pub fn kunde() -> Kunde {
    Kunde::person(Anrede::Frau, Some(Vorname::new("Erika").unwrap()), Nachname::new("Mustermann").unwrap())
}

pub fn standort() -> Standort {
    Standort {
        strassenname: Strassenname::new("Hauptstraße").unwrap(),
        hausnummer: Hausnummer::new("12").unwrap(),
        hausnummern_zusatz: Some(HausnummernZusatz::new("a").unwrap()),
        postleitzahl: Postleitzahl::new("10115").unwrap(),
        ort: Ortsname::new("Berlin").unwrap(),
    }
}

pub fn nummer(onkz: &str, nr: &str) -> OnkzRufnummer {
    OnkzRufnummer { onkz: Onkz::new(onkz).unwrap(), rufnummer: Rufnummer::new(nr).unwrap() }
}

pub fn einzel() -> Rufnummernportierung {
    Rufnummernportierung {
        anschluss: Anschluss::Einzel { rufnummern: vec![nummer("30", "1234567")], alle_rufnummern: true },
        zeitfenster: Portierungszeitfenster::Zf2,
    }
}

pub fn anlage() -> Rufnummernportierung {
    Rufnummernportierung {
        anschluss: Anschluss::Anlage {
            kopf: Anlagenkopf {
                onkz: Onkz::new("228").unwrap(),
                durchwahl: Durchwahlnummer::new("4711").unwrap(),
                abfragestelle: Abfragestelle::new("0").unwrap(),
            },
            bloecke: vec![Rufnummernblock {
                von: RufnummernBlock::new("00").unwrap(),
                bis: RufnummernBlock::new("99").unwrap(),
            }],
        },
        zeitfenster: Portierungszeitfenster::Zf1,
    }
}

pub fn va_kue_mrn() -> Vorabstimmung {
    Vorabstimmung {
        ekp: ekp(),
        id: va_id(),
        kundenwunschtermin: date("2027-01-15"),
        endkunde: kunde(),
        weitere_anschlussinhaber: vec![Kunde::person(Anrede::Herr, None, Nachname::new("Mustermann").unwrap())],
        projekt: Some(ProjektId { projektkenner: Projektkenner::new("P-2026-11").unwrap(), kopplungskenner: None }),
        fall: VorabstimmungsFall::KueMrn { standort: standort(), rufnummernportierung: einzel() },
    }
}

pub fn va_kue_orn() -> Vorabstimmung {
    Vorabstimmung {
        id: VorabstimmungsId::allocate(&cc("DEU.EKP1"), 14).unwrap(),
        endkunde: Kunde::firma(Firmenname::new("Beispiel GmbH").unwrap()),
        weitere_anschlussinhaber: vec![],
        projekt: None,
        fall: VorabstimmungsFall::KueOrn {
            standort: standort(),
            anschlussidentifikation: Some(nummer("30", "7654321")),
        },
        ..va_kue_mrn()
    }
}

pub fn va_rrnp() -> Vorabstimmung {
    Vorabstimmung {
        id: VorabstimmungsId::allocate(&cc("DEU.EKP1"), 15).unwrap(),
        weitere_anschlussinhaber: vec![],
        projekt: None,
        kundenwunschtermin: date("2026-12-10"),
        fall: VorabstimmungsFall::Rrnp {
            rufnummernportierung: anlage(),
            pki_auf: Portierungskennung::new("D001").unwrap(),
        },
        ..va_kue_mrn()
    }
}

pub fn str_auf_auf() -> Storno {
    Storno {
        ekp: ekp(),
        storno_id: StornoId::allocate(&cc("DEU.EKP1"), 1).unwrap(),
        bezug: Bezug::Id(va_id()),
        art: StornoArt::AufhebungAuf,
    }
}

pub fn str_auf_abg() -> Storno {
    Storno {
        storno_id: StornoId::allocate(&cc("DEU.EKP2"), 2).unwrap(),
        art: StornoArt::AufhebungAbg {
            name: Some(kunde()),
            standort: Some(standort()),
            stornogrund: Freitext::new("Widerruf des Vertrags mit dem EKPauf\r\nNachweis liegt vor").unwrap(),
        },
        ..str_auf_auf()
    }
}

pub fn str_aen_auf() -> Storno {
    Storno {
        storno_id: StornoId::allocate(&cc("DEU.EKP1"), 3).unwrap(),
        bezug: Bezug::Originalanfrage(Box::new(va_kue_mrn())),
        art: StornoArt::AenderungAuf { name: Some(kunde()), standort: None },
        ..str_auf_auf()
    }
}

pub fn str_aen_abg() -> Storno {
    Storno {
        storno_id: StornoId::allocate(&cc("DEU.EKP2"), 4).unwrap(),
        art: StornoArt::AenderungAbg {
            name: None,
            standort: None,
            stornogrund: Freitext::new("Umzug des Endkunden").unwrap(),
        },
        ..str_auf_auf()
    }
}

pub fn tvs() -> Terminverschiebung {
    Terminverschiebung {
        ekp: ekp(),
        aenderungs_id: AenderungsId::allocate(&cc("DEU.EKP1"), 1).unwrap(),
        bezug: Bezug::Id(va_id()),
        name: None,
        neuer_kundenwunschtermin: date("2027-01-29"),
    }
}

pub fn kopf(absender: &str, fall: Geschaeftsfall) -> MeldungKopf {
    MeldungKopf { absender: cc(absender), vorabstimmungs_id: va_id(), ekp: ekp(), geschaeftsfall: fall }
}

pub fn ruem_va() -> RuemVa {
    RuemVa {
        kopf: kopf("DEU.EKP2", Geschaeftsfall::VaKueMrn),
        positionen: vec![
            Position::new(Meldungscode::Zwa).into(),
            PositionRuemVa {
                position: Position::new(Meldungscode::AdaPlz),
                adresse_abweichend: Some(AdresseAbweichend::Postleitzahl(Postleitzahl::new("10117").unwrap())),
            },
        ],
        ressourcen: vec![TechnischeRessource {
            line_id: Some(LineId::new("DEU.EKP2.1754639821").unwrap()),
            ..Default::default()
        }],
        technologie: Some(Technologie::new("001 TAL ISDN").unwrap()),
        wechseltermin: date("2027-01-15"),
        rufnummernportierung: Some(PortierteRufnummern::Rufnummern(vec![
            (nummer("30", "1234567"), Portierungskennung::new("D123").unwrap()),
            (nummer("30", "1234568"), Portierungskennung::new("D123").unwrap()),
        ])),
    }
}

pub fn abbm() -> Abbm {
    Abbm {
        kopf: kopf("DEU.EKP2", Geschaeftsfall::VaKueMrn),
        positionen: vec![PositionAbbm {
            position: Position::new(Meldungscode::Rng),
            rufnummern: vec![Rufnummer::new("1234567").unwrap(), Rufnummer::new("1234568").unwrap()],
        }],
        wechseltermin: None,
        storno_id: None,
        aenderungs_id: None,
        begruendung: Some(Freitext::new("RN unbekannt").unwrap()),
    }
}

pub fn erlm() -> Erlm {
    Erlm {
        kopf: kopf("DEU.EKP2", Geschaeftsfall::TvsVa),
        positionen: vec![Position::new(Meldungscode::TerminverschiebungOk)],
        wechseltermin: Some(date("2027-01-29")),
        storno_id: None,
        aenderungs_id: Some(AenderungsId::allocate(&cc("DEU.EKP1"), 1).unwrap()),
    }
}

pub fn akm_tr() -> AkmTr {
    AkmTr {
        kopf: kopf("DEU.EKP1", Geschaeftsfall::VaKueMrn),
        positionen: vec![Position::new(Meldungscode::AntwortAufnehmend)],
        ressourcenuebernahme: true,
        uebernahme_leitung: vec![UebernahmeLeitung::LineId(LineId::new("DEU.EKP2.1754639821").unwrap())],
        sicherer_hafen: false,
        pki_auf: Some(Portierungskennung::new("D001").unwrap()),
    }
}

pub fn abbm_tr() -> AbbmTr {
    AbbmTr {
        kopf: kopf("DEU.EKP2", Geschaeftsfall::VaKueMrn),
        positionen: vec![Position::new(Meldungscode::LineIdFremd)],
    }
}

pub fn teq_ok() -> Teq {
    Teq {
        absender: cc("DEU.EKP2"),
        vorabstimmungs_id: va_id(),
        positionen: vec![Position::new(Meldungscode::Ok)],
        storno_id: None,
        aenderungs_id: None,
    }
}

/// Every sample as a named payload, the source of the golden fixtures.
pub fn all_payloads() -> Vec<(&'static str, Payload)> {
    let c = || control("2026-11-23T09:30:00+01:00");
    let req = |a: Anfrage| Payload::AnfrageRequest { control: c(), anfrage: a };
    let msg = |m: Meldung| Payload::MeldungRequest { control: c(), meldung: m };
    vec![
        ("va-kue-mrn", req(va_kue_mrn().into())),
        ("va-kue-orn", req(va_kue_orn().into())),
        ("va-rrnp", req(va_rrnp().into())),
        ("str-auf-ekpauf", req(str_auf_auf().into())),
        ("str-auf-ekpabg", req(str_auf_abg().into())),
        ("str-aen-ekpauf-original", req(str_aen_auf().into())),
        ("str-aen-ekpabg", req(str_aen_abg().into())),
        ("tvs-va", req(tvs().into())),
        ("ruem-va", msg(ruem_va().into())),
        ("abbm", msg(abbm().into())),
        ("erlm", msg(erlm().into())),
        ("akm-tr", msg(akm_tr().into())),
        ("abbm-tr", msg(abbm_tr().into())),
        ("teq-anfrage", Payload::AnfrageResponse { control: c(), quittung: teq_ok() }),
        ("teq-meldung", Payload::MeldungResponse { control: c(), quittung: teq_ok() }),
    ]
}
