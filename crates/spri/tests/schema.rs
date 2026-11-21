//! Conformance: everything the library writes validates against the AK S&P's S/PRI XML schema
//! 4.00.03 (Anlage 3), fetched by `scripts/fetch-spri-xsd.sh` and never committed.
//!
//! Without the schema or `xmllint` the tests report themselves skipped; the CI schema job sets
//! `SPRI_XSD_REQUIRED=1`, which turns a skip into a failure.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::*;
use spri::*;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The schema folder, or `None` (skipped) — a failure when the job requires the schema.
fn schema_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("SPRI_XSD_DIR").map_or_else(|| workspace().join("target/spri-xsd"), PathBuf::from);
    let required = std::env::var("SPRI_XSD_REQUIRED").is_ok_and(|v| v == "1");
    let xmllint = Command::new("xmllint").arg("--version").output().is_ok();
    let present = dir.join("wsdl/oss-envelope.xsd").is_file();
    match availability(present, xmllint, required) {
        Ok(true) => Some(dir),
        Ok(false) => {
            eprintln!("SKIPPED: S/PRI schema or xmllint not available — run `just schema`");
            None
        }
        Err(why) => panic!("{why}"),
    }
}

/// Whether to run (`Ok(true)`), skip (`Ok(false)`) or fail (`Err`): never a silent pass where
/// the check is required.
fn availability(schema: bool, xmllint: bool, required: bool) -> Result<bool, String> {
    match (schema && xmllint, required) {
        (true, _) => Ok(true),
        (false, true) => Err(format!(
            "SPRI_XSD_REQUIRED=1 but {} (run scripts/fetch-spri-xsd.sh; install xmllint)",
            if schema { "xmllint is missing" } else { "the schema is missing" }
        )),
        (false, false) => Ok(false),
    }
}

// Required and missing is a failure naming what is missing; not required is a skip, never a
// pass.
#[test]
fn a_required_check_fails_without_schema_or_validator() {
    assert!(availability(false, true, true).unwrap_err().contains("the schema is missing"));
    assert!(availability(true, false, true).unwrap_err().contains("xmllint is missing"));
    assert_eq!(availability(false, true, false), Ok(false));
    assert_eq!(availability(true, true, false), Ok(true));
}

/// Runs xmllint on `files`; returns the failures with the validator's messages.
fn xmllint(schema: &Path, files: &[PathBuf]) -> Vec<String> {
    let mut failures = Vec::new();
    for chunk in files.chunks(200) {
        // No network access for the validator.
        let out = Command::new("xmllint")
            .args(["--nonet", "--noout", "--schema"])
            .arg(schema.join("wsdl/oss-envelope.xsd"))
            .args(chunk)
            .output()
            .expect("xmllint runs");
        let stderr = String::from_utf8_lossy(&out.stderr);
        let errors: Vec<&str> = stderr.lines().filter(|l| !l.ends_with(" validates")).collect();
        if !errors.is_empty() {
            failures.extend(errors.into_iter().map(str::to_owned));
        }
    }
    failures
}

fn write_corpus(corpus: &[(String, Payload)]) -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("spri-corpus");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    corpus
        .iter()
        .map(|(name, payload)| {
            let path = dir.join(format!("{name}.xml"));
            std::fs::write(&path, payload.to_xml().unwrap_or_else(|e| panic!("{name}: {e}"))).unwrap();
            path
        })
        .collect()
}

fn order(a: Auftrag) -> Payload {
    Payload::AuftragRequest { control: c(), auftrag: a }
}

fn msg(m: Meldung) -> Payload {
    Payload::MeldungRequest { control: c(), meldung: m }
}

fn art(fall: GeschaeftsfallCode) -> GeschaeftsfallArt {
    match fall {
        GeschaeftsfallCode::Neu => GeschaeftsfallArt::Bereitstellung,
        GeschaeftsfallCode::KueAg | GeschaeftsfallCode::KueLe => GeschaeftsfallArt::Kuendigung,
        GeschaeftsfallCode::Lae | GeschaeftsfallCode::AenLmae => GeschaeftsfallArt::Aenderung,
        GeschaeftsfallCode::Est => GeschaeftsfallArt::Entstoerung,
        GeschaeftsfallCode::Pv => GeschaeftsfallArt::Endkundenanbieterwechsel,
        GeschaeftsfallCode::Get | GeschaeftsfallCode::Set => GeschaeftsfallArt::Diagnose,
    }
}

/// A message of `typ` in business case `fall` carrying the single code `code`.
fn coded(typ: Meldungstyp, fall: GeschaeftsfallCode, code: &Meldungscode) -> Payload {
    let p = Position::new(code.clone());
    let inhalt: Meldungsinhalt = match typ {
        Meldungstyp::Teq => {
            return Payload::AuftragResponse { control: c(), quittung: Teq { positionen: vec![p], ..teq_ok() } };
        }
        Meldungstyp::Qeb => Qeb { positionen: vec![p.into()], ..qeb() }.into(),
        Meldungstyp::Abm => Abm { positionen: vec![p.into()], ..abm() }.into(),
        Meldungstyp::Abbm => Abbm { positionen: vec![p.into()], ..abbm() }.into(),
        Meldungstyp::Erlm => Erlm { positionen: vec![p.into()], ..erlm() }.into(),
        Meldungstyp::Entm => Entm { positionen: vec![p], ..entm() }.into(),
        Meldungstyp::Vzm => Vzm { positionen: vec![p.into()], ..vzm() }.into(),
        Meldungstyp::Tam => Meldungsinhalt::Tam(Terminanforderung { positionen: vec![p.into()], ..tam() }),
        Meldungstyp::Mtam => Meldungsinhalt::Mtam(Terminanforderung { positionen: vec![p.into()], ..tam() }),
        Meldungstyp::Kdle => Kdle { positionen: vec![p], ..kdle() }.into(),
        Meldungstyp::ZwmLe => ZwmLe { positionen: vec![p], ..zwm_le() }.into(),
        Meldungstyp::ZwmAg => ZwmAg { positionen: vec![p], ..zwm_ag() }.into(),
        Meldungstyp::TbkLe => Meldungsinhalt::TbkLe(Tbk { positionen: vec![p], ..tbk() }),
        Meldungstyp::TbkAg => Meldungsinhalt::TbkAg(Tbk { positionen: vec![p], ..tbk() }),
        Meldungstyp::AkmPv => AkmPv { positionen: vec![p], ..akm_pv() }.into(),
        Meldungstyp::RuemPv => RuemPv { positionen: vec![p], ..ruem_pv() }.into(),
        Meldungstyp::AbmPv => AbmPv { positionen: vec![p], ..abm_pv() }.into(),
        Meldungstyp::ErlmPv => ErlmPv { positionen: vec![p], ..erlm_pv() }.into(),
        Meldungstyp::EntmPv => EntmPv { positionen: vec![p], ..entm_pv() }.into(),
        Meldungstyp::AbbmPv => AbbmPv { positionen: vec![p], ..abbm_pv() }.into(),
        Meldungstyp::ErlmK => ErlmK { positionen: vec![p], ..erlm_k() }.into(),
    };
    msg(meldung(fall, art(fall), inhalt))
}

fn long(base: &str, len: usize) -> String {
    base.chars().cycle().take(len).collect()
}

fn with_fall(mut a: Auftrag, f: impl FnOnce(&mut Geschaeftsfall)) -> Auftrag {
    f(&mut a.geschaeftsfall);
    a
}

#[allow(clippy::too_many_lines)]
fn corpus() -> Vec<(String, Payload)> {
    let mut out: Vec<(String, Payload)> = Vec::new();
    let mut add = |name: String, p: Payload| out.push((name, p));

    // Every fixture kind.
    for (name, p) in all_payloads() {
        add(format!("fixture-{name}"), p);
    }

    // Every published code on every message type and business case it applies to.
    for code in Meldungscode::ALL {
        for typ in Meldungstyp::ALL {
            for fall in GeschaeftsfallCode::ALL {
                if code.applies(*typ, *fall) {
                    add(format!("code-{}-{typ}-{fall}", code.as_str()), coded(*typ, *fall, code));
                }
            }
        }
    }

    // Repetitions at their bounds.
    let mut a = neu();
    if let Geschaeftsfall::Neu(b) = &mut a.geschaeftsfall {
        b.positionen = (0..50).map(|i| produkt(&format!("Produkt {i}"))).collect();
        b.ansprechpartner.weitere = (0..20)
            .map(|_| AnsprechpartnerMitRolle {
                ansprechpartner: ansprechpartner(),
                rolle: AnsprechpartnerRolle::Endkunde,
            })
            .collect();
        b.fttx.wita.rufnummern_portierung = Some(RufnummernPortierung {
            anschluss: PortierungAnschluss::Rufnummern(
                (0..10)
                    .map(|i| OnkzRufnummer {
                        onkz: Onkz::new("30").unwrap(),
                        rufnummer: Rufnummer::new(format!("123456{i}")).unwrap(),
                    })
                    .collect(),
            ),
            portierungskenner: Portierungskenner::new("D001").unwrap(),
        });
    }
    add("max-neu-10-numbers".into(), order(a));
    let mut a = neu();
    if let Geschaeftsfall::Neu(b) = &mut a.geschaeftsfall {
        b.fttx.wita.rufnummern_portierung = Some(RufnummernPortierung {
            anschluss: PortierungAnschluss::Anlage {
                onkz: Onkz::new("89").unwrap(),
                durchwahlnummer: Durchwahlnummer::new("12345678").unwrap(),
                abfragestelle: Abfragestelle::new("0").unwrap(),
                bloecke: (0..5)
                    .map(|i| Rufnummernblock {
                        von: RufnummernBlock::new(format!("{i}00")).unwrap(),
                        bis: RufnummernBlock::new(format!("{i}99")).unwrap(),
                    })
                    .collect(),
            },
            portierungskenner: Portierungskenner::new("D001").unwrap(),
        });
    }
    add("max-neu-5-blocks".into(), order(a));
    let mut a = neu();
    if let Geschaeftsfall::Neu(b) = &mut a.geschaeftsfall {
        b.fttx.wita.rufnummern_portierung = Some(RufnummernPortierung {
            anschluss: PortierungAnschluss::AlleRufnummern,
            portierungskenner: Portierungskenner::new("").unwrap(),
        });
        b.anlage.as_mut().unwrap().inhalt = (0..=255u8).cycle().take(100_000).collect();
    }
    add("max-neu-alle-rufnummern-large-anlage".into(), order(a));
    add(
        "max-get-many-operations".into(),
        order(with_fall(get(), |g| {
            if let Geschaeftsfall::Get(d) = g {
                d.operationen = (0..30).map(|i| Diagnosename::new(format!("Op{i}")).unwrap()).collect();
            }
        })),
    );
    add(
        "max-erlm-many-results".into(),
        msg(meldung(
            GeschaeftsfallCode::Get,
            GeschaeftsfallArt::Diagnose,
            Erlm {
                diagnoseergebnisse: (0..30)
                    .map(|i| DiagnoseErgebnis {
                        name: Diagnosename::new(format!("Wert{i}")).unwrap(),
                        wert: Diagnosewert::new(long("1.5 dB; ", 1024)).unwrap(),
                    })
                    .collect(),
                positionen: (0..40).map(|_| wita_pos("0010")).collect(),
                ..erlm()
            },
        )),
    );

    // Texts at maximum length with umlauts and markup characters.
    let mut a = est();
    if let Geschaeftsfall::Est(e) = &mut a.geschaeftsfall {
        e.stoerungsbeschreibung = Freitext::new(long("Zeile <1> & \"2\"\r\n\tÄÖÜß€漢 ", 1024)).unwrap();
        e.stoerungs_nr_auftraggeber = StoerungsNr::new(long("<&>", 30)).unwrap();
        e.kontakt_endkunde.vorname = Vorname::new(long("Ä<&>\"'ßü ", 30)).unwrap();
        e.kontakt_endkunde.nachname = Nachname::new(long("Öß&<>'\"Ñ", 30)).unwrap();
        e.kontakt_endkunde.emailadresse = Some(Emailadresse::new(format!("{}@example.de", long("a.b", 300))).unwrap());
        e.kontakt_endkunde.telefonnummer = Some(Telefonnummer::new("+49 30123 12345678901234").unwrap());
    }
    a.externe_auftragsnummer = ExterneAuftragsnummer::new(long("A<&>", 20)).unwrap();
    add("text-max-est".into(), order(a));
    let mut a = pv();
    if let Geschaeftsfall::Pv(p) = &mut a.geschaeftsfall {
        let st = &mut p.fttx.standort_a;
        st.strasse.strassenname = Strassenname::new(long("Straße §°±²³-<&>", 40)).unwrap();
        st.strasse.hausnummer = Hausnummer::new("9999").unwrap();
        st.strasse.hausnummern_zusatz = Some(HausnummernZusatz::new("a-<&>b").unwrap());
        st.ort = Ort {
            ortsname: Ortsname::new(long("Frankfurt (Oder) ", 40)).unwrap(),
            ortsteil: Some(Ortsteil::new(long("Süd ", 40)).unwrap()),
        };
        st.lage_tae_ont = Some(LageTaeOnt::new(long("Keller <links> ", 60)).unwrap());
        st.postleitzahl = Postleitzahl::new("D-1011").unwrap();
        st.inhaber = Inhaber::Firma(Firma {
            anrede: AnredeFirma::Keine,
            firmenname: Firmenname::new(long("Müller & Söhne ", 30)).unwrap(),
            firmenname_zweiter_teil: None,
        });
        p.produkt = produkt(&long("FTTH <1000> & mehr ", 100));
    }
    add("text-max-pv".into(), order(a));
    add(
        "text-max-zwm-ag".into(),
        msg(meldung(
            GeschaeftsfallCode::Neu,
            GeschaeftsfallArt::Bereitstellung,
            ZwmAg {
                nachricht: Freitext::new(long("Hinweis <&> \"x\" ", 1024)).unwrap(),
                positionen: vec![Position {
                    code: Meldungscode::C0073,
                    text: Meldungstext::new(long("Text <&> ", 255)).unwrap(),
                }],
                ..zwm_ag()
            },
        )),
    );

    // Deterministic variants across optional elements, choices and enumerations.
    for i in 0..40u32 {
        let iu = usize::try_from(i).unwrap();
        let zf = if i % 5 == 0 { None } else { Some(Zeitfenster::new(["1", "2", "3", "99", "Z"][iu % 5]).unwrap()) };
        let kz = Aenderungskennzeichen::ALL[iu % 3];
        let mut a = [neu(), kue_ag(), lae(), aen_lmae(), est(), pv(), get(), set()][iu % 8].clone();
        a.aenderungskennzeichen = kz;
        a.besteller = None;
        a.termin_neu = (i % 2 == 0).then(|| date("2027-03-01"));
        a.kenner = match i % 4 {
            0 => None,
            1 => Some(Kenner::default()),
            2 => Some(Kenner {
                projekt_id: Some(ProjektId {
                    projektkenner: Projektkenner::new(format!("P{i}")).unwrap(),
                    kopplungskenner: None,
                }),
                auftragskenner: None,
            }),
            _ => Some(Kenner {
                projekt_id: None,
                auftragskenner: Some(Auftragskenner { auftragsklammer: u64::from(i), anzahl_auftraege: -1 }),
            }),
        };
        match &mut a.geschaeftsfall {
            Geschaeftsfall::Neu(b) => {
                b.vorabstimmungs_id = None;
                b.anlage = None;
                b.wunschtermin.zeitfenster = zf;
                b.fttx.standort_versand = None;
                b.fttx.vormieter = (i % 3 == 0).then(Vormieter::default);
                b.fttx.montageleistung = None;
                b.fttx.wita = WitaExtendedInfos::default();
                b.fttx.standort_a.inhaber = Inhaber::Person(Person {
                    anrede: Anrede::ALL[iu % 3],
                    vorname: None,
                    nachname: Nachname::new("X").unwrap(),
                });
                b.fttx.standort_a.land = None;
                b.fttx.standort_a.gebaeudeteil = None;
                b.positionen.clear();
            }
            Geschaeftsfall::KueAg(k) => {
                k.fttx = (i % 2 == 0).then(WitaInfos::default);
                k.vorabstimmungs_id = None;
                k.anlage = (i % 3 == 0).then(|| Anlage {
                    dateityp: Dokumenttyp::ALL[iu % 5],
                    anlagentyp: Anlagentyp::ALL[iu % 6],
                    beschreibung: None,
                    ..anlage()
                });
            }
            Geschaeftsfall::Lae(l) => l.positionen.clear(),
            Geschaeftsfall::Est(e) => {
                e.reklamation = None;
                e.expressentstoerung = None;
                e.kontakt_endkunde.telefonnummer = None;
                e.kontakt_endkunde.emailadresse = None;
            }
            Geschaeftsfall::Pv(p) => {
                p.fttx.montageleistung = None;
                p.positionen.clear();
                p.ansprechpartner.weitere.clear();
                p.ansprechpartner.auftragsmanagement.mobilfunknummer = None;
                p.ansprechpartner.auftragsmanagement.faxnummer = None;
                p.ansprechpartner.auftragsmanagement.emailadresse = None;
            }
            _ => {}
        }
        add(format!("variant-{i:02}-auftrag"), order(a));
        let fall = GeschaeftsfallCode::ALL[iu % 9];
        let mut m = messages()[iu % 20].1.clone();
        m.auftragstyp = Auftragstyp { geschaeftsfall: fall, aenderungskennzeichen: kz, geschaeftsfallart: art(fall) };
        match &mut m.inhalt {
            Meldungsinhalt::Abm(x) => {
                x.abgebender_provider = None;
                x.verbindlicher_liefertermin.zeitfenster = zf;
                x.anschluss.s_vlan = None;
                x.anschluss.c_vlan = None;
                x.anschluss.uebergabepunkt_kunde = UebergabepunktKunde::Tae;
                x.anschluss.wita_vertragsnummer = Some(WitaVertragsnummer::new("W1").unwrap());
            }
            Meldungsinhalt::Qeb(x) => {
                x.positionen[0].ansprechpartner_leistungserbringer = None;
                x.stoerungs_nr_leistungserbringer = Some(StoerungsNr::new("LE-1").unwrap());
            }
            Meldungsinhalt::Erlm(x) => {
                x.entstoer_ergebnis = Some(EntstoerErgebnis {
                    ergebnis: Entstoerergebnis::KeinenFehlerGefunden,
                    entstoer_zeitpunkt: None,
                    anmerkung: None,
                });
                x.anlage = None;
                x.diagnoseergebnisse.clear();
            }
            Meldungsinhalt::RuemPv(x) => {
                x.abgebender_provider = ProviderwechselRueckmeldung {
                    providername: None,
                    zustimmung: JaNein::Nein,
                    antwortcode: None,
                    antworttext: Some(Antworttext::new("").unwrap()),
                };
            }
            Meldungsinhalt::TbkLe(x) | Meldungsinhalt::TbkAg(x) => {
                x.termin.zeit = (i % 2 == 0).then(|| "23:59:59.5".parse().unwrap());
                x.termin.zeitfenster = zf;
            }
            Meldungsinhalt::ZwmLe(x) => {
                x.versand_ont = None;
                x.ansprechpartner = None;
                x.anlage = Some(anlage());
            }
            _ => {}
        }
        add(format!("variant-{i:02}-meldung"), msg(m));
        let mut ctl = c();
        ctl.zeitstempel = Zeitstempel {
            timestamp: ts("2026-11-23T09:30:00Z"),
            offset: jiff::tz::Offset::from_hours(i8::try_from(i % 15).unwrap() - 7).unwrap(),
        };
        ctl.release.minor = u8::try_from(i % 5).unwrap();
        let quittung = Teq {
            meldungstyp: Meldungstyp::ALL[iu % 21],
            externe_auftragsnummer: (i % 2 == 0).then(ext),
            auftraggebernummer: (i % 3 == 0).then(ag),
            positionen: (0..=iu % 3).map(|_| Position::new(Meldungscode::ALL[iu % 6].clone())).collect(),
        };
        add(format!("variant-{i:02}-teq"), Payload::MeldungResponse { control: ctl, quittung });
    }
    out
}

#[test]
fn corpus_is_valid_against_the_official_schema() {
    let corpus = corpus();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for (name, _) in &corpus {
        *counts.entry(name.split('-').next().unwrap()).or_default() += 1;
    }
    assert!(counts.values().all(|&n| n > 0));
    let Some(schema) = schema_dir() else { return };
    let files = write_corpus(&corpus);
    let failures = xmllint(&schema, &files);
    assert!(failures.is_empty(), "{} invalid payload(s):\n{}", failures.len(), failures.join("\n"));
    eprintln!("corpus: {} payloads valid against S/PRI XSD 4.00.03 — {counts:?}", corpus.len());
}

/// The validator must reject what the schema forbids, or the corpus test proves nothing.
#[test]
fn validator_rejects_a_reordered_payload() {
    let Some(schema) = schema_dir() else { return };
    let xml = order(pv()).to_xml().unwrap();
    let line = "      <lineId>DEU.NB01.1754639821</lineId>\n";
    let va = "      <vorabstimmungId>DEU.EKP1.V000000013</vorabstimmungId>\n";
    let reordered = xml.replacen(&format!("{line}{va}"), &format!("{va}{line}"), 1);
    assert_ne!(reordered, xml);
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("reordered.xml");
    std::fs::write(&path, reordered).unwrap();
    let failures = xmllint(&schema, &[path]);
    assert!(failures.iter().any(|l| l.contains("vorabstimmungId")), "{failures:?}");
}

fn fetch(zip: &Path, dir: &Path) -> std::process::Output {
    Command::new("bash")
        .arg(workspace().join("scripts/fetch-spri-xsd.sh"))
        .arg(dir)
        .env("SPRI_XSD_ZIP", zip)
        .output()
        .expect("bash runs")
}

// A zip with another hash is refused before anything is extracted.
#[test]
fn fetch_refuses_a_tampered_schema() {
    let Some(schema) = schema_dir() else { return };
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join("tampered");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let mut bytes = std::fs::read(schema.join("schema.zip")).unwrap();
    bytes.push(0);
    std::fs::write(tmp.join("schema.zip"), bytes).unwrap();
    let out = fetch(&tmp.join("schema.zip"), &tmp.join("xsd"));
    assert_eq!(out.status.code(), Some(2), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("expected sha256"));
    assert!(!tmp.join("xsd").exists(), "nothing may be extracted");
}

// Only the known files, by name, in a fresh folder.
#[test]
fn fetch_writes_only_the_known_files() {
    let Some(schema) = schema_dir() else { return };
    let out_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("fresh-xsd");
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::write(out_dir.join("stale.txt"), "left over").unwrap();
    let out = fetch(&schema.join("schema.zip"), &out_dir);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let mut files: Vec<String> = Vec::new();
    for sub in ["", "wsdl", "xsd"] {
        for e in std::fs::read_dir(out_dir.join(sub)).unwrap() {
            let e = e.unwrap();
            if e.file_type().unwrap().is_file() {
                files.push(format!("{sub}/{}", e.file_name().to_string_lossy()).trim_start_matches('/').to_owned());
            }
        }
    }
    files.sort();
    assert_eq!(
        files,
        [
            "schema.zip",
            "wsdl/oss-envelope.xsd",
            "wsdl/policy.xml",
            "wsdl/spri.wsdl",
            "xsd/oss-message.xsd",
            "xsd/oss-order.xsd",
            "xsd/oss-product-FTTx.xsd",
            "xsd/oss-type-complex.xsd",
            "xsd/oss-type-enum.xsd",
            "xsd/oss-type-simple.xsd",
        ]
    );
}
