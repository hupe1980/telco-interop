//! The simulated losing provider: its answers, their timing, the refusals the process
//! prescribes, and robustness against any input.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::*;
use proptest::prelude::*;
use wbci::deadline;
use wbci::pruefung::Bestand;
use wbci::sim::{Antwort, Ekpabg, Latency, Outbound, Rejection};
use wbci::*;

fn signatur() -> SignaturId {
    SignaturId { issuer: SignaturText::new("CN=Sandbox").unwrap(), serial: SignaturText::new("1").unwrap() }
}

fn t0() -> Timestamp {
    ts("2026-11-23T08:00:00Z")
}

fn record() -> Bestand {
    let mut b = Bestand::new(kunde(), "D123".parse().unwrap());
    b.standort = Some(standort());
    b.rufnummern = vec![nummer("30", "1234567")];
    b
}

fn request(a: impl Into<Anfrage>, at: Timestamp) -> Payload {
    Payload::AnfrageRequest { control: Control::new(at, signatur()), anfrage: a.into() }
}

fn ack(m: &Meldung, at: Timestamp) -> Payload {
    Payload::MeldungResponse { control: Control::new(at, signatur()), quittung: Teq::for_meldung(m, Meldungscode::Ok) }
}

fn meldung(o: &Outbound) -> &Meldung {
    match &o.payload {
        Payload::MeldungRequest { meldung, .. } => meldung,
        p => panic!("not a message: {p:?}"),
    }
}

fn codes(m: &Meldung) -> Vec<&str> {
    m.codes().map(Meldungscode::as_str).collect()
}

/// Every outbound validates without an error against what it answers, at its own time.
#[track_caller]
fn clean(out: &[Outbound], answering: &Anfrage) {
    let cal = Calendar::wbci();
    for o in out {
        let issues = match &o.payload {
            Payload::MeldungRequest { meldung, .. } => meldung.issues_at(Some(answering), o.at, &cal),
            Payload::AnfrageResponse { quittung, .. } | Payload::MeldungResponse { quittung, .. } => quittung.issues(),
            Payload::AnfrageRequest { anfrage, .. } => anfrage.issues_at(o.at, &cal),
        };
        assert!(issues.is_empty(), "{issues:#?}");
    }
}

/// Sends the VA, acknowledges the answer; returns the peer and the answer.
fn answered(antwort: Antwort, va: &Vorabstimmung) -> (Ekpabg, Outbound) {
    let mut peer = Ekpabg::new(antwort, signatur());
    let reply = peer.receive(&request(va.clone(), t0()), t0());
    assert!(reply.rejection.is_none());
    assert_eq!(reply.outbound.len(), 1);
    let Payload::AnfrageResponse { quittung, .. } = &reply.outbound[0].payload else { panic!() };
    assert!(quittung.is_ok());
    assert_eq!(reply.outbound[0].at, t0() + jiff::SignedDuration::from_secs(2));
    let due = peer.next_due().unwrap();
    let out = peer.advance(due);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].at, due);
    clean(&out, &va.clone().into());
    let answer = out[0].clone();
    let r = peer.receive(&ack(meldung(&answer), due), due);
    assert!(r.rejection.is_none() && r.outbound.is_empty(), "{r:?}");
    assert!(peer.stopped().is_none());
    (peer, answer)
}

#[test]
fn agrees_from_a_matching_record() {
    // sandbox/SR-001
    let va = va_kue_mrn();
    let (peer, answer) = answered(Antwort::Pruefung(Box::new(record())), &va);
    assert_eq!(codes(meldung(&answer)), ["8001"]);
    // One hour after the receipt, well inside the 3 AT answer deadline.
    assert_eq!(answer.at, t0() + jiff::SignedDuration::from_secs(2 + 3600));
    assert_eq!(*peer.process().unwrap().state(), process::State::Agreed { wechseltermin: date("2027-01-15") });
    assert_eq!(peer.next_due(), None);
}

#[test]
fn refuses_from_a_record_with_another_number() {
    let mut b = record();
    b.rufnummern = vec![nummer("30", "9999999")];
    let (peer, answer) = answered(Antwort::Pruefung(Box::new(b)), &va_kue_mrn());
    assert_eq!(codes(meldung(&answer))[0], "8103");
    assert!(matches!(peer.process().unwrap().state(), process::State::Rejected { .. }));
}

#[test]
fn forced_outcomes() {
    let (_, answer) = answered(Antwort::Zustimmung, &va_kue_orn());
    assert_eq!(codes(meldung(&answer)), ["8001"]);
    // A requested date closer than the 15 AT a switch date needs is moved: 8002.
    let mut soon = va_kue_orn();
    soon.kundenwunschtermin = date("2026-12-04");
    let mut peer = Ekpabg::new(Antwort::Zustimmung, signatur());
    peer.receive(&request(soon, t0()), t0());
    let out = peer.advance(Timestamp::MAX);
    let Meldung::RuemVa(r) = meldung(&out[0]) else { panic!() };
    assert_eq!(r.positionen[0].position.code, Meldungscode::Nat);
    let earliest = deadline::earliest_wechseltermin(out[0].at, date("2026-12-04"), &Calendar::wbci()).date;
    assert_eq!(r.wechseltermin, earliest);
    let (_, answer) = answered(Antwort::Ablehnung(Meldungscode::Sonstiges), &va_kue_orn());
    let Meldung::Abbm(a) = meldung(&answer) else { panic!() };
    assert_eq!(codes(meldung(&answer)), ["8116"]);
    assert!(a.begruendung.is_some());
}

#[test]
fn confirms_cancellation_and_postponement() {
    let va = va_kue_mrn();
    let (mut peer, ruem) = answered(Antwort::Zustimmung, &va);
    let at = ruem.at + jiff::SignedDuration::from_hours(24);
    let storno = str_auf_auf();
    let r = peer.receive(&request(storno.clone(), at), at);
    assert!(r.rejection.is_none(), "{r:?}");
    let erlm = peer.advance(Timestamp::MAX);
    assert_eq!(codes(meldung(&erlm[0])), ["8004"]);
    clean(&erlm, &storno.into());
    assert_eq!(*peer.process().unwrap().state(), process::State::Cancelled);

    let (mut peer, ruem) = answered(Antwort::Zustimmung, &va);
    let at = ruem.at + jiff::SignedDuration::from_hours(24);
    let tvs = tvs();
    let r = peer.receive(&request(tvs.clone(), at), at);
    assert!(r.rejection.is_none(), "{r:?}");
    let erlm = peer.advance(Timestamp::MAX);
    assert_eq!(codes(meldung(&erlm[0])), ["8003"]);
    clean(&erlm, &tvs.into());
}

#[test]
fn refuses_what_the_process_forbids() {
    // sandbox/SR-001
    let va = va_kue_mrn();
    // A cancellation two days before the switch date: too late, 8142.
    let (mut peer, _) = answered(Antwort::Zustimmung, &va);
    let late = ts("2027-01-13T08:00:00Z");
    let r = peer.receive(&request(str_auf_auf(), late), late);
    assert!(matches!(&r.rejection, Some(Rejection::Refused(f)) if f.code == Meldungscode::StornoAbgelehnt), "{r:?}");
    assert_eq!(r.outbound.len(), 2, "receipt and ABBM: {r:?}");
    assert_eq!(codes(meldung(&r.outbound[1])), ["8142"]);
    // A second pre-agreement for the line: 8118.
    let mut other = va_kue_mrn();
    other.id = VorabstimmungsId::allocate(&cc("DEU.EKP1"), 99).unwrap();
    let r = peer.receive(&request(other, late), late);
    assert_eq!(codes(meldung(&r.outbound[1])), ["8118"]);
    // A receipt nothing awaits: no answer.
    let r = peer.receive(&ack(meldung(&r.outbound[1]), late), late);
    assert!(r.outbound.is_empty() && r.rejection.is_some());

    // A cancellation before any pre-agreement: 8140; a postponement: 8116 with the reason.
    let mut fresh = Ekpabg::new(Antwort::Zustimmung, signatur());
    let r = fresh.receive(&request(str_auf_auf(), t0()), t0());
    assert_eq!(codes(meldung(&r.outbound[1])), ["8140"]);
    clean(&r.outbound[1..], &str_auf_auf().into());
    let r = fresh.receive(&request(tvs(), t0()), t0());
    // No agreed date is known, so the ABBM cannot name the date that stays valid.
    assert_eq!(codes(meldung(&r.outbound[1])), ["8116"]);
    assert!(fresh.process().is_none());

    // An AKM-TR before the RUEM-VA: receipt, refusal, ABBM-TR.
    let mut peer = Ekpabg::new(Antwort::Zustimmung, signatur());
    peer.receive(&request(va.clone(), t0()), t0());
    let akm = AkmTr::uebernahme(&RuemVa::zustimmung(&va, va.kundenwunschtermin), vec![], false);
    let p = Payload::MeldungRequest { control: Control::new(t0(), signatur()), meldung: akm.into() };
    let r = peer.receive(&p, t0());
    assert!(matches!(r.rejection, Some(Rejection::Refused(_))), "{r:?}");
    assert!(matches!(r.outbound[0].payload, Payload::MeldungResponse { .. }));
    assert!(peer.stopped().is_none());

    // A postponement to an earlier date after the agreement: refused, the date that stays valid named.
    let (mut peer, _) = answered(Antwort::Zustimmung, &va);
    let mut earlier = tvs();
    earlier.neuer_kundenwunschtermin = date("2027-01-08");
    let at = ts("2026-11-25T08:00:00Z");
    let r = peer.receive(&request(earlier.clone(), at), at);
    assert_eq!(codes(meldung(&r.outbound[1])), ["8141"]);
    clean(&r.outbound[1..], &earlier.into());
}

#[test]
fn a_slow_answer_stays_inside_its_deadline() {
    let slow = Latency { answer: jiff::SignedDuration::from_hours(24 * 10), ..Latency::default() };
    let mut peer = Ekpabg::new(Antwort::Zustimmung, signatur()).with_latency(slow);
    peer.receive(&request(va_kue_mrn(), t0()), t0());
    let out = peer.advance(Timestamp::MAX);
    let teq_at = t0() + jiff::SignedDuration::from_secs(2);
    let due = deadline::va_answer(teq_at, &Calendar::wbci()).due;
    assert!(out[0].at < due, "{} not before {due}", out[0].at);
}

#[test]
fn unreadable_bytes_are_named_with_their_receipt_code() {
    // sandbox/SR-001
    let mut peer = Ekpabg::new(Antwort::Zustimmung, signatur());
    let r = peer.receive_xml(b"<!DOCTYPE x []><x/>", t0());
    assert!(r.outbound.is_empty());
    let Some(Rejection::Unreadable(e)) = &r.rejection else { panic!() };
    assert_eq!(e.teq_code().as_str(), "0995");
    assert!(r.rejection.unwrap().to_string().contains("TEQ 0995"));
}

const PERSONAL: [&str; 6] = ["Mustermann", "Erika", "Hauptstraße", "10115", "1234567", "Berlin"];

fn va_xml() -> String {
    request(va_kue_mrn(), t0()).to_xml().unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..2048), secs in 0i64..10_000_000) {
        // sandbox/SR-001, sandbox/SR-002
        let at = t0() + jiff::SignedDuration::from_secs(secs);
        let mut peer = Ekpabg::new(Antwort::Zustimmung, signatur());
        let _ = peer.receive_xml(&bytes, at);
        let _ = peer.advance(Timestamp::MAX);
        prop_assert!(peer.next_due().is_none());
    }

    #[test]
    fn mutated_requests_never_leak_personal_data(cut in 0usize..4000, flip in 0usize..4000, byte in any::<u8>()) {
        // sandbox/SR-003
        let mut bytes = va_xml().into_bytes();
        if let Some(b) = bytes.get_mut(flip) { *b = byte; }
        bytes.truncate(cut.max(1));
        let mut peer = Ekpabg::new(Antwort::Zustimmung, signatur());
        let r = peer.receive_xml(&bytes, t0());
        let text = format!("{} {:?} {:?}", r.rejection.as_ref().map(ToString::to_string).unwrap_or_default(), r.rejection, peer.stopped());
        for secret in PERSONAL {
            prop_assert!(!text.contains(secret), "{} in {}", secret, text);
        }
    }

    #[test]
    fn any_sequence_terminates(steps in proptest::collection::vec((0u8..6, 0i64..2_000_000), 0..12)) {
        // sandbox/SR-002
        let va = va_kue_mrn();
        let mut peer = Ekpabg::new(Antwort::Pruefung(Box::new(record())), signatur());
        let ruem: Meldung = RuemVa::zustimmung(&va, va.kundenwunschtermin).into();
        for (kind, secs) in steps {
            let at = t0() + jiff::SignedDuration::from_secs(secs);
            let p = match kind {
                0 => request(va.clone(), at),
                1 => request(str_auf_auf(), at),
                2 => request(tvs(), at),
                3 => ack(&ruem, at),
                4 => Payload::MeldungRequest { control: Control::new(at, signatur()), meldung: AkmTr::uebernahme(&RuemVa::zustimmung(&va, va.kundenwunschtermin), vec![], false).into() },
                _ => request(str_aen_abg(), at),
            };
            let _ = peer.receive(&p, at);
        }
        let _ = peer.advance(Timestamp::MAX);
        prop_assert!(peer.next_due().is_none_or(|t| t > Timestamp::MAX - jiff::SignedDuration::from_secs(1)));
        let debug = format!("{peer:?}");
        for secret in PERSONAL {
            prop_assert!(!debug.contains(secret), "{} in Debug", secret);
        }
    }
}
