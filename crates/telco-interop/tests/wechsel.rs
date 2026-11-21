//! One switch across WBCI and S/PRI: the sequence of S/PRI 4.4 p. 27 as one timeline, one
//! test per cross-interface rule, the attach check and the value-free output.
#![cfg(all(feature = "wbci", feature = "spri"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::fmt::Write as _;

use telco_interop::spri::process::{Action as SAction, Direction as SDir, Process as SProcess, Role as SRole};
use telco_interop::wechsel::regeln;
use telco_interop::{Aktion, Befund, Partei, Schnittstelle, Wechsel, spri, wbci};

// -- fixtures -------------------------------------------------------------------------------

fn at(s: &str) -> wbci::Timestamp {
    format!("{s}:00+01:00").parse().unwrap()
}

fn date(s: &str) -> wbci::Date {
    s.parse().unwrap()
}

fn wbci_anfrage(xml: &str) -> wbci::Anfrage {
    match wbci::Payload::from_xml(xml.as_bytes()).unwrap() {
        wbci::Payload::AnfrageRequest { anfrage, .. } => anfrage,
        other => panic!("not a request: {other:?}"),
    }
}

fn wbci_meldung(xml: &str) -> wbci::Meldung {
    match wbci::Payload::from_xml(xml.as_bytes()).unwrap() {
        wbci::Payload::MeldungRequest { meldung, .. } => meldung,
        other => panic!("not a message: {other:?}"),
    }
}

fn va() -> wbci::Vorabstimmung {
    match wbci_anfrage(include_str!("../../wbci/tests/fixtures/va-kue-mrn.xml")) {
        wbci::Anfrage::Vorabstimmung(va) => va,
        other => panic!("not a pre-agreement: {other:?}"),
    }
}

fn ruem_va() -> wbci::Meldung {
    wbci_meldung(include_str!("../../wbci/tests/fixtures/ruem-va.xml"))
}

/// The AKM-TR, taking over `line`.
fn akm_tr(line: Option<&str>) -> wbci::Meldung {
    let mut m = wbci_meldung(include_str!("../../wbci/tests/fixtures/akm-tr.xml"));
    if let (wbci::Meldung::AkmTr(t), Some(line)) = (&mut m, line) {
        t.uebernahme_leitung = vec![wbci::UebernahmeLeitung::LineId(line.parse().unwrap())];
    }
    m
}

const PV_LINE: &str = "DEU.NB01.1754639821";

fn order(xml: &str) -> spri::Auftrag {
    match spri::Payload::from_xml(xml.as_bytes()).unwrap() {
        spri::Payload::AuftragRequest { auftrag, .. } => auftrag,
        other => panic!("not an order: {other:?}"),
    }
}

fn pv() -> spri::Auftrag {
    order(include_str!("../../spri/tests/fixtures/auftrag-pv.xml"))
}

fn pv_wish(order: &mut spri::Auftrag, d: &str) {
    if let spri::Geschaeftsfall::Pv(p) = &mut order.geschaeftsfall {
        p.wunschtermin.datum = date(d);
    }
}

/// A message fixture, as a message of business case PV.
fn sm(xml: &str) -> spri::Meldung {
    let mut m = match spri::Payload::from_xml(xml.as_bytes()).unwrap() {
        spri::Payload::MeldungRequest { meldung, .. } => meldung,
        other => panic!("not a message: {other:?}"),
    };
    m.auftragstyp.geschaeftsfall = spri::GeschaeftsfallCode::Pv;
    m
}

macro_rules! sm {
    ($name:literal) => {
        sm(include_str!(concat!("../../spri/tests/fixtures/", $name, ".xml")))
    };
}

fn abm(d: &str) -> spri::Meldung {
    let mut m = sm!("meldung-abm");
    if let spri::Meldungsinhalt::Abm(a) = &mut m.inhalt {
        a.verbindlicher_liefertermin.datum = date(d);
    }
    m
}

fn erlm(d: &str) -> spri::Meldung {
    let mut m = sm!("meldung-erlm");
    if let spri::Meldungsinhalt::Erlm(e) = &mut m.inhalt {
        e.erledigungstermin = date(d);
    }
    m
}

fn erlm_pv(d: &str) -> spri::Meldung {
    let mut m = sm!("meldung-erlm-pv");
    if let spri::Meldungsinhalt::ErlmPv(e) = &mut m.inhalt {
        e.erledigungstermin = date(d);
    }
    m
}

// -- the harness ----------------------------------------------------------------------------

/// The gaining provider's view of WBCI, with the pre-agreement sent.
fn requested() -> Wechsel {
    let p = wbci::Process::new(wbci::Role::EkpAuf, va(), wbci::Direction::Sent, at("2026-11-23T09:00")).unwrap();
    Wechsel::new(p)
}

/// … answered with the RUEM-VA for Fr 15.01.2027.
fn agreed() -> Wechsel {
    let mut w = requested();
    w.wbci_mut().apply(wbci::Direction::Received, at("2026-11-24T10:00"), ruem_va()).unwrap();
    w
}

/// … and the takeover of `line` announced with the AKM-TR.
fn taken_over(line: Option<&str>) -> Wechsel {
    let mut w = agreed();
    w.wbci_mut().apply(wbci::Direction::Sent, at("2026-11-25T08:00"), akm_tr(line)).unwrap();
    w
}

/// Attaches `order` in the service provider's and the network owner's view, sent at `when`.
fn place(w: &mut Wechsel, order: &spri::Auftrag, when: &str) {
    let t = at(when);
    w.attach(SProcess::new(SRole::Auftraggeber, order, SDir::Sent, t).unwrap()).unwrap();
    w.attach(SProcess::new(SRole::Leistungserbringer, order, SDir::Received, t).unwrap()).unwrap();
}

/// Applies an S/PRI order or message to both views: the service provider does not see the
/// leg with the losing provider.
fn send(w: &mut Wechsel, when: &str, msg: impl Into<spri::process::Message>) {
    let msg = msg.into();
    let t = at(when);
    let (le_dir, ag_dir) = match &msg {
        spri::process::Message::Auftrag(_) => (SDir::Received, Some(SDir::Sent)),
        spri::process::Message::Meldung(m) => match m.typ() {
            spri::Meldungstyp::RuemPv => (SDir::Received, None),
            spri::Meldungstyp::AkmPv
            | spri::Meldungstyp::AbmPv
            | spri::Meldungstyp::ErlmPv
            | spri::Meldungstyp::EntmPv
            | spri::Meldungstyp::AbbmPv => (SDir::Sent, None),
            spri::Meldungstyp::ErlmK | spri::Meldungstyp::ZwmAg | spri::Meldungstyp::TbkAg => {
                (SDir::Received, Some(SDir::Sent))
            }
            _ => (SDir::Sent, Some(SDir::Received)),
        },
        spri::process::Message::Teq(_) => panic!("no receipts in these scenarios"),
    };
    for p in w.spri_mut() {
        let dir = match p.role() {
            SRole::Leistungserbringer => Some(le_dir),
            SRole::Auftraggeber => ag_dir,
            SRole::AbgebenderProvider => None,
        };
        if let Some(dir) = dir {
            p.apply(dir, t, msg.clone()).unwrap();
        }
    }
}

/// The p. 27 sequence up to the ABM with the given dates.
fn up_to_abm(abm_date: &str) -> Wechsel {
    let mut w = taken_over(Some(PV_LINE));
    place(&mut w, &pv(), "2026-11-26T09:00");
    send(&mut w, "2026-11-26T09:05", sm!("meldung-qeb"));
    send(&mut w, "2026-11-26T10:00", sm!("meldung-akm-pv"));
    send(&mut w, "2026-11-27T10:00", sm!("meldung-ruem-pv"));
    send(&mut w, "2026-11-27T11:00", abm(abm_date));
    send(&mut w, "2026-11-27T11:01", sm!("meldung-abm-pv"));
    w
}

/// The whole p. 27 sequence, completed on `done`.
fn complete(done: &str) -> Wechsel {
    let mut w = up_to_abm("2027-01-15");
    send(&mut w, "2027-01-15T10:00", erlm(done));
    send(&mut w, "2027-01-15T10:01", erlm_pv(done));
    send(&mut w, "2027-01-16T10:00", sm!("meldung-entm"));
    send(&mut w, "2027-01-16T10:01", sm!("meldung-entm-pv"));
    w
}

fn only(w: &Wechsel, regel: (&str, &str)) -> Befund {
    let befunde = w.befunde();
    assert_eq!(befunde.len(), 1, "{befunde:#?}");
    let b = befunde.into_iter().next().unwrap();
    assert_eq!((b.regel, b.quelle), regel, "{b}");
    b
}

// -- the timeline ---------------------------------------------------------------------------

#[test]
fn the_p27_sequence_is_one_timeline() {
    use Partei::{EkpAbg, EkpAuf, Netzbetreiber as Nb};
    use Schnittstelle::{Spri, Wbci};
    let w = complete("2027-01-15");
    let timeline = w.timeline();
    let labels: Vec<&str> = timeline.iter().map(|e| e.nachricht).collect();
    assert_eq!(
        labels,
        [
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
            "ENTM-PV"
        ]
    );
    let parties: Vec<(Schnittstelle, Partei)> = timeline.iter().map(|e| (e.schnittstelle, e.partei)).collect();
    assert_eq!(
        parties,
        [
            (Wbci, EkpAuf),
            (Wbci, EkpAbg),
            (Wbci, EkpAuf),
            (Spri, EkpAuf),
            (Spri, Nb),
            (Spri, Nb),
            (Spri, EkpAbg),
            (Spri, Nb),
            (Spri, Nb),
            (Spri, Nb),
            (Spri, Nb),
            (Spri, Nb),
            (Spri, Nb)
        ]
    );
    assert!(timeline.windows(2).all(|p| p[0].at <= p[1].at));
    assert_eq!(timeline[1].to_string(), "Di 24.11.2026 10:00 WBCI EKPabg → RUEM-VA 8001 8104");
    assert_eq!(timeline[6].to_string(), "Fr 27.11.2026 10:00 S/PRI EKPabg → RUEM-PV 0021");
    assert!(w.befunde().is_empty(), "{:#?}", w.befunde());
}

#[test]
fn deadlines_and_actions_of_both_interfaces() {
    let mut w = taken_over(Some(PV_LINE));
    // Agreed, takeover announced, no order yet: the gaining provider places it.
    assert!(w.next_actions(Partei::EkpAuf).contains(&Aktion::SpriAuftragStellen));
    place(&mut w, &pv(), "2026-11-26T09:00");
    assert!(!w.next_actions(Partei::EkpAuf).contains(&Aktion::SpriAuftragStellen));
    send(&mut w, "2026-11-26T09:05", sm!("meldung-qeb"));
    send(&mut w, "2026-11-26T10:00", sm!("meldung-akm-pv"));

    let deadlines = w.deadlines();
    assert!(deadlines.windows(2).all(|p| p[0].due <= p[1].due));
    assert!(deadlines.iter().any(|f| f.schnittstelle == Schnittstelle::Wbci));
    let ruem = deadlines.iter().find(|f| f.rule().0 == "ruem-pv.antwortfrist").unwrap();
    assert_eq!((ruem.schnittstelle, ruem.partei), (Schnittstelle::Spri, Partei::EkpAbg));
    assert_eq!(deadlines.iter().filter(|f| f.rule().0 == "ruem-pv.antwortfrist").count(), 1, "deduplicated");

    let ext = w.spri()[0].externe_auftragsnummer().clone();
    let abg = w.next_actions(Partei::EkpAbg);
    assert!(abg.contains(&Aktion::Spri { auftrag: ext.clone(), action: SAction::Send(spri::Meldungstyp::RuemPv) }));
    assert!(abg.iter().any(|a| matches!(a, Aktion::Wbci(_))));
    let auf = w.next_actions(Partei::EkpAuf);
    assert!(auf.contains(&Aktion::Spri { auftrag: ext, action: SAction::Storno }));
    assert!(w.next_actions(Partei::Netzbetreiber).iter().all(|a| matches!(a, Aktion::Spri { .. })));
}

#[test]
fn kue_ag_and_neu_with_the_same_id_are_attached() {
    let mut w = taken_over(None);
    let mut kue = order(include_str!("../../spri/tests/fixtures/auftrag-kue-ag.xml"));
    kue.externe_auftragsnummer = spri::ExterneAuftragsnummer::new("A-2026-0002").unwrap();
    if let spri::Geschaeftsfall::KueAg(k) = &mut kue.geschaeftsfall {
        k.wunschtermin.datum = date("2027-01-15");
    }
    let neu = order(include_str!("../../spri/tests/fixtures/auftrag-neu.xml"));
    w.attach(SProcess::new(SRole::Auftraggeber, &kue, SDir::Sent, at("2026-11-26T09:00")).unwrap()).unwrap();
    w.attach(SProcess::new(SRole::Auftraggeber, &neu, SDir::Sent, at("2026-11-26T09:30")).unwrap()).unwrap();
    assert_eq!(w.spri().len(), 2);
    let t = w.timeline();
    assert_eq!((t[3].nachricht, t[3].partei), ("KUE-AG", Partei::EkpAbg), "the losing provider cancels its line");
    assert_eq!((t[4].nachricht, t[4].partei), ("NEU", Partei::EkpAuf));
    assert!(w.befunde().is_empty(), "{:#?}", w.befunde());
}

// -- attach ---------------------------------------------------------------------------------

#[test]
fn another_or_no_vorabstimmungs_id_is_refused() {
    // switch-timeline/SR-001
    let mut w = agreed();
    let before = w.clone();
    let mut other = pv();
    if let spri::Geschaeftsfall::Pv(p) = &mut other.geschaeftsfall {
        p.vorabstimmungs_id = "DEU.EKP1.V000000014".parse().unwrap();
    }
    let p = SProcess::new(SRole::Auftraggeber, &other, SDir::Sent, at("2026-11-26T09:00")).unwrap();
    let err = w.attach(p.clone()).unwrap_err();
    assert_eq!(*err.prozess, p, "the refused process is handed back");
    assert_eq!(err.gefunden.as_ref().map(spri::VorabstimmungsId::as_str), Some("DEU.EKP1.V000000014"));
    assert_eq!(err.erwartet.as_str(), "DEU.EKP1.V000000013");
    assert!(err.to_string().contains("§5.5.1.1"), "{err}");
    assert_eq!(w, before, "the switch is unchanged");

    // A NEU without the id cannot be part of the switch either.
    let mut neu = order(include_str!("../../spri/tests/fixtures/auftrag-neu.xml"));
    if let spri::Geschaeftsfall::Neu(n) = &mut neu.geschaeftsfall {
        n.vorabstimmungs_id = None;
    }
    let p = SProcess::new(SRole::Auftraggeber, &neu, SDir::Sent, at("2026-11-26T09:00")).unwrap();
    assert_eq!(w.attach(p).unwrap_err().gefunden, None);
    assert_eq!(w, before);
    assert!(w.spri().is_empty());
}

// -- one finding per broken link ------------------------------------------------------------

#[test]
fn an_order_before_the_agreement() {
    // switch-timeline/SR-002
    let mut w = requested();
    place(&mut w, &pv(), "2026-11-23T12:00");
    let b = only(&w, regeln::NICHT_ABGESTIMMT);
    assert_eq!(b.partei, Partei::EkpAuf);
    // Agreeing afterwards does not make the order placed before it right.
    w.wbci_mut().apply(wbci::Direction::Received, at("2026-11-24T10:00"), ruem_va()).unwrap();
    only(&w, regeln::NICHT_ABGESTIMMT);
}

#[test]
fn a_line_the_akm_tr_does_not_take_over() {
    // switch-timeline/SR-002
    let mut w = taken_over(Some("DEU.EKP2.1754639821"));
    place(&mut w, &pv(), "2026-11-26T09:00");
    let b = only(&w, regeln::LEITUNG);
    assert_eq!(b.partei, Partei::EkpAuf);
    assert!(b.text.contains(PV_LINE), "{b}");
}

#[test]
fn a_requested_date_other_than_the_switch_date() {
    // switch-timeline/SR-002
    let mut w = taken_over(Some(PV_LINE));
    let mut order = pv();
    pv_wish(&mut order, "2027-01-22");
    place(&mut w, &order, "2026-11-26T09:00");
    let b = only(&w, regeln::TERMIN);
    assert_eq!(b.partei, Partei::EkpAuf);
    assert_eq!(b.text, "order A-2026-0001: requested Fr 22.01.2027 — the WBCI switch date is Fr 15.01.2027");
}

#[test]
fn a_binding_date_other_than_the_switch_date() {
    // switch-timeline/SR-002
    let w = up_to_abm("2027-01-18");
    let b = only(&w, regeln::TERMIN);
    assert_eq!(b.partei, Partei::Netzbetreiber);
}

#[test]
fn a_spri_postponement_before_the_wbci_one() {
    // switch-timeline/SR-002
    let mut w = up_to_abm("2027-01-15");
    let tv = spri::Auftrag {
        aenderungskennzeichen: spri::Aenderungskennzeichen::Terminverschiebung,
        termin_neu: Some(date("2027-01-29")),
        ..pv()
    };
    send(&mut w, "2026-12-01T09:00", tv);
    let b = only(&w, regeln::TV_ZUERST_WBCI);
    assert_eq!(b.partei, Partei::EkpAuf);
    assert!(b.text.contains("Fr 29.01.2027") && b.text.contains("Fr 15.01.2027"), "{b}");
}

#[test]
fn a_postponement_first_in_wbci_then_in_spri_is_fine() {
    let mut w = up_to_abm("2027-01-15");
    let tvs = wbci_anfrage(include_str!("../../wbci/tests/fixtures/tvs-va.xml"));
    w.wbci_mut().apply(wbci::Direction::Sent, at("2026-12-01T09:00"), tvs).unwrap();
    let erlm_tvs = wbci_meldung(include_str!("../../wbci/tests/fixtures/erlm.xml"));
    w.wbci_mut().apply(wbci::Direction::Received, at("2026-12-01T12:00"), erlm_tvs).unwrap();
    assert_eq!(w.wechseltermin(), Some(date("2027-01-29")));
    let tv = spri::Auftrag {
        aenderungskennzeichen: spri::Aenderungskennzeichen::Terminverschiebung,
        termin_neu: Some(date("2027-01-29")),
        ..pv()
    };
    send(&mut w, "2026-12-01T13:00", tv);
    assert!(w.befunde().is_empty(), "{:#?}", w.befunde());
    send(&mut w, "2026-12-01T15:00", abm("2027-01-29"));
    assert!(w.befunde().is_empty(), "{:#?}", w.befunde());
}

#[test]
fn a_wbci_cancellation_with_the_order_still_open() {
    // switch-timeline/SR-002
    let mut w = taken_over(Some(PV_LINE));
    place(&mut w, &pv(), "2026-11-26T09:00");
    send(&mut w, "2026-11-26T09:05", sm!("meldung-qeb"));
    let wbci::Anfrage::Storno(storno) = wbci_anfrage(include_str!("../../wbci/tests/fixtures/str-auf-ekpauf.xml"))
    else {
        panic!("not a cancellation");
    };
    w.wbci_mut().apply(wbci::Direction::Sent, at("2026-12-01T09:00"), wbci::Anfrage::from(storno.clone())).unwrap();
    assert!(w.befunde().is_empty(), "nothing is cancelled yet");
    let ok = wbci::Meldung::from(wbci::Erlm::storno_ok(&storno));
    w.wbci_mut().apply(wbci::Direction::Received, at("2026-12-01T12:00"), ok).unwrap();
    let b = only(&w, regeln::STORNO);
    assert_eq!(b.partei, Partei::EkpAuf);
    // The S/PRI Storno settles it.
    let s = spri::Auftrag { aenderungskennzeichen: spri::Aenderungskennzeichen::Storno, ..pv() };
    send(&mut w, "2026-12-01T13:00", s);
    assert!(w.befunde().is_empty(), "{:#?}", w.befunde());
}

#[test]
fn a_completion_later_than_one_working_day() {
    // switch-timeline/SR-002
    // Fr 15.01.2027 + 1 working day is Mo 18.01.2027.
    assert!(complete("2027-01-18").befunde().is_empty());
    let b = only(&complete("2027-01-19"), regeln::KONTINUITAET);
    assert_eq!(b.partei, Partei::Netzbetreiber);
    assert_eq!(
        b.to_string(),
        "wechsel.kontinuitaet (§ 59 Abs. 1 TKG (interpretation)) — Netzbetreiber: order A-2026-0001 completed on Di 19.01.2027, later than one working day after the WBCI switch date Fr 15.01.2027 (latest Mo 18.01.2027)"
    );
}

// -- no personal data -----------------------------------------------------------------------

#[test]
fn timeline_and_findings_carry_no_personal_data() {
    // switch-timeline/SR-003
    let mut w = complete("2027-01-19");
    let mut order = pv();
    order.externe_auftragsnummer = spri::ExterneAuftragsnummer::new("A-2026-0009").unwrap();
    pv_wish(&mut order, "2027-01-22");
    w.attach(SProcess::new(SRole::Auftraggeber, &order, SDir::Sent, at("2026-11-26T09:00")).unwrap()).unwrap();
    let befunde = w.befunde();
    assert!(befunde.len() >= 2, "{befunde:#?}");
    let mut out = String::new();
    for e in w.timeline() {
        writeln!(out, "{e}\n{e:?}").unwrap();
    }
    for b in &befunde {
        writeln!(out, "{b}\n{b:?}").unwrap();
    }
    for f in w.deadlines() {
        writeln!(out, "{f}").unwrap();
    }
    for secret in [
        "Mustermann",
        "Erika",
        "Max",
        "Beispiel",
        "Hauptstraße",
        "10115",
        "Berlin",
        "Mitte",
        "1234567",
        "9876543",
        "7654321",
        "example.de",
        "Neuer Anbieter",
        "Alter Anbieter",
    ] {
        assert!(!out.contains(secret), "{secret} in:\n{out}");
    }
}
