//! Validation over the fixtures, against `Process`, and on randomised values.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use spri::*;
use spri::{Calendar, Date, Timestamp};

fn issues(p: &Payload) -> Vec<Issue> {
    match p {
        Payload::AuftragRequest { auftrag, .. } => auftrag.issues(),
        Payload::MeldungRequest { meldung, .. } => meldung.issues(),
        Payload::AuftragResponse { quittung, .. } | Payload::MeldungResponse { quittung, .. } => quittung.issues(),
    }
}

#[test]
fn every_fixture_has_no_error() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut n = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let p = Payload::from_xml(&std::fs::read(&path).unwrap()).unwrap();
        let errors: Vec<String> =
            issues(&p).iter().filter(|i| i.severity == Severity::Error).map(ToString::to_string).collect();
        assert!(errors.is_empty(), "{}: {errors:#?}", path.display());
        n += 1;
    }
    assert_eq!(n, 30);
}

// -- consistency with Process ---------------------------------------------------------------

use spri::process::{Direction, Process, Role};

fn ts(s: &str) -> Timestamp {
    s.parse().unwrap()
}

/// The service provider's process of `auftrag`, delivered and accepted (TEQ OK, QEB).
fn accepted(auftrag: &Auftrag) -> Process {
    let mut p = Process::new(Role::Auftraggeber, auftrag, Direction::Sent, ts("2026-11-23T08:00:00Z")).unwrap();
    p.apply(Direction::Received, ts("2026-11-23T08:00:01Z"), common::teq_ok()).unwrap();
    let qeb = common::meldung(auftrag.geschaeftsfall.code(), auftrag.geschaeftsfall_art, common::qeb());
    p.apply(Direction::Received, ts("2026-11-23T09:00:00Z"), qeb).unwrap();
    p
}

fn code_of(issues: &[Issue], rule: &str) -> Option<Meldungscode> {
    issues.iter().find(|i| i.rule == rule && i.severity == Severity::Error).and_then(|i| i.code.clone())
}

// spri-validation/SR-004: a rule Process also enforces gives the same verdict and code.
#[test]
fn validation_and_process_agree() {
    let at = ts("2026-11-24T08:00:00Z");
    for (base, aekz, rule) in [
        (common::aen_lmae(), Aenderungskennzeichen::Terminverschiebung, "spri.aekz.terminverschiebung"),
        (common::get(), Aenderungskennzeichen::Storno, "spri.aekz.storno"),
        (common::set(), Aenderungskennzeichen::Storno, "spri.aekz.storno"),
        (common::est(), Aenderungskennzeichen::Terminverschiebung, "spri.aekz.terminverschiebung"),
    ] {
        let follow = Auftrag { aenderungskennzeichen: aekz, ..base.clone() };
        let mut p = Process::new(Role::Auftraggeber, &base, Direction::Sent, ts("2026-11-23T08:00:00Z")).unwrap();
        let refusal = p.apply(Direction::Sent, at, follow.clone()).unwrap_err();
        let issues = follow.issues_against(&base);
        assert_eq!(refusal.answer, Some(Meldungscode::C1007), "{rule}");
        assert_eq!(code_of(&issues, rule), refusal.answer, "{rule}: {issues:#?}");
    }
    // A Terminverschiebung with the unchanged date; a PV moved earlier.
    let pv = common::pv();
    for (termin, rule) in [(None, "spri.tv.new-date"), (Some("2027-01-08"), "spri.tv.earlier")] {
        let mut tv = Auftrag { aenderungskennzeichen: Aenderungskennzeichen::Terminverschiebung, ..pv.clone() };
        tv.termin_neu = termin.map(|d| d.parse().unwrap());
        let refusal = accepted(&pv).apply(Direction::Sent, at, tv.clone()).unwrap_err();
        assert_eq!(refusal.answer, Some(Meldungscode::C1085), "{rule}");
        assert_eq!(code_of(&tv.issues_against(&pv), rule), refusal.answer, "{rule}");
    }
    // Where validation finds nothing, Process accepts.
    let mut tv = Auftrag { aenderungskennzeichen: Aenderungskennzeichen::Terminverschiebung, ..pv.clone() };
    tv.termin_neu = Some("2027-01-22".parse().unwrap());
    assert!(tv.issues_against(&pv).is_empty());
    accepted(&pv).apply(Direction::Sent, at, tv).unwrap();
}

// spri-validation/SR-004: the refusal reasons mean the same in both — `LnB` (handbook) and
// `LNB` (EPK) refuse.
#[test]
fn refusal_reasons_agree_with_process() {
    for code in ["LnB", "LNB", "kve", "TAV", "VnB", "Vnv"] {
        let mut r = common::ruem_pv();
        r.abgebender_provider.antwortcode = Some(Antwortcode::new(code).unwrap());
        let m = common::meldung(GeschaeftsfallCode::Pv, GeschaeftsfallArt::Endkundenanbieterwechsel, r);
        // Consent with a refusal reason is a contradiction for validation …
        assert!(m.issues().iter().any(|i| i.rule == "spri.ruem-pv.contradiction"), "{code}");
        // … and a refusal for the network owner's process.
        let akm =
            common::meldung(GeschaeftsfallCode::Pv, GeschaeftsfallArt::Endkundenanbieterwechsel, common::akm_pv());
        let mut p = Process::from_akm_pv(&akm, ts("2026-11-23T08:00:00Z")).unwrap();
        p.apply(Direction::Sent, ts("2026-11-23T08:00:01Z"), common::teq_ok()).unwrap();
        p.apply(Direction::Sent, ts("2026-11-24T08:00:00Z"), m).unwrap();
        assert!(matches!(p.pv(), Some(spri::process::PvLeg::Refused { .. })), "{code}: {:?}", p.pv());
    }
}

// spri-validation/SR-004: every rule cites a document, version and clause.
#[test]
fn every_issue_cites_its_source() {
    let docs = include_str!("../src/validate.rs");
    let page = include_str!("../../../site/content/docs/spri/validation.md");
    for rule in docs.lines().filter_map(|l| l.strip_prefix("//! | `spri.")).filter_map(|l| l.split('`').next()) {
        assert!(page.contains(&format!("`spri.{rule}`")), "spri.{rule} missing from the site page");
    }
    let mut all = Vec::new();
    for (_, a) in common::orders() {
        for aekz in [Aenderungskennzeichen::Storno, Aenderungskennzeichen::Terminverschiebung] {
            let f = Auftrag { aenderungskennzeichen: aekz, termin_neu: None, ..a.clone() };
            all.extend(f.issues_against(&common::pv()));
            all.extend(f.issues_at(ts("2028-01-01T00:00:00Z"), &Calendar::werktage()));
        }
    }
    assert!(all.len() > 10);
    for i in all {
        let s = i.source;
        let version =
            ["S/PRI 4.4", "AH 2.9.5", "XSD 4.00.03", "LineID V1.4", "Anl. 7 EPK V.13"].iter().any(|v| s.contains(v));
        assert!(version && (s.contains('§') || s.contains("XSD")), "{i}");
    }
}

// -- property: no panic, no personal data ---------------------------------------------------

const SECRETS: [&str; 14] = [
    "Erika",
    "Mustermann",
    "Max",
    "Beispiel",
    "Vormann",
    "Hauptstraße",
    "10115",
    "1234567",
    "7654321",
    "9876543",
    "example.",
    "Kein Sync",
    "Störung besteht",
    "Alter Anbieter",
];

fn set_positions(m: &mut Meldungsinhalt, ps: Vec<Position>) {
    let w = |ps: Vec<Position>| ps.into_iter().map(PositionWita::from).collect::<Vec<_>>();
    match m {
        Meldungsinhalt::Qeb(x) => x.positionen = ps.into_iter().map(Into::into).collect(),
        Meldungsinhalt::Abm(x) => x.positionen = ps.into_iter().map(Into::into).collect(),
        Meldungsinhalt::Abbm(x) => x.positionen = w(ps),
        Meldungsinhalt::Erlm(x) => x.positionen = w(ps),
        Meldungsinhalt::Vzm(x) => {
            x.verzoegerungstermin = None;
            x.positionen = w(ps);
        }
        Meldungsinhalt::Tam(x) | Meldungsinhalt::Mtam(x) => x.positionen = w(ps),
        Meldungsinhalt::Entm(x) => x.positionen = ps,
        Meldungsinhalt::Kdle(x) => x.positionen = ps,
        Meldungsinhalt::ZwmLe(x) => x.positionen = ps,
        Meldungsinhalt::ZwmAg(x) => x.positionen = ps,
        Meldungsinhalt::TbkLe(x) | Meldungsinhalt::TbkAg(x) => x.positionen = ps,
        Meldungsinhalt::AkmPv(x) => x.positionen = ps,
        Meldungsinhalt::RuemPv(x) => x.positionen = ps,
        Meldungsinhalt::AbmPv(x) => x.positionen = ps,
        Meldungsinhalt::ErlmPv(x) => x.positionen = ps,
        Meldungsinhalt::EntmPv(x) => x.positionen = ps,
        Meldungsinhalt::AbbmPv(x) => x.positionen = ps,
        Meldungsinhalt::ErlmK(x) => x.positionen = ps,
    }
}

#[track_caller]
fn assert_clean(issues: &[Issue]) {
    for i in issues {
        let text = format!("{i} {} {} {i:?}", i.path, i.message);
        for secret in SECRETS {
            assert!(!text.contains(secret), "{secret} in {text}");
        }
    }
}

mod property {
    use proptest::prelude::*;

    use super::*;

    fn aekz() -> impl Strategy<Value = Aenderungskennzeichen> {
        prop_oneof![
            Just(Aenderungskennzeichen::Standard),
            Just(Aenderungskennzeichen::Storno),
            Just(Aenderungskennzeichen::Terminverschiebung),
        ]
    }

    fn day() -> impl Strategy<Value = Date> {
        (-9999i16..=9999, 1i8..=12, 1i8..=28).prop_map(|(y, m, d)| Date::new(y, m, d).unwrap())
    }

    fn instant() -> impl Strategy<Value = Timestamp> {
        prop_oneof![
            Just(Timestamp::MIN),
            Just(Timestamp::MAX),
            (Timestamp::MIN.as_second()..=Timestamp::MAX.as_second()).prop_map(|s| Timestamp::from_second(s).unwrap()),
        ]
    }

    fn code() -> impl Strategy<Value = Meldungscode> {
        prop_oneof![
            proptest::sample::select(Meldungscode::ALL.to_vec()),
            "[\\x00-\\x7f]{1,10}".prop_map(|s| Meldungscode::parse(&s)),
        ]
    }

    fn zeitfenster() -> impl Strategy<Value = Option<Zeitfenster>> {
        proptest::option::of(
            prop_oneof![(1u8..=99).prop_map(|n| n.to_string()), "[A-Z]"].prop_map(|s| Zeitfenster::new(&s).unwrap()),
        )
    }

    fn with_date(a: &mut Auftrag, datum: Date, z: Option<Zeitfenster>) {
        let t = Wunschtermin { datum, zeitfenster: z };
        match &mut a.geschaeftsfall {
            Geschaeftsfall::Neu(x) => x.wunschtermin = t,
            Geschaeftsfall::KueAg(x) => x.wunschtermin = t,
            Geschaeftsfall::Lae(x) => x.wunschtermin = t,
            Geschaeftsfall::AenLmae(x) => x.wunschtermin = t,
            Geschaeftsfall::Est(x) => x.wunschtermin = t,
            Geschaeftsfall::Pv(x) => x.wunschtermin = t,
            Geschaeftsfall::Get(_) | Geschaeftsfall::Set(_) => {}
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(512))]

        // spri-validation/SR-001, spri-validation/SR-002: randomised orders never panic and
        // their issues carry no personal value of the fixtures.
        #[test]
        fn orders(
            i in 0usize..8, j in 0usize..8, aekz in aekz(), datum in day(), z in zeitfenster(),
            neu in proptest::option::of(day()), anzahl in any::<i32>(), now in instant(),
        ) {
            let orders = common::orders();
            let (mut a, original) = (orders[i].1.clone(), orders[j].1.clone());
            a.aenderungskennzeichen = aekz;
            a.termin_neu = neu;
            with_date(&mut a, datum, z);
            if let Some(k) = a.kenner.as_mut().and_then(|k| k.auftragskenner.as_mut()) {
                k.anzahl_auftraege = anzahl;
            }
            let cal = Calendar::werktage();
            assert_clean(&a.issues());
            assert_clean(&a.issues_at(now, &cal));
            assert_clean(&a.issues_against(&original));
            assert_clean(&original.issues_against(&a));
        }

        // spri-validation/SR-001, spri-validation/SR-002: randomised messages and receipts.
        #[test]
        fn messages(
            i in 0usize..20, codes in proptest::collection::vec(code(), 0..6), aekz in aekz(),
            fall in proptest::sample::select(GeschaeftsfallCode::ALL.to_vec()),
            antwort in proptest::option::of("[A-Za-z0-9]{1,10}"), nein in any::<bool>(),
        ) {
            let (_, mut m) = common::messages()[i].clone();
            m.auftragstyp.aenderungskennzeichen = aekz;
            m.auftragstyp.geschaeftsfall = fall;
            let ps: Vec<Position> = codes.iter().cloned().map(Position::new).collect();
            if let Meldungsinhalt::RuemPv(r) = &mut m.inhalt {
                r.abgebender_provider.zustimmung = if nein { JaNein::Nein } else { JaNein::Ja };
                r.abgebender_provider.antwortcode = antwort.map(|a| Antwortcode::new(a).unwrap());
            }
            if let Meldungsinhalt::Abbm(a) = &mut m.inhalt {
                a.wiedervorlagetermin = None;
            }
            set_positions(&mut m.inhalt, ps.clone());
            assert_clean(&m.issues());
            let teq = Teq { positionen: ps, ..common::teq_ok() };
            assert_clean(&teq.issues());
        }
    }
}
