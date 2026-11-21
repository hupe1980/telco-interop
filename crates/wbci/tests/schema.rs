//! Conformance: everything the library writes validates against the AK S&P's WBCI XML schema
//! V2.00 (Anlage 4), fetched by `scripts/fetch-wbci-xsd.sh` and never committed.
//!
//! Without the schema or `xmllint` the tests report themselves skipped; the CI schema job sets
//! `WBCI_XSD_REQUIRED=1`, which turns a skip into a failure.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::*;
use wbci::pruefung::{self, Bestand};
use wbci::*;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The schema folder, or `None` (skipped) — a failure when the job requires the schema.
fn schema_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("WBCI_XSD_DIR").map_or_else(|| workspace().join("target/wbci-xsd"), PathBuf::from);
    let required = std::env::var("WBCI_XSD_REQUIRED").is_ok_and(|v| v == "1");
    let xmllint = Command::new("xmllint").arg("--version").output().is_ok();
    let present = dir.join("wsdl/wbci-envelope.xsd").is_file();
    match availability(present, xmllint, required) {
        Ok(true) => Some(dir),
        Ok(false) => {
            eprintln!("SKIPPED: WBCI schema or xmllint not available — run `just schema`");
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
            "WBCI_XSD_REQUIRED=1 but {} (run scripts/fetch-wbci-xsd.sh; install xmllint)",
            if schema { "xmllint is missing" } else { "the schema is missing" }
        )),
        (false, false) => Ok(false),
    }
}

// schema-conformance/SR-004: required and missing is a failure naming what is missing; not
// required is a skip, never a pass.
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
        // schema-conformance/SR-003: no network access for the validator.
        let out = Command::new("xmllint")
            .args(["--nonet", "--noout", "--schema"])
            .arg(schema.join("wsdl/wbci-envelope.xsd"))
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
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("wbci-corpus");
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

fn c() -> Control {
    control("2026-11-23T09:30:00+01:00")
}

fn msg(m: impl Into<Meldung>) -> Payload {
    Payload::MeldungRequest { control: c(), meldung: m.into() }
}

fn req(a: impl Into<Anfrage>) -> Payload {
    Payload::AnfrageRequest { control: c(), anfrage: a.into() }
}

/// A message of `typ` answering `fall` with the single code `code`.
fn coded(typ: Meldungstyp, fall: Geschaeftsfall, code: &Meldungscode) -> Payload {
    let pos = Position::new(code.clone());
    let mut k = kopf("DEU.EKP2", fall);
    let storno = matches!(fall, Geschaeftsfall::StrAuf | Geschaeftsfall::StrAen)
        .then(|| StornoId::allocate(&cc("DEU.EKP1"), 1).unwrap());
    let aenderung = (fall == Geschaeftsfall::TvsVa).then(|| AenderungsId::allocate(&cc("DEU.EKP1"), 1).unwrap());
    match typ {
        Meldungstyp::Teq => Payload::AnfrageResponse {
            control: c(),
            quittung: Teq {
                absender: cc("DEU.EKP2"),
                vorabstimmungs_id: va_id(),
                positionen: vec![pos],
                storno_id: storno,
                aenderungs_id: aenderung,
            },
        },
        Meldungstyp::RuemVa => msg(RuemVa { kopf: k, positionen: vec![pos.into()], ..ruem_va() }),
        Meldungstyp::Abbm => {
            msg(Abbm { kopf: k, positionen: vec![pos.into()], storno_id: storno, aenderungs_id: aenderung, ..abbm() })
        }
        Meldungstyp::Erlm => {
            msg(Erlm { kopf: k, positionen: vec![pos], storno_id: storno, aenderungs_id: aenderung, ..erlm() })
        }
        Meldungstyp::AkmTr => {
            k.absender = cc("DEU.EKP1");
            msg(AkmTr { kopf: k, positionen: vec![pos], ..akm_tr() })
        }
        Meldungstyp::AbbmTr => msg(AbbmTr { kopf: k, positionen: vec![pos] }),
    }
}

fn long(base: &str, len: usize) -> String {
    base.chars().cycle().take(len).collect()
}

#[allow(clippy::too_many_lines)]
fn corpus() -> Vec<(String, Payload)> {
    let mut out: Vec<(String, Payload)> = Vec::new();
    let mut add = |name: String, p: Payload| out.push((name, p));

    // Every fixture kind.
    for (name, p) in all_payloads() {
        add(format!("fixture-{name}"), p);
    }

    // Every published code on every (message kind, business case) it answers.
    let typen = [
        Meldungstyp::Teq,
        Meldungstyp::RuemVa,
        Meldungstyp::Abbm,
        Meldungstyp::Erlm,
        Meldungstyp::AkmTr,
        Meldungstyp::AbbmTr,
    ];
    let faelle = [
        Geschaeftsfall::VaKueMrn,
        Geschaeftsfall::VaKueOrn,
        Geschaeftsfall::VaRrnp,
        Geschaeftsfall::StrAuf,
        Geschaeftsfall::StrAen,
        Geschaeftsfall::TvsVa,
    ];
    for code in Meldungscode::ALL {
        for typ in typen {
            for fall in faelle {
                if code.applies(typ, fall) {
                    add(format!("code-{}-{typ}-{fall}", code.as_str()), coded(typ, fall, code));
                }
            }
        }
    }

    // The schema's maxima.
    let mut abbm = abbm();
    abbm.positionen = (0..99)
        .map(|i| PositionAbbm {
            position: Position::new(Meldungscode::Rng),
            rufnummern: (0..10).map(|j| Rufnummer::new(format!("12345{i:02}{j}")).unwrap()).collect(),
        })
        .collect();
    add("max-abbm-99-positions-10-numbers".into(), msg(abbm));
    let mut va = va_kue_mrn();
    va.weitere_anschlussinhaber = (0..99).map(|_| kunde()).collect();
    if let VorabstimmungsFall::KueMrn { rufnummernportierung, .. } = &mut va.fall {
        rufnummernportierung.anschluss = Anschluss::Einzel {
            rufnummern: (0..10).map(|i| nummer("30", &format!("123456{i}"))).collect(),
            alle_rufnummern: false,
        };
    }
    add("max-va-99-holders-10-numbers".into(), req(va));
    let mut rrnp = va_rrnp();
    if let VorabstimmungsFall::Rrnp { rufnummernportierung, .. } = &mut rrnp.fall
        && let Anschluss::Anlage { bloecke, .. } = &mut rufnummernportierung.anschluss
    {
        *bloecke = (0..5)
            .map(|i| Rufnummernblock { von: format!("{i}0").parse().unwrap(), bis: format!("{i}9").parse().unwrap() })
            .collect();
    }
    add("max-rrnp-5-blocks".into(), req(rrnp));
    let mut ruem = ruem_va();
    ruem.ressourcen = (0..99)
        .map(|i| TechnischeRessource {
            line_id: Some(format!("DEU.EKP2.{i:010}").parse().unwrap()),
            ..Default::default()
        })
        .collect();
    ruem.rufnummernportierung = Some(PortierteRufnummern::Rufnummern(
        (0..10).map(|i| (nummer("30", &format!("765432{i}")), "D123".parse().unwrap())).collect(),
    ));
    add("max-ruem-va-99-resources-10-numbers".into(), msg(ruem));
    let mut akm = akm_tr();
    akm.uebernahme_leitung =
        (0..99).map(|i| UebernahmeLeitung::Vertragsnummer(format!("V{i:09}").parse().unwrap())).collect();
    add("max-akm-tr-99-lines".into(), msg(akm));

    // Texts at maximum length with umlauts and markup characters.
    let mut va = va_kue_mrn();
    va.endkunde = Kunde::person(
        Anrede::Keine,
        Some(Vorname::new(long("Ä<&>\"'ßü ", 30)).unwrap()),
        Nachname::new(long("Öß&<>'\"Ñ", 30)).unwrap(),
    );
    if let VorabstimmungsFall::KueMrn { standort, .. } = &mut va.fall {
        standort.strassenname = Strassenname::new(long("Straße §°±²³-", 40)).unwrap();
        standort.ort = Ortsname::new(long("Frankfurt am Main (Oder) ", 40)).unwrap();
        standort.hausnummern_zusatz = Some(HausnummernZusatz::new("a-<&>b").unwrap());
    }
    add("text-max-person".into(), req(va));
    let mut s = str_auf_abg();
    if let StornoArt::AufhebungAbg { stornogrund, .. } = &mut s.art {
        *stornogrund = Freitext::new(long("Zeile <1> & \"2\"\r\n\tEinrückung ", 255)).unwrap();
    }
    add("text-max-freitext-multiline".into(), req(s));

    // The answers the losing provider's check produces.
    let mut record = Bestand::new(kunde(), "D123".parse().unwrap());
    record.standort = Some(standort());
    record.rufnummern = vec![nummer("30", "1234567"), nummer("30", "1234568")];
    record.vertragsende = Some(date("2027-02-01"));
    for (i, (vor, nach, plz, number)) in [
        ("Erika", "Mustermann", "10115", "1234567"),
        ("Erika", "Mustermann", "10117", "1234567"),
        ("Lisa", "Mustermann", "10115", "1234567"),
        ("Max", "Schmidt", "10117", "5550000"),
        ("Erika", "Mustermann", "10115", "5550000"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut va = va_kue_mrn();
        va.endkunde = Kunde::person(Anrede::Frau, Some(Vorname::new(vor).unwrap()), Nachname::new(nach).unwrap());
        if let VorabstimmungsFall::KueMrn { standort, rufnummernportierung } = &mut va.fall {
            standort.postleitzahl = plz.parse().unwrap();
            rufnummernportierung.anschluss =
                Anschluss::Einzel { rufnummern: vec![nummer("30", number)], alle_rufnummern: i % 2 == 0 };
        }
        let p = pruefung::check(&va, &record, ts("2026-11-23T08:00:00Z"), &Calendar::wbci());
        add(format!("pruefung-{i}"), msg(p.antwort));
    }
    add("pruefung-not-found".into(), msg(pruefung::not_found(&va_kue_mrn()).antwort));

    // Deterministic variants across optional elements and choices.
    for i in 0..60u64 {
        let mut va = if i % 3 == 0 {
            va_kue_mrn()
        } else if i % 3 == 1 {
            va_kue_orn()
        } else {
            va_rrnp()
        };
        va.id = VorabstimmungsId::allocate(&cc("DEU.EKP1"), 1000 + i).unwrap();
        va.kundenwunschtermin =
            date("2027-01-04").checked_add(jiff::Span::new().days(i64::try_from(i).unwrap())).unwrap();
        va.projekt = (i % 4 == 0).then(|| ProjektId {
            projektkenner: Projektkenner::new(format!("P{i}")).unwrap(),
            kopplungskenner: (i % 8 == 0).then(|| Kopplungskenner::new(format!("K{i}")).unwrap()),
        });
        if i % 5 == 0 {
            va.endkunde = Kunde::Firma(Firma {
                anrede: if i % 10 == 0 { AnredeFirma::Firma } else { AnredeFirma::Keine },
                name: Firmenname::new(format!("Firma {i} GmbH")).unwrap(),
                name_zweiter_teil: (i % 10 == 0).then(|| Nachname::new("& Co. KG").unwrap()),
            });
        }
        let zf = [Portierungszeitfenster::Zf1, Portierungszeitfenster::Zf2, Portierungszeitfenster::Zf3]
            [usize::try_from(i % 3).unwrap()];
        if i % 2 == 0 {
            match &mut va.fall {
                VorabstimmungsFall::KueMrn { rufnummernportierung, .. }
                | VorabstimmungsFall::Rrnp { rufnummernportierung, .. } => {
                    rufnummernportierung.zeitfenster = zf;
                }
                VorabstimmungsFall::KueOrn { anschlussidentifikation, .. } => *anschlussidentifikation = None,
            }
        }
        let bezug = if i % 2 == 0 { Bezug::Id(va.id.clone()) } else { Bezug::Originalanfrage(Box::new(va.clone())) };
        add(format!("variant-{i:02}-va"), req(va.clone()));
        let mut t = tvs();
        t.bezug = bezug.clone();
        t.name = (i % 3 == 0).then(kunde);
        add(format!("variant-{i:02}-tvs"), req(t));
        let mut s =
            [str_auf_auf(), str_auf_abg(), str_aen_auf(), str_aen_abg()][usize::try_from(i % 4).unwrap()].clone();
        s.bezug = bezug;
        add(format!("variant-{i:02}-storno"), req(s));
        let mut ctl = c();
        ctl.zeitstempel = Zeitstempel {
            timestamp: ts("2026-11-23T09:30:00Z"),
            offset: jiff::tz::Offset::from_hours(i8::try_from(i % 15).unwrap() - 7).unwrap(),
        };
        ctl.release.minor = u8::try_from(i % 3).unwrap();
        add(format!("variant-{i:02}-teq"), Payload::MeldungResponse { control: ctl, quittung: teq_ok() });
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
    eprintln!("corpus: {} payloads valid against WBCI XSD V2.00 — {counts:?}", corpus.len());
}

/// The validator must reject what the schema forbids, or the corpus test proves nothing.
#[test]
fn validator_rejects_a_reordered_payload() {
    let Some(schema) = schema_dir() else { return };
    let xml = req(va_kue_mrn()).to_xml().unwrap();
    let termin = "    <Kundenwunschtermin>2027-01-15</Kundenwunschtermin>\n";
    let reordered = xml.replacen(termin, "", 1).replacen(
        "    <Standort>",
        &format!("{}    <Standort>", termin.trim_start_matches(' ').replacen('<', "    <", 1)),
        1,
    );
    assert_ne!(reordered, xml);
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("reordered.xml");
    std::fs::write(&path, reordered).unwrap();
    let failures = xmllint(&schema, &[path]);
    assert!(failures.iter().any(|l| l.contains("Kundenwunschtermin")), "{failures:?}");
}

fn fetch(zip: &Path, dir: &Path) -> std::process::Output {
    Command::new("bash")
        .arg(workspace().join("scripts/fetch-wbci-xsd.sh"))
        .arg(dir)
        .env("WBCI_XSD_ZIP", zip)
        .output()
        .expect("bash runs")
}

// schema-conformance/SR-001: a zip with another hash is refused before anything is extracted.
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

// schema-conformance/SR-002: only the known files, by name, in a fresh folder.
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
            "wsdl/policy.xml",
            "wsdl/wbci-envelope.xsd",
            "wsdl/wbci.wsdl",
            "xsd/wbci-message.xsd",
            "xsd/wbci-request.xsd",
            "xsd/wbci-type-complex.xsd",
            "xsd/wbci-type-enum.xsd",
            "xsd/wbci-type-simple.xsd",
        ]
    );
}
