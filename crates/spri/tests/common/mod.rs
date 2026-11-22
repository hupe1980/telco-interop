//! Synthetic sample values shared by the integration tests. Every name, number and address
//! here is invented.
#![allow(
    dead_code,
    unreachable_pub,
    clippy::unwrap_used,
    clippy::missing_panics_doc,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines
)]

use spri::*;

pub fn ts(s: &str) -> Timestamp {
    s.parse().unwrap()
}

pub fn date(s: &str) -> Date {
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

pub fn c() -> Control {
    control("2026-11-23T09:30:00+01:00")
}

pub fn va_id() -> VorabstimmungsId {
    "DEU.EKP1.V000000013".parse().unwrap()
}

pub fn line_id() -> LineId {
    "DEU.NB01.1754639821".parse().unwrap()
}

pub fn person() -> Person {
    Person {
        anrede: Anrede::Frau,
        vorname: Some(Vorname::new("Erika").unwrap()),
        nachname: Nachname::new("Mustermann").unwrap(),
    }
}

pub fn ansprechpartner() -> Ansprechpartner {
    Ansprechpartner {
        person: Person {
            anrede: Anrede::Herr,
            vorname: Some(Vorname::new("Max").unwrap()),
            nachname: Nachname::new("Beispiel").unwrap(),
        },
        telefonnummer: Telefonnummer::new("030 1234567").unwrap(),
        mobilfunknummer: Some(Telefonnummer::new("+49 171 7654321").unwrap()),
        faxnummer: Some(Telefonnummer::new("030 1234568").unwrap()),
        emailadresse: Some(Emailadresse::new("max.beispiel@example.de").unwrap()),
    }
}

pub fn ansprechpartner_auftrag() -> AnsprechpartnerAuftrag {
    AnsprechpartnerAuftrag {
        auftragsmanagement: ansprechpartner(),
        weitere: vec![AnsprechpartnerMitRolle {
            ansprechpartner: ansprechpartner(),
            rolle: AnsprechpartnerRolle::Technik,
        }],
    }
}

pub fn strasse() -> Strasse {
    Strasse {
        strassenname: Strassenname::new("Hauptstraße").unwrap(),
        hausnummer: Hausnummer::new("12").unwrap(),
        hausnummern_zusatz: Some(HausnummernZusatz::new("a").unwrap()),
    }
}

pub fn ort() -> Ort {
    Ort { ortsname: Ortsname::new("Berlin").unwrap(), ortsteil: Some(Ortsteil::new("Mitte").unwrap()) }
}

pub fn standort_a() -> StandortA {
    StandortA {
        inhaber: Inhaber::Person(person()),
        telefonnummer: Some(Telefonnummer::new("030 9876543").unwrap()),
        mobilfunknummer: None,
        strasse: strasse(),
        gebaeudeteil: Some(GebaeudeteilAngabe {
            name: Gebaeudeteil::new("Haus B").unwrap(),
            zusatz: Some(Gebaeudeteil::new("Hinterhof").unwrap()),
        }),
        land: Some(Land::new("de").unwrap()),
        postleitzahl: Postleitzahl::new("10115").unwrap(),
        ort: ort(),
        lage_tae_ont: Some(LageTaeOnt::new("Flur, links").unwrap()),
    }
}

pub fn anlage() -> Anlage {
    Anlage {
        dateiname: Dateiname::new("Kuendigung Mustermann.pdf").unwrap(),
        dateityp: Dokumenttyp::Pdf,
        beschreibung: Some(Beschreibung::new("Kündigungsschreiben").unwrap()),
        inhalt: b"%PDF-1.4\n% synthetic test document\n".to_vec(),
        anlagentyp: Anlagentyp::Kuendigungsschreiben,
    }
}

pub fn produkt(name: &str) -> Produkt {
    Produkt { bezeichner: Produktbezeichner::new(name).unwrap() }
}

pub fn termin() -> Wunschtermin {
    Wunschtermin { datum: date("2027-01-15"), zeitfenster: Some(Zeitfenster::WERKTAGS_VORMITTAG) }
}

pub fn montageleistung() -> Montageleistung {
    Montageleistung {
        ansprechpartner: ansprechpartner(),
        montagehinweis: Some(Montagehinweis::new("Bitte vorher anrufen").unwrap()),
    }
}

fn auftrag(fall: Geschaeftsfall, art: GeschaeftsfallArt) -> Auftrag {
    Auftrag {
        externe_auftragsnummer: ExterneAuftragsnummer::new("A-2026-0001").unwrap(),
        auftraggeber: Auftraggeber {
            auftraggebernummer: Auftraggebernummer::new("4711").unwrap(),
            leistungsnummer: Leistungsnummer::new("RV-1").unwrap(),
        },
        besteller: None,
        geschaeftsfall: fall,
        geschaeftsfall_art: art,
        aenderungskennzeichen: Aenderungskennzeichen::Standard,
        termin_neu: None,
        kenner: None,
    }
}

pub fn vertragsnummer() -> Vertragsnummer {
    Vertragsnummer::new("V123456").unwrap()
}

pub fn neu() -> Auftrag {
    let mut a = auftrag(
        Geschaeftsfall::Neu(Bereitstellung {
            vorabstimmungs_id: Some(va_id()),
            anlage: Some(anlage()),
            ansprechpartner: ansprechpartner_auftrag(),
            wunschtermin: termin(),
            produkt: produkt("FTTH 1000"),
            fttx: FttxBereitstellung {
                standort_a: standort_a(),
                standort_versand: Some(StandortVersand {
                    inhaber: Inhaber::Firma(Firma {
                        anrede: AnredeFirma::Firma,
                        firmenname: Firmenname::new("Beispiel GmbH").unwrap(),
                        firmenname_zweiter_teil: Some(Nachname::new("& Co. KG").unwrap()),
                    }),
                    strasse: strasse(),
                    land: None,
                    postleitzahl: Postleitzahl::new("10117").unwrap(),
                    ort: Ort { ortsname: Ortsname::new("Berlin").unwrap(), ortsteil: None },
                }),
                vormieter: Some(Vormieter {
                    person: Some(VormieterPerson { vorname: None, nachname: Nachname::new("Vormann").unwrap() }),
                    home_id: Some(HomeId::new("HID0001").unwrap()),
                    seriennummer_ont: Some(SeriennummerOnt::new("ONT-0001").unwrap()),
                }),
                montageleistung: Some(montageleistung()),
                wita: WitaExtendedInfos {
                    wita: WitaInfos {
                        wita_vertragsnummer: Some(WitaVertragsnummer::new("W123").unwrap()),
                        technologie: Some(Technologie::new("FTTH").unwrap()),
                    },
                    bestandssuche: Some(OnkzRufnummer {
                        onkz: Onkz::new("30").unwrap(),
                        rufnummer: Rufnummer::new("1234567").unwrap(),
                    }),
                    rufnummern_portierung: Some(RufnummernPortierung {
                        anschluss: PortierungAnschluss::Rufnummern(vec![OnkzRufnummer {
                            onkz: Onkz::new("30").unwrap(),
                            rufnummer: Rufnummer::new("1234567").unwrap(),
                        }]),
                        portierungskenner: Portierungskenner::new("D123").unwrap(),
                    }),
                    abgebender_ekp: Some(true),
                },
            },
            positionen: vec![produkt("Router"), produkt("Montage")],
        }),
        GeschaeftsfallArt::Bereitstellung,
    );
    a.besteller = Some(Auftraggeber {
        auftraggebernummer: Auftraggebernummer::new("4712").unwrap(),
        leistungsnummer: Leistungsnummer::new("RV-2").unwrap(),
    });
    a.termin_neu = Some(date("2027-01-20"));
    a.kenner = Some(Kenner {
        projekt_id: Some(ProjektId {
            projektkenner: Projektkenner::new("P-1").unwrap(),
            kopplungskenner: Some(Projektkenner::new("K-1").unwrap()),
        }),
        auftragskenner: Some(Auftragskenner { auftragsklammer: 9_999_999_999, anzahl_auftraege: 2 }),
    });
    a
}

pub fn kue_ag() -> Auftrag {
    auftrag(
        Geschaeftsfall::KueAg(Kuendigung {
            vertragsnummer: vertragsnummer(),
            line_id: line_id(),
            vorabstimmungs_id: Some(va_id()),
            anlage: Some(anlage()),
            ansprechpartner: ansprechpartner_auftrag(),
            wunschtermin: Wunschtermin::from(date("2027-02-01")),
            fttx: Some(WitaInfos {
                wita_vertragsnummer: Some(WitaVertragsnummer::new("W123").unwrap()),
                technologie: None,
            }),
        }),
        GeschaeftsfallArt::Kuendigung,
    )
}

pub fn lae() -> Auftrag {
    auftrag(
        Geschaeftsfall::Lae(Leistungsaenderung {
            vertragsnummer: vertragsnummer(),
            line_id: line_id(),
            anlage: None,
            ansprechpartner: ansprechpartner_auftrag(),
            wunschtermin: termin(),
            produkt: produkt("FTTH 500"),
            positionen: vec![AenderungPosition { produkt: produkt("FTTH 1000"), aktionscode: Aktionscode::Aenderung }],
        }),
        GeschaeftsfallArt::Aenderung,
    )
}

pub fn aen_lmae() -> Auftrag {
    auftrag(
        Geschaeftsfall::AenLmae(LeistungsmerkmalAenderung {
            vertragsnummer: vertragsnummer(),
            line_id: line_id(),
            anlage: None,
            ansprechpartner: ansprechpartner_auftrag(),
            wunschtermin: termin(),
            produkt: produkt("FTTH 1000"),
            positionen: vec![
                AenderungPosition { produkt: produkt("Feste IP"), aktionscode: Aktionscode::Zugang },
                AenderungPosition { produkt: produkt("IPTV"), aktionscode: Aktionscode::Wegfall },
            ],
        }),
        GeschaeftsfallArt::Aenderung,
    )
}

pub fn est() -> Auftrag {
    auftrag(
        Geschaeftsfall::Est(Entstoerung {
            vertragsnummer: vertragsnummer(),
            line_id: line_id(),
            anlage: None,
            ansprechpartner: ansprechpartner_auftrag(),
            kontakt_endkunde: KontaktEndkunde {
                anrede: Anrede::Frau,
                vorname: Vorname::new("Erika").unwrap(),
                nachname: Nachname::new("Mustermann").unwrap(),
                telefonnummer: Some(Telefonnummer::new("030 9876543").unwrap()),
                mobilfunknummer: None,
                faxnummer: None,
                emailadresse: Some(Emailadresse::new("erika@example.org").unwrap()),
                kontaktaufnahme_erwuenscht: true,
            },
            wunschtermin: Wunschtermin {
                datum: date("2026-11-24"),
                zeitfenster: Some(Zeitfenster::WERKTAGS_NACHMITTAG),
            },
            stoerungs_nr_auftraggeber: StoerungsNr::new("ST-1").unwrap(),
            reklamation: Some(Reklamation {
                stoerungs_nr_leistungserbringer: StoerungsNr::new("LE-9").unwrap(),
                reklamationsgrund: Freitext::new("Störung besteht weiter").unwrap(),
            }),
            vorpruefung_erfolgt: true,
            stoerungsbeschreibung: Freitext::new("Kein Sync seit Umzug\r\nONT blinkt rot").unwrap(),
            expressentstoerung: Some(false),
        }),
        GeschaeftsfallArt::Entstoerung,
    )
}

pub fn pv() -> Auftrag {
    auftrag(
        Geschaeftsfall::Pv(Providerwechsel {
            line_id: line_id(),
            vorabstimmungs_id: va_id(),
            anlage: None,
            ansprechpartner: ansprechpartner_auftrag(),
            wunschtermin: termin(),
            produkt: produkt("FTTH 1000"),
            fttx: FttxProviderwechsel { standort_a: standort_a(), montageleistung: Some(montageleistung()) },
            positionen: vec![produkt("Router")],
        }),
        GeschaeftsfallArt::Endkundenanbieterwechsel,
    )
}

pub fn diagnose() -> Diagnose {
    Diagnose {
        vertragsnummer: vertragsnummer(),
        line_id: line_id(),
        operationen: vec![Diagnosename::new("CurrBBDS").unwrap(), Diagnosename::new("CurrBBUS").unwrap()],
    }
}

pub fn get() -> Auftrag {
    auftrag(Geschaeftsfall::Get(diagnose()), GeschaeftsfallArt::Diagnose)
}

pub fn set() -> Auftrag {
    let d = Diagnose { operationen: vec![Diagnosename::new("PortReset").unwrap()], ..diagnose() };
    auftrag(Geschaeftsfall::Set(d), GeschaeftsfallArt::Diagnose)
}

pub fn orders() -> Vec<(&'static str, Auftrag)> {
    vec![
        ("auftrag-neu", neu()),
        ("auftrag-kue-ag", kue_ag()),
        ("auftrag-lae", lae()),
        ("auftrag-aen-lmae", aen_lmae()),
        ("auftrag-est", est()),
        ("auftrag-pv", pv()),
        ("auftrag-get", get()),
        ("auftrag-set", set()),
    ]
}

// -- messages --------------------------------------------------------------------------------

pub fn ext() -> ExterneAuftragsnummer {
    ExterneAuftragsnummer::new("A-2026-0001").unwrap()
}

pub fn ag() -> Auftraggebernummer {
    Auftraggebernummer::new("4711").unwrap()
}

pub fn pos(code: &str) -> Position {
    Position::new(Meldungscode::parse(code))
}

pub fn wita_pos(code: &str) -> PositionWita {
    PositionWita {
        position: pos(code),
        wita_meldungscode: Some(WitaMeldungscode::new(code).unwrap()),
        wita_meldungstext: Some(WitaMeldungstext::new("WITA-Text").unwrap()),
    }
}

pub fn meldung(fall: GeschaeftsfallCode, art: GeschaeftsfallArt, inhalt: impl Into<Meldungsinhalt>) -> Meldung {
    Meldung {
        auftragstyp: Auftragstyp {
            geschaeftsfall: fall,
            aenderungskennzeichen: Aenderungskennzeichen::Standard,
            geschaeftsfallart: art,
        },
        inhalt: inhalt.into(),
    }
}

pub fn rueckmeldung() -> ProviderwechselRueckmeldung {
    ProviderwechselRueckmeldung {
        providername: Some(Providername::new("Alter Anbieter GmbH").unwrap()),
        zustimmung: JaNein::Ja,
        antwortcode: Some(Antwortcode::new("0021").unwrap()),
        antworttext: Some(Antworttext::new("Zustimmung").unwrap()),
    }
}

pub fn qeb() -> Qeb {
    Qeb {
        externe_auftragsnummer: ext(),
        auftraggebernummer: ag(),
        auftraggebernummer_besteller: Some(Auftraggebernummer::new("4712").unwrap()),
        vorabstimmungs_id: Some(va_id()),
        stoerungs_nr_leistungserbringer: None,
        positionen: vec![PositionQeb {
            position: pos("0000"),
            ansprechpartner_leistungserbringer: Some(ansprechpartner()),
        }],
    }
}

pub fn abm() -> Abm {
    Abm {
        vertragsnummer: vertragsnummer(),
        externe_auftragsnummer: ext(),
        auftraggebernummer: ag(),
        auftraggebernummer_besteller: None,
        vorabstimmungs_id: Some(va_id()),
        verbindlicher_liefertermin: termin(),
        abgebender_provider: Some(rueckmeldung()),
        anschluss: FttxAnschluss {
            a10nsp: A10nsp::new("A10-1").unwrap(),
            port: Port::new("P1").unwrap(),
            s_vlan: Some(Vlan::new("100").unwrap()),
            c_vlan: Some(Vlan::new("7").unwrap()),
            line_id: line_id(),
            uebergabepunkt_kunde: UebergabepunktKunde::Rj45,
            wita_vertragsnummer: None,
        },
        positionen: vec![
            PositionAbm { position: wita_pos("0000"), ansprechpartner_leistungserbringer: Some(ansprechpartner()) },
            pos("0002").into(),
        ],
    }
}

pub fn abbm() -> Abbm {
    Abbm {
        vertragsnummer: None,
        externe_auftragsnummer: ext(),
        auftraggebernummer: ag(),
        auftraggebernummer_besteller: None,
        vorabstimmungs_id: None,
        wiedervorlagetermin: Some(date("2026-12-01")),
        abgebender_provider: None,
        positionen: vec![wita_pos("1001")],
    }
}

pub fn erlm() -> Erlm {
    Erlm {
        vertragsnummer: Some(vertragsnummer()),
        externe_auftragsnummer: ext(),
        auftraggebernummer: ag(),
        auftraggebernummer_besteller: None,
        vorabstimmungs_id: None,
        erledigungstermin: date("2027-01-15"),
        entstoer_ergebnis: Some(EntstoerErgebnis {
            ergebnis: Entstoerergebnis::FehlerBeseitigt,
            entstoer_zeitpunkt: Some(date("2027-01-15")),
            anmerkung: Some(Freitext::new("Spleiß erneuert").unwrap()),
        }),
        anlage: Some(anlage()),
        diagnoseergebnisse: vec![DiagnoseErgebnis {
            name: Diagnosename::new("CurrBBDS").unwrap(),
            wert: Diagnosewert::new("940.2").unwrap(),
        }],
        positionen: vec![pos("0010").into()],
    }
}

pub fn entm() -> Entm {
    Entm {
        vertragsnummer: vertragsnummer(),
        externe_auftragsnummer: ext(),
        auftraggebernummer: ag(),
        auftraggebernummer_besteller: None,
        vorabstimmungs_id: None,
        entgelttermin: date("2027-01-16"),
        anlage: None,
        positionen: vec![pos("0010")],
    }
}

pub fn vzm() -> Vzm {
    Vzm {
        vertragsnummer: None,
        externe_auftragsnummer: ext(),
        auftraggebernummer: ag(),
        auftraggebernummer_besteller: None,
        vorabstimmungs_id: None,
        verzoegerungstermin: Some(date("2027-01-29")),
        verzoegerungsgrund: Some(Freitext::new("Tiefbau").unwrap()),
        positionen: vec![wita_pos("0070")],
    }
}

pub fn tam() -> Terminanforderung {
    Terminanforderung {
        vertragsnummer: vertragsnummer(),
        externe_auftragsnummer: ext(),
        auftraggebernummer: ag(),
        auftraggebernummer_besteller: None,
        vorabstimmungs_id: None,
        positionen: vec![wita_pos("6001")],
    }
}

pub fn kdle() -> Kdle {
    Kdle {
        vertragsnummer: vertragsnummer(),
        externe_auftragsnummer: ext(),
        auftraggebernummer: ag(),
        abschaltungstermin: date("2027-06-30"),
        abschaltungsgrund: Some(Freitext::new("Netzumbau").unwrap()),
        positionen: vec![pos("8006")],
    }
}

pub fn zwm_le() -> ZwmLe {
    ZwmLe {
        vertragsnummer: vertragsnummer(),
        externe_auftragsnummer: ext(),
        auftraggebernummer: ag(),
        auftraggebernummer_besteller: None,
        vorabstimmungs_id: Some(va_id()),
        nachricht: Freitext::new("ONT wurde versandt").unwrap(),
        ansprechpartner: Some(AnsprechpartnerMitRolle {
            ansprechpartner: ansprechpartner(),
            rolle: AnsprechpartnerRolle::Auftragsmanagement,
        }),
        anlage: None,
        versand_ont: Some(VersandinfosOnt {
            seriennummer_ont: SeriennummerOnt::new("ONT-0002").unwrap(),
            unternehmen: Versandunternehmen::new("DHL").unwrap(),
            sendungsnummer: Sendungsnummer::new("00340434169999000011").unwrap(),
        }),
        positionen: vec![pos("0073")],
    }
}

pub fn zwm_ag() -> ZwmAg {
    ZwmAg {
        vertragsnummer: vertragsnummer(),
        externe_auftragsnummer: ext(),
        auftraggebernummer: ag(),
        auftraggebernummer_besteller: None,
        nachricht: Freitext::new("Kunde ist ab 14 Uhr erreichbar").unwrap(),
        ansprechpartner: None,
        anlage: Some(anlage()),
        positionen: vec![pos("7040")],
    }
}

pub fn tbk() -> Tbk {
    Tbk {
        vertragsnummer: vertragsnummer(),
        externe_auftragsnummer: ext(),
        auftraggebernummer: ag(),
        auftraggebernummer_besteller: None,
        termin: Termin {
            datum: date("2027-01-15"),
            zeit: Some("09:30:00".parse().unwrap()),
            zeitfenster: Some(Zeitfenster::WERKTAGS_VORMITTAG),
        },
        positionen: vec![pos("7039")],
    }
}

pub fn akm_pv() -> AkmPv {
    AkmPv {
        vertragsnummer: Vertragsnummer::new("V999").unwrap(),
        externe_auftragsnummer: ext(),
        auftraggebernummer: Auftraggebernummer::new("0815").unwrap(),
        vorabstimmungs_id: Some(va_id()),
        line_id: line_id(),
        anlage: None,
        aufnehmender_provider: ProviderwechselAnkuendigung {
            providername: Providername::new("Neuer Anbieter AG").unwrap(),
            uebernahmedatum_geplant: date("2027-01-15"),
            antwortfrist: date("2026-12-01"),
        },
        positionen: vec![pos("0020")],
    }
}

pub fn ruem_pv() -> RuemPv {
    RuemPv {
        vertragsnummer: Vertragsnummer::new("V999").unwrap(),
        externe_auftragsnummer: ext(),
        auftraggebernummer: Auftraggebernummer::new("0815").unwrap(),
        vorabstimmungs_id: Some(va_id()),
        line_id: line_id(),
        abgebender_provider: rueckmeldung(),
        positionen: vec![pos("0021")],
    }
}

pub fn abm_pv() -> AbmPv {
    AbmPv {
        vertragsnummer: Vertragsnummer::new("V999").unwrap(),
        externe_auftragsnummer: ext(),
        auftraggebernummer: Auftraggebernummer::new("0815").unwrap(),
        line_id: line_id(),
        aufnehmender_provider: ProviderwechselBestaetigung {
            providername: Providername::new("Neuer Anbieter AG").unwrap(),
            uebernahmedatum_verbindlich: date("2027-01-15"),
        },
        positionen: vec![pos("0030")],
    }
}

pub fn erlm_pv() -> ErlmPv {
    ErlmPv {
        vertragsnummer: Vertragsnummer::new("V999").unwrap(),
        externe_auftragsnummer: ext(),
        auftraggebernummer: Auftraggebernummer::new("0815").unwrap(),
        erledigungstermin: date("2027-01-15"),
        positionen: vec![pos("0010")],
    }
}

pub fn entm_pv() -> EntmPv {
    EntmPv {
        vertragsnummer: Vertragsnummer::new("V999").unwrap(),
        externe_auftragsnummer: ext(),
        auftraggebernummer: Auftraggebernummer::new("0815").unwrap(),
        entgelttermin: date("2027-01-15"),
        positionen: vec![pos("0010")],
    }
}

pub fn abbm_pv() -> AbbmPv {
    AbbmPv {
        vertragsnummer: Some(Vertragsnummer::new("V999").unwrap()),
        externe_auftragsnummer: ext(),
        auftraggebernummer: Auftraggebernummer::new("0815").unwrap(),
        positionen: vec![pos("0040")],
    }
}

pub fn erlm_k() -> ErlmK {
    ErlmK {
        vertragsnummer: vertragsnummer(),
        externe_auftragsnummer: ext(),
        auftraggebernummer: ag(),
        auftraggebernummer_besteller: None,
        vorabstimmungs_id: None,
        positionen: vec![pos("0015")],
    }
}

pub fn messages() -> Vec<(&'static str, Meldung)> {
    use GeschaeftsfallArt as A;
    use GeschaeftsfallCode as G;
    vec![
        ("meldung-qeb", meldung(G::Neu, A::Bereitstellung, qeb())),
        ("meldung-abm", meldung(G::Pv, A::Endkundenanbieterwechsel, abm())),
        ("meldung-abbm", meldung(G::Neu, A::Bereitstellung, abbm())),
        ("meldung-erlm", meldung(G::Est, A::Entstoerung, erlm())),
        ("meldung-entm", meldung(G::Neu, A::Bereitstellung, entm())),
        ("meldung-vzm", meldung(G::Neu, A::Bereitstellung, vzm())),
        ("meldung-tam", meldung(G::Neu, A::Bereitstellung, Meldungsinhalt::Tam(tam()))),
        ("meldung-mtam", meldung(G::Neu, A::Bereitstellung, Meldungsinhalt::Mtam(tam()))),
        ("meldung-kdle", meldung(G::KueLe, A::Kuendigung, kdle())),
        ("meldung-zwm-le", meldung(G::Neu, A::Bereitstellung, zwm_le())),
        ("meldung-zwm-ag", meldung(G::Est, A::Entstoerung, zwm_ag())),
        ("meldung-tbk-le", meldung(G::Est, A::Entstoerung, Meldungsinhalt::TbkLe(tbk()))),
        (
            "meldung-tbk-ag",
            meldung(G::Est, A::Entstoerung, Meldungsinhalt::TbkAg(Tbk { positionen: vec![pos("7030")], ..tbk() })),
        ),
        ("meldung-akm-pv", meldung(G::Pv, A::Endkundenanbieterwechsel, akm_pv())),
        ("meldung-ruem-pv", meldung(G::Pv, A::Endkundenanbieterwechsel, ruem_pv())),
        ("meldung-abm-pv", meldung(G::Pv, A::Endkundenanbieterwechsel, abm_pv())),
        ("meldung-erlm-pv", meldung(G::Pv, A::Endkundenanbieterwechsel, erlm_pv())),
        ("meldung-entm-pv", meldung(G::Pv, A::Endkundenanbieterwechsel, entm_pv())),
        ("meldung-abbm-pv", meldung(G::Pv, A::Endkundenanbieterwechsel, abbm_pv())),
        ("meldung-erlm-k", meldung(G::Neu, A::Bereitstellung, erlm_k())),
    ]
}

pub fn teq_ok() -> Teq {
    Teq::new(Meldungscode::Ok, Some(ext()), Some(ag()))
}

/// One payload per order kind, message kind and receipt, with its fixture name.
pub fn all_payloads() -> Vec<(&'static str, Payload)> {
    let mut out: Vec<(&'static str, Payload)> =
        orders().into_iter().map(|(n, auftrag)| (n, Payload::AuftragRequest { control: c(), auftrag })).collect();
    out.extend(messages().into_iter().map(|(n, meldung)| (n, Payload::MeldungRequest { control: c(), meldung })));
    out.push(("teq-auftrag", Payload::AuftragResponse { control: c(), quittung: teq_ok() }));
    out.push((
        "teq-meldung",
        Payload::MeldungResponse {
            control: c(),
            quittung: Teq {
                meldungstyp: Meldungstyp::Teq,
                externe_auftragsnummer: None,
                auftraggebernummer: None,
                positionen: vec![Position::new(Meldungscode::DATENSTRUKTUR_UNZULAESSIG)],
            },
        },
    ));
    out
}
