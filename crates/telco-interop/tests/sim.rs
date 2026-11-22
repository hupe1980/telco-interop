//! The scenario catalogue: every scenario's sequence, its validation, its acceptance by every
//! party's process with no obligation overdue, the cross-interface switch, determinism, golden
//! files, and start times across a year.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::fs;
use std::path::{Path, PathBuf};

use telco_interop::sim::{self, Ablauf, Inhalt, Nachricht};
use telco_interop::{Partei, Wechsel, spri, wbci};

const EXPECTED: &[(&str, &[&str])] = &[
    ("wbci-agreed", &["VA-KUE-MRN", "RUEM-VA", "AKM-TR"]),
    ("wbci-refused-rng", &["VA-KUE-MRN", "ABBM"]),
    ("wbci-storno", &["VA-KUE-ORN", "RUEM-VA", "STR-AUF", "ERLM"]),
    ("wbci-tvs", &["VA-KUE-ORN", "RUEM-VA", "TVS-VA", "ERLM"]),
    ("spri-neu", &["NEU", "QEB", "ABM", "ERLM", "ENTM"]),
    ("spri-neu-tam", &["NEU", "QEB", "ABM", "TAM", "MTAM", "ABBM"]),
    ("spri-storno", &["NEU", "QEB", "ABM", "Storno", "ERLM", "ENTM"]),
    ("spri-storno-refused", &["NEU", "QEB", "ABM", "Storno", "ABBM", "ERLM", "ENTM"]),
    ("spri-tv", &["NEU", "QEB", "ABM", "Terminverschiebung", "ABM", "ERLM", "ENTM"]),
    ("spri-pv", &["PV", "QEB", "AKM-PV", "RUEM-PV", "ABM", "ABM-PV", "ERLM", "ERLM-PV", "ENTM", "ENTM-PV"]),
    ("spri-pv-refused", &["PV", "QEB", "AKM-PV", "RUEM-PV", "ABBM", "ABBM-PV"]),
    ("spri-pv-timeout", &["PV", "QEB", "AKM-PV", "ABBM", "ABBM-PV"]),
    (
        "wechsel",
        &[
            "VA-KUE-MRN",
            "RUEM-VA",
            "AKM-TR",
            "PV",
            "QEB",
            "AKM-PV",
            "RUEM-PV",
            "ABM",
            "ABM-PV",
            "ERLM",
            "ERLM-PV",
            "ENTM",
            "ENTM-PV",
        ],
    ),
];

fn kinds(a: &Ablauf) -> Vec<&'static str> {
    a.nachrichten.iter().map(Nachricht::art).filter(|k| *k != "TEQ").collect()
}

#[test]
fn the_catalogue_has_the_named_scenarios() {
    // sandbox/SR-004
    let names: Vec<&str> = sim::katalog().iter().map(|s| s.name).collect();
    assert!(names.len() >= 13);
    assert_eq!(names, EXPECTED.iter().map(|e| e.0).collect::<Vec<_>>());
    for s in sim::katalog() {
        assert!(!s.beschreibung.is_empty() && !s.quelle.is_empty(), "{}", s.name);
        assert!(s.quelle.contains('§') || s.quelle.contains("EPK"), "{} cites a clause or page", s.name);
    }
}

#[test]
fn every_scenario_produces_its_sequence() {
    for (name, expected) in EXPECTED {
        let a = sim::szenario(name).unwrap().play(sim::START);
        assert!(a.abgelehnt.is_empty(), "{name}: {:?}", a.abgelehnt);
        assert_eq!(kinds(&a), *expected, "{name}");
        // Every business message is acknowledged.
        let teqs = a.nachrichten.iter().filter(|n| n.art() == "TEQ").count();
        assert_eq!(teqs, expected.len(), "{name}: one receipt per message");
    }
}

/// The request a WBCI message answers, from the scenario.
fn answering(a: &Ablauf, m: &wbci::Meldung) -> Option<wbci::Anfrage> {
    a.nachrichten.iter().rev().find_map(|n| match &n.inhalt {
        Inhalt::Wbci(p) => match &**p {
            wbci::Payload::AnfrageRequest { anfrage, .. } => {
                let fits = match anfrage {
                    wbci::Anfrage::Storno(s) => m.storno_id() == Some(&s.storno_id),
                    wbci::Anfrage::Terminverschiebung(t) => m.aenderungs_id() == Some(&t.aenderungs_id),
                    wbci::Anfrage::Vorabstimmung(_) => m.storno_id().is_none() && m.aenderungs_id().is_none(),
                };
                fits.then(|| anfrage.clone())
            }
            _ => None,
        },
        Inhalt::Spri(_) => None,
    })
}

/// Every message validates without any issue, at its own time.
fn validate(name: &str, a: &Ablauf) {
    let wcal = wbci::Calendar::wbci();
    let scal = spri::Calendar::werktage();
    let mut original: Option<spri::Auftrag> = None;
    for n in &a.nachrichten {
        let issues: Vec<String> =
            match &n.inhalt {
                Inhalt::Wbci(p) => match &**p {
                    wbci::Payload::AnfrageRequest { anfrage, .. } => anfrage.issues_at(n.at, &wcal),
                    wbci::Payload::MeldungRequest { meldung, .. } => {
                        meldung.issues_at(answering(a, meldung).as_ref(), n.at, &wcal)
                    }
                    wbci::Payload::AnfrageResponse { quittung, .. }
                    | wbci::Payload::MeldungResponse { quittung, .. } => quittung.issues(),
                }
                .iter()
                .map(ToString::to_string)
                .collect(),
                Inhalt::Spri(p) => match &**p {
                    spri::Payload::AuftragRequest { auftrag, .. } => {
                        let mut i = auftrag.issues_at(n.at, &scal);
                        match &original {
                            Some(o) => i.extend(auftrag.issues_against(o)),
                            None => original = Some(auftrag.clone()),
                        }
                        i
                    }
                    spri::Payload::MeldungRequest { meldung, .. } => meldung.issues(),
                    spri::Payload::AuftragResponse { quittung, .. }
                    | spri::Payload::MeldungResponse { quittung, .. } => quittung.issues(),
                }
                .iter()
                .map(ToString::to_string)
                .collect(),
            };
        assert!(issues.is_empty(), "{name}: {n}: {issues:#?}");
    }
}

const fn wrole(p: Partei) -> wbci::Role {
    match p {
        Partei::EkpAbg => wbci::Role::EkpAbg,
        _ => wbci::Role::EkpAuf,
    }
}

const fn srole(p: Partei) -> spri::process::Role {
    match p {
        Partei::EkpAuf => spri::process::Role::Auftraggeber,
        Partei::EkpAbg => spri::process::Role::AbgebenderProvider,
        Partei::Netzbetreiber => spri::process::Role::Leistungserbringer,
    }
}

/// The processes of every party, fed the scenario in time order.
#[derive(Default)]
struct Parties {
    wbci: Vec<wbci::Process>,
    netz: Option<spri::process::Process>,
    auf: Option<spri::process::Process>,
    abg: Option<spri::process::Process>,
}

/// Replays `a` into every party's process; before each message no obligation is overdue —
/// except the RUEM-PV of a losing provider who stays silent by design (the network owner's
/// ABBM 0042 answers exactly that).
fn replay(name: &str, a: &Ablauf) -> Parties {
    use spri::process::{Direction as D, Process as SP, Role as R};
    let silent = a.nachrichten.iter().any(|n| n.art() == "ABBM" && n.codes() == ["0042"]);
    let mut ps = Parties::default();
    for n in &a.nachrichten {
        match &n.inhalt {
            Inhalt::Wbci(p) => {
                let message = match (**p).clone() {
                    wbci::Payload::AnfrageRequest { anfrage, .. } => wbci::process::Message::from(anfrage),
                    wbci::Payload::MeldungRequest { meldung, .. } => wbci::process::Message::from(meldung),
                    wbci::Payload::AnfrageResponse { quittung, .. }
                    | wbci::Payload::MeldungResponse { quittung, .. } => wbci::process::Message::from(quittung),
                };
                if ps.wbci.is_empty() {
                    let wbci::process::Message::Anfrage(wbci::Anfrage::Vorabstimmung(va)) = &message else {
                        panic!("{name}: WBCI starts with the VA")
                    };
                    ps.wbci = vec![
                        wbci::Process::new(wbci::Role::EkpAuf, va.clone(), wbci::Direction::Sent, n.at).unwrap(),
                        wbci::Process::new(wbci::Role::EkpAbg, va.clone(), wbci::Direction::Received, n.at).unwrap(),
                    ];
                    continue;
                }
                for p in &mut ps.wbci {
                    let overdue = p.overdue(n.at);
                    assert!(overdue.is_empty(), "{name}: {n}: overdue for {}: {overdue:?}", p.role());
                    let d = if wrole(n.von) == p.role() { wbci::Direction::Sent } else { wbci::Direction::Received };
                    p.apply(d, n.at, message.clone()).unwrap_or_else(|e| panic!("{name}: {n} as {}: {e}", p.role()));
                }
            }
            Inhalt::Spri(p) => {
                let message = match (**p).clone() {
                    spri::Payload::AuftragRequest { auftrag, .. } => spri::process::Message::from(auftrag),
                    spri::Payload::MeldungRequest { meldung, .. } => spri::process::Message::from(meldung),
                    spri::Payload::AuftragResponse { quittung, .. }
                    | spri::Payload::MeldungResponse { quittung, .. } => spri::process::Message::from(quittung),
                };
                if ps.netz.is_none() {
                    let spri::process::Message::Auftrag(order) = &message else {
                        panic!("{name}: S/PRI starts with the order")
                    };
                    ps.netz = Some(SP::new(R::Leistungserbringer, order, D::Received, n.at).unwrap());
                    ps.auf = Some(SP::new(R::Auftraggeber, order, D::Sent, n.at).unwrap());
                    continue;
                }
                let (from, to) = (srole(n.von), srole(n.an));
                if ps.abg.is_none()
                    && let spri::process::Message::Meldung(m) = &message
                    && m.typ() == spri::Meldungstyp::AkmPv
                {
                    ps.abg = Some(SP::from_akm_pv(m, n.at).unwrap());
                    apply(name, n, ps.netz.as_mut().unwrap(), D::Sent, &message, silent);
                    continue;
                }
                for p in [ps.netz.as_mut(), ps.auf.as_mut(), ps.abg.as_mut()].into_iter().flatten() {
                    let role = p.role();
                    if role != from && role != to {
                        continue;
                    }
                    let d = if role == from { D::Sent } else { D::Received };
                    apply(name, n, p, d, &message, silent);
                }
            }
        }
    }
    ps
}

fn apply(
    name: &str,
    n: &Nachricht,
    p: &mut spri::process::Process,
    d: spri::process::Direction,
    m: &spri::process::Message,
    silent: bool,
) {
    let overdue: Vec<_> =
        p.overdue(n.at).into_iter().filter(|f| !(silent && f.kind == spri::deadline::DeadlineKind::RuemPv)).collect();
    assert!(overdue.is_empty(), "{name}: {n}: overdue for {}: {overdue:?}", p.role());
    p.apply(d, n.at, m.clone()).unwrap_or_else(|e| panic!("{name}: {n} as {}: {e}", p.role()));
}

fn check(name: &str, a: &Ablauf) {
    assert!(a.abgelehnt.is_empty(), "{name}: {:?}", a.abgelehnt);
    validate(name, a);
    let ps = replay(name, a);
    // Every receipt arrived: nothing is left awaiting one.
    for p in &ps.wbci {
        assert!(!p.next_actions(p.role()).contains(&wbci::process::Action::Acknowledge), "{name}: {}", p.role());
    }
    if name == "wechsel" {
        let mut w = Wechsel::new(ps.wbci.into_iter().next().unwrap());
        w.attach(ps.auf.unwrap()).unwrap();
        assert!(w.befunde().is_empty(), "{:?}", w.befunde());
    }
}

#[test]
fn every_scenario_validates_and_every_party_accepts_it_in_time() {
    // sandbox/SR-004
    for s in sim::katalog() {
        check(s.name, &s.play(sim::START));
    }
}

#[test]
fn start_times_across_a_year_keep_every_rule() {
    // sandbox/SR-004: weekends, public holidays, Christmas and the turn of the year.
    let first: wbci::Timestamp = "2027-01-01T16:30:00Z".parse().unwrap();
    for day in (0..366).step_by(3) {
        let start = first + wbci::sim::SignedDuration::from_hours(24 * day);
        for s in sim::katalog() {
            check(&format!("{} from {start}", s.name), &s.play(start));
        }
    }
    for start in ["2027-12-23T22:59:00Z", "2027-12-31T12:00:00Z", "2027-03-26T15:00:00Z", "2027-04-30T07:00:00Z"] {
        let start: wbci::Timestamp = start.parse().unwrap();
        for s in sim::katalog() {
            check(&format!("{} from {start}", s.name), &s.play(start));
        }
    }
}

#[test]
fn the_same_start_gives_the_same_bytes() {
    // sandbox/SR-005
    for s in sim::katalog() {
        assert_eq!(s.play(sim::START).files().unwrap(), s.play(sim::START).files().unwrap(), "{}", s.name);
    }
}

#[test]
fn every_payload_reads_back_equal() {
    for s in sim::katalog() {
        for n in &s.play(sim::START).nachrichten {
            let xml = n.to_xml().unwrap();
            match &n.inhalt {
                Inhalt::Wbci(p) => assert_eq!(wbci::Payload::from_xml(xml.as_bytes()).unwrap(), **p, "{n}"),
                Inhalt::Spri(p) => assert_eq!(spri::Payload::from_xml(xml.as_bytes()).unwrap(), **p, "{n}"),
            }
        }
    }
}

#[test]
fn file_names_follow_the_time_order() {
    let a = sim::szenario("wechsel").unwrap().play(sim::START);
    let files = a.files().unwrap();
    assert_eq!(files[0].0, "001-wbci-ekpauf-va-kue-mrn.xml");
    assert_eq!(files[1].0, "002-wbci-ekpabg-teq.xml");
    assert!(files.iter().any(|f| f.0.ends_with("-spri-netzbetreiber-abm-pv.xml")));
    let mut sorted = files.clone();
    sorted.sort();
    assert_eq!(files, sorted);
    assert!(a.nachrichten.windows(2).all(|w| w[0].at <= w[1].at));
}

fn golden_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sim").join(name)
}

#[test]
fn the_files_equal_the_golden_fixtures() {
    // sandbox/SR-005: recorded, synthetic fixtures; `SIM_BLESS=1` rewrites them after an
    // intended change.
    let bless = std::env::var("SIM_BLESS").is_ok_and(|v| v == "1");
    for s in sim::katalog() {
        let dir = golden_dir(s.name);
        let files = s.play(sim::START).files().unwrap();
        if bless {
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            for (name, xml) in &files {
                fs::write(dir.join(name), xml).unwrap();
            }
            continue;
        }
        let mut on_disk: Vec<String> =
            fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        on_disk.sort();
        assert_eq!(on_disk, files.iter().map(|f| f.0.clone()).collect::<Vec<_>>(), "{}", s.name);
        for (name, xml) in &files {
            assert_eq!(&fs::read_to_string(dir.join(name)).unwrap(), xml, "{}/{name}", s.name);
        }
    }
}

#[test]
fn lines_carry_no_personal_data() {
    // sandbox/SR-003
    for s in sim::katalog() {
        let a = s.play(sim::START);
        let text = a.nachrichten.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n");
        for secret in ["Musterfrau", "Erika", "Musterweg", "12345", "Musterstadt", "5550100", "5550200"] {
            assert!(!text.contains(secret), "{secret} in {}", s.name);
        }
    }
}
