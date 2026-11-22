//! The simulated network owner and losing provider: the path of every business case, Storno,
//! Terminverschiebung, the provider-switch leg, refusals, and robustness against any input.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::*;
use proptest::prelude::*;
use spri::process::{Role, State};
use spri::sim::{AbgebenderProvider, Latency, Leistungserbringer, Outbound, Rejection, Rueckmeldung};
use spri::*;

fn signatur() -> SignaturId {
    SignaturId { issuer: SignaturText::new("CN=Sandbox").unwrap(), serial: SignaturText::new("1").unwrap() }
}

fn t0() -> Timestamp {
    ts("2026-11-23T08:00:00Z")
}

fn secs(n: i64) -> jiff::SignedDuration {
    jiff::SignedDuration::from_secs(n)
}

/// The network owner, an optional losing provider and the service provider, who acknowledges
/// every message it receives two seconds later and sends nothing else on its own.
struct Net {
    le: Leistungserbringer,
    ab: Option<AbgebenderProvider>,
    /// What travelled, in delivery order: (time, sender, receiver, payload).
    log: Vec<(Timestamp, Role, Role, Payload)>,
    queue: Vec<(Timestamp, Role, Role, Payload)>,
    rejections: Vec<String>,
}

impl Net {
    fn new(le: Leistungserbringer, ab: Option<AbgebenderProvider>) -> Self {
        Self { le, ab, log: Vec::new(), queue: Vec::new(), rejections: Vec::new() }
    }

    fn enqueue(&mut self, from: Role, out: Vec<Outbound>) {
        for o in out {
            self.queue.push((o.at, from, o.to, o.payload));
        }
    }

    /// The service provider sends `p` at `at`.
    fn send(&mut self, p: Payload, at: Timestamp) {
        self.queue.push((at, Role::Auftraggeber, Role::Leistungserbringer, p));
    }

    fn run(&mut self, until: Timestamp) {
        for _ in 0..10_000 {
            self.queue.sort_by_key(|q| q.0);
            let q = self.queue.first().map(|q| q.0);
            let le = self.le.next_due();
            let ab = self.ab.as_ref().and_then(AbgebenderProvider::next_due);
            let next = [q, le, ab].into_iter().flatten().min();
            let Some(t) = next.filter(|t| *t <= until) else { return };
            if q == Some(t) {
                let (at, from, to, payload) = self.queue.remove(0);
                self.log.push((at, from, to, payload.clone()));
                let reply = match to {
                    Role::Leistungserbringer => Some((Role::Leistungserbringer, self.le.receive(&payload, at))),
                    Role::AbgebenderProvider => {
                        self.ab.as_mut().map(|ab| (Role::AbgebenderProvider, ab.receive(&payload, at)))
                    }
                    Role::Auftraggeber => {
                        if let Payload::MeldungRequest { meldung, .. } = &payload {
                            let teq =
                                Teq::new(Meldungscode::Ok, Some(meldung.externe_auftragsnummer().clone()), Some(ag()));
                            let at = at + secs(2);
                            self.queue.push((
                                at,
                                Role::Auftraggeber,
                                Role::Leistungserbringer,
                                Payload::MeldungResponse { control: Control::new(at, signatur()), quittung: teq },
                            ));
                        }
                        None
                    }
                };
                if let Some((who, r)) = reply {
                    if let Some(rej) = &r.rejection {
                        self.rejections.push(rej.to_string());
                    }
                    self.enqueue(who, r.outbound);
                }
            } else if le == Some(t) {
                let out = self.le.advance(t);
                self.enqueue(Role::Leistungserbringer, out);
            } else if let Some(ab) = self.ab.as_mut() {
                let out = ab.advance(t);
                self.enqueue(Role::AbgebenderProvider, out);
            }
        }
        panic!("the simulation does not settle");
    }

    /// The message kinds the network owner and the losing provider sent, receipts left out.
    fn kinds(&self) -> Vec<&'static str> {
        self.log
            .iter()
            .filter(|(_, from, _, p)| *from != Role::Auftraggeber && matches!(p, Payload::MeldungRequest { .. }))
            .map(|(_, _, _, p)| p.kind())
            .collect()
    }

    /// Every message the peers send validates without an issue; every peer accepted what it was sent.
    #[track_caller]
    fn clean(&self) {
        assert!(self.rejections.is_empty(), "{:?}", self.rejections);
        assert!(self.le.stopped().is_none());
        assert!(self.ab.as_ref().is_none_or(|a| a.stopped().is_none()));
        for (at, _, _, p) in &self.log {
            let issues = match p {
                Payload::MeldungRequest { meldung, .. } => meldung.issues(),
                Payload::AuftragResponse { quittung, .. } | Payload::MeldungResponse { quittung, .. } => {
                    quittung.issues()
                }
                // The orders are the test's input (fixtures with deliberate extras).
                Payload::AuftragRequest { .. } => Vec::new(),
            };
            assert!(issues.is_empty(), "{} at {at}: {issues:#?}", p.kind());
        }
    }

    fn state(&self) -> State {
        self.le.process().unwrap().state().clone()
    }
}

fn order(a: Auftrag, at: Timestamp) -> Payload {
    Payload::AuftragRequest { control: Control::new(at, signatur()), auftrag: a }
}

fn far() -> Timestamp {
    ts("2028-01-01T00:00:00Z")
}

/// A repair order that is not a Reklamation.
fn est_new() -> Auftrag {
    let mut a = est();
    if let Geschaeftsfall::Est(e) = &mut a.geschaeftsfall {
        e.reklamation = None;
    }
    a
}

#[test]
fn every_business_case_walks_its_path() {
    // sandbox/SR-001
    use GeschaeftsfallCode as G;
    let cases: Vec<(Auftrag, &[&str])> = vec![
        (neu(), &["QEB", "ABM", "ERLM", "ENTM"]),
        (kue_ag(), &["QEB", "ABM", "ERLM", "ENTM"]),
        (lae(), &["QEB", "ABM", "ERLM", "ENTM"]),
        (aen_lmae(), &["QEB", "ABM", "ERLM", "ENTM"]),
        (est_new(), &["QEB", "ERLM", "ENTM"]),
        (get(), &["ERLM"]),
        (set(), &["ERLM"]),
    ];
    for (a, kinds) in cases {
        let fall = a.geschaeftsfall.code();
        let mut net = Net::new(Leistungserbringer::new(signatur()), None);
        net.send(order(a, t0()), t0());
        net.run(far());
        net.clean();
        assert_eq!(net.kinds(), kinds, "{fall}");
        assert_eq!(net.state(), State::Closed, "{fall}");
        assert_eq!(net.le.next_due(), None);
        if !matches!(fall, G::Get | G::Set | G::Est) {
            // The ERLM goes at 10:00 German time on the binding date.
            let erlm = net.log.iter().find(|(_, _, _, p)| p.kind() == "ERLM").unwrap();
            let expected = if fall == G::KueAg { "2027-02-01T09:00:00Z" } else { "2027-01-15T09:00:00Z" };
            assert_eq!(erlm.0, ts(expected), "{fall}");
        }
    }
}

#[test]
fn an_unanswered_appointment_ends_with_1198() {
    let mut net = Net::new(Leistungserbringer::new(signatur()).with_terminanforderung(), None);
    net.send(order(neu(), t0()), t0());
    net.run(far());
    net.clean();
    assert_eq!(net.kinds(), ["QEB", "ABM", "TAM", "MTAM", "ABBM"]);
    assert_eq!(net.state(), State::Aborted { code: Meldungscode::C1198 });
    // Each inside its 10-Werktage deadline.
    let at =
        |k: &str| net.log.iter().find(|(_, from, _, p)| *from == Role::Leistungserbringer && p.kind() == k).unwrap().0;
    let mtam_due = deadline::mtam_due(at("TAM"), &Calendar::werktage()).due;
    assert!(at("MTAM") < mtam_due);
    assert!(at("ABBM") < deadline::abbm_1198_due(at("MTAM"), &Calendar::werktage()).due);
}

fn after_abm(le: Leistungserbringer) -> (Net, Timestamp) {
    let mut net = Net::new(le, None);
    net.send(order(neu(), t0()), t0());
    let at = t0() + jiff::SignedDuration::from_hours(24);
    net.run(at);
    assert!(matches!(net.state(), State::Confirmed { .. }));
    (net, at)
}

#[test]
fn storno_confirmed_and_refused() {
    let (mut net, at) = after_abm(Leistungserbringer::new(signatur()));
    let mut storno = neu();
    storno.aenderungskennzeichen = Aenderungskennzeichen::Storno;
    net.send(order(storno.clone(), at), at);
    net.run(far());
    net.clean();
    assert_eq!(net.kinds(), ["QEB", "ABM", "ERLM", "ENTM"]);
    let erlm = net.log.iter().rev().find(|(_, _, _, p)| p.kind() == "ERLM").unwrap();
    let Payload::MeldungRequest { meldung, .. } = &erlm.3 else { panic!() };
    assert_eq!(meldung.codes()[0].as_str(), "0012");
    assert_eq!(net.state(), State::Closed);

    let (mut net, at) = after_abm(Leistungserbringer::new(signatur()).with_storno_abgelehnt());
    net.send(order(storno, at), at);
    net.run(far());
    net.clean();
    assert_eq!(net.kinds(), ["QEB", "ABM", "ABBM", "ERLM", "ENTM"]);
    assert_eq!(net.state(), State::Closed);
}

#[test]
fn a_terminverschiebung_is_confirmed_with_a_new_abm() {
    let (mut net, at) = after_abm(Leistungserbringer::new(signatur()));
    let mut tv = neu();
    tv.aenderungskennzeichen = Aenderungskennzeichen::Terminverschiebung;
    tv.termin_neu = Some(date("2027-01-22"));
    net.send(order(tv.clone(), at), at);
    net.run(far());
    net.clean();
    assert_eq!(net.kinds(), ["QEB", "ABM", "ABM", "ERLM", "ENTM"]);
    let erlm = net.log.iter().find(|(_, _, _, p)| p.kind() == "ERLM").unwrap();
    assert_eq!(erlm.0, ts("2027-01-22T09:00:00Z"));

    let (mut net, at) = after_abm(Leistungserbringer::new(signatur()).with_terminverschiebung_abgelehnt());
    net.send(order(tv, at), at);
    net.run(far());
    net.clean();
    assert_eq!(net.kinds(), ["QEB", "ABM", "ABBM", "ERLM", "ENTM"]);
}

fn pv_net(r: Rueckmeldung) -> Net {
    let mut net = Net::new(Leistungserbringer::new(signatur()), Some(AbgebenderProvider::new(r, signatur())));
    net.send(order(pv(), t0()), t0());
    net.run(far());
    net
}

#[test]
fn the_provider_switch_and_its_aborts() {
    let net = pv_net(Rueckmeldung::Zustimmung);
    net.clean();
    assert_eq!(net.kinds(), ["QEB", "AKM-PV", "RUEM-PV", "ABM", "ABM-PV", "ERLM", "ERLM-PV", "ENTM", "ENTM-PV"]);
    assert_eq!(net.state(), State::Closed);
    assert_eq!(*net.ab.as_ref().unwrap().process().unwrap().state(), State::Closed);

    let net = pv_net(Rueckmeldung::Ablehnung(Antwortcode::new("KVE").unwrap()));
    net.clean();
    assert_eq!(net.kinds(), ["QEB", "AKM-PV", "RUEM-PV", "ABBM", "ABBM-PV"]);
    assert_eq!(net.state(), State::Aborted { code: Meldungscode::C0041 });

    let net = pv_net(Rueckmeldung::Keine);
    net.clean();
    assert_eq!(net.kinds(), ["QEB", "AKM-PV", "ABBM", "ABBM-PV"]);
    assert_eq!(net.state(), State::Aborted { code: Meldungscode::C0042 });
    assert!(matches!(net.ab.as_ref().unwrap().process().unwrap().state(), State::Aborted { .. }));
}

#[test]
fn refuses_what_the_process_forbids() {
    // sandbox/SR-001
    // A Storno before the QEB: ABBM 1087 with the Änderungskennzeichen Storno.
    let mut le = Leistungserbringer::new(signatur());
    le.receive(&order(neu(), t0()), t0());
    let mut storno = neu();
    storno.aenderungskennzeichen = Aenderungskennzeichen::Storno;
    let r = le.receive(&order(storno, t0() + secs(5)), t0() + secs(5));
    let Some(Rejection::Refused(f)) = &r.rejection else { panic!("{r:?}") };
    assert_eq!(f.answer, Some(Meldungscode::C1087));
    let Payload::MeldungRequest { meldung, .. } = &r.outbound.last().unwrap().payload else { panic!() };
    assert_eq!(meldung.auftragstyp.aenderungskennzeichen, Aenderungskennzeichen::Storno);
    assert!(meldung.issues().is_empty());
    // An ERLM-K before the ABM: receipt, refusal, nothing else.
    let erlm_k = Payload::MeldungRequest {
        control: c(),
        meldung: meldung_of(GeschaeftsfallCode::Neu, GeschaeftsfallArt::Bereitstellung, erlm_k()),
    };
    let r = le.receive(&erlm_k, t0() + secs(10));
    assert!(matches!(r.rejection, Some(Rejection::Refused(_))));
    assert!(r.outbound.iter().all(|o| matches!(o.payload, Payload::MeldungResponse { .. })));
    // An order whose date lies in the past: refused before the QEB with the predicted code.
    let mut old = neu();
    if let Geschaeftsfall::Neu(b) = &mut old.geschaeftsfall {
        b.wunschtermin.datum = date("2026-11-01");
    }
    let mut le = Leistungserbringer::new(signatur());
    le.receive(&order(old, t0()), t0());
    let out = le.advance(far());
    let Payload::MeldungRequest { meldung, .. } = &out[0].payload else { panic!() };
    assert_eq!(meldung.typ(), Meldungstyp::Abbm);
    assert!(meldung.codes().iter().any(|c| c.as_str() == "1003"), "{:?}", meldung.codes());
    // A receipt with nothing received: refused.
    let mut fresh = Leistungserbringer::new(signatur());
    let r = fresh.receive(&Payload::MeldungResponse { control: c(), quittung: teq_ok() }, t0());
    assert!(r.outbound.is_empty() && r.rejection.is_some());
    let mut ab = AbgebenderProvider::new(Rueckmeldung::Zustimmung, signatur());
    let r = ab.receive(&order(pv(), t0()), t0());
    assert!(r.rejection.is_some());
}

fn meldung_of(fall: GeschaeftsfallCode, art: GeschaeftsfallArt, inhalt: impl Into<Meldungsinhalt>) -> Meldung {
    meldung(fall, art, inhalt)
}

#[test]
fn slow_latencies_stay_inside_the_deadlines() {
    let slow =
        Latency { receipt: jiff::SignedDuration::from_mins(5), answer: jiff::SignedDuration::from_hours(24 * 30) };
    let mut ab = AbgebenderProvider::new(Rueckmeldung::Zustimmung, signatur()).with_latency(slow);
    let akm = Payload::MeldungRequest {
        control: c(),
        meldung: meldung(GeschaeftsfallCode::Pv, GeschaeftsfallArt::Endkundenanbieterwechsel, akm_pv()),
    };
    let r = ab.receive(&akm, t0());
    assert_eq!(r.outbound[0].at, t0() + secs(30), "the receipt is clamped to 30 s");
    let out = ab.advance(far());
    assert!(out[0].at < deadline::ruem_pv(date("2026-12-01")).due);
}

#[test]
fn unreadable_bytes_are_named_with_their_receipt_code() {
    let mut le = Leistungserbringer::new(signatur());
    let r = le.receive_xml(b"<!DOCTYPE x []><x/>", t0());
    assert!(r.outbound.is_empty());
    assert!(r.rejection.unwrap().to_string().contains("TEQ 0995"));
}

const PERSONAL: [&str; 7] = ["Mustermann", "Erika", "Hauptstraße", "10115", "Berlin", "030 ", "example"];

fn pv_xml() -> String {
    order(pv(), t0()).to_xml().unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..2048), s in 0i64..10_000_000) {
        // sandbox/SR-001, sandbox/SR-002
        let at = t0() + secs(s);
        let mut le = Leistungserbringer::new(signatur());
        let mut ab = AbgebenderProvider::new(Rueckmeldung::Zustimmung, signatur());
        let _ = le.receive_xml(&bytes, at);
        let _ = ab.receive_xml(&bytes, at);
        let _ = le.advance(Timestamp::MAX);
        let _ = ab.advance(Timestamp::MAX);
        prop_assert!(le.next_due().is_none() && ab.next_due().is_none());
    }

    #[test]
    fn mutated_orders_never_leak_personal_data(cut in 0usize..6000, flip in 0usize..6000, byte in any::<u8>()) {
        // sandbox/SR-003
        let mut bytes = pv_xml().into_bytes();
        if let Some(b) = bytes.get_mut(flip) { *b = byte; }
        bytes.truncate(cut.max(1));
        let mut le = Leistungserbringer::new(signatur());
        let r = le.receive_xml(&bytes, t0());
        let _ = le.advance(Timestamp::MAX);
        let text = format!("{} {:?} {le:?}", r.rejection.as_ref().map(ToString::to_string).unwrap_or_default(), r.rejection);
        for secret in PERSONAL {
            prop_assert!(!text.contains(secret), "{} in {}", secret, text);
        }
    }

    #[test]
    fn any_sequence_terminates(steps in proptest::collection::vec((0u8..7, 0i64..5_000_000), 0..12)) {
        // sandbox/SR-002
        let mut le = Leistungserbringer::new(signatur()).with_terminanforderung();
        let mut ab = AbgebenderProvider::new(Rueckmeldung::Zustimmung, signatur());
        for (kind, s) in steps {
            let at = t0() + secs(s);
            let mut a = pv();
            let p = match kind {
                0 => order(a, at),
                1 => { a.aenderungskennzeichen = Aenderungskennzeichen::Storno; order(a, at) }
                2 => { a.aenderungskennzeichen = Aenderungskennzeichen::Terminverschiebung; a.termin_neu = Some(date("2027-01-29")); order(a, at) }
                3 => Payload::MeldungResponse { control: c(), quittung: teq_ok() },
                4 => Payload::MeldungRequest { control: c(), meldung: meldung(GeschaeftsfallCode::Pv, GeschaeftsfallArt::Endkundenanbieterwechsel, ruem_pv()) },
                5 => Payload::MeldungRequest { control: c(), meldung: meldung(GeschaeftsfallCode::Pv, GeschaeftsfallArt::Endkundenanbieterwechsel, akm_pv()) },
                _ => Payload::MeldungRequest { control: c(), meldung: meldung(GeschaeftsfallCode::Pv, GeschaeftsfallArt::Endkundenanbieterwechsel, erlm_k()) },
            };
            let _ = le.receive(&p, at);
            let _ = ab.receive(&p, at);
        }
        let _ = le.advance(Timestamp::MAX);
        let _ = ab.advance(Timestamp::MAX);
        let end = Timestamp::MAX - secs(1);
        prop_assert!(le.next_due().is_none_or(|t| t > end) && ab.next_due().is_none_or(|t| t > end));
        let debug = format!("{le:?} {ab:?}");
        for secret in PERSONAL {
            prop_assert!(!debug.contains(secret), "{} in Debug", secret);
        }
    }
}
