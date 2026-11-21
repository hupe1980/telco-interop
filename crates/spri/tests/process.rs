//! The order lifecycle: one scenario per transition of the EPKs (Anlage 7 V.13), seen by both
//! sides at once — every event is applied to the sender's process as sent and to the
//! receiver's process as received, and both must agree.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::items_after_statements
)]

mod common;

use common::*;
use spri::deadline::DeadlineKind;
use spri::process::{Action, Direction, Event, Message, Pending, Process, PvAnswer, PvLeg, Role, State};
use spri::*;

use GeschaeftsfallCode as G;
use Meldungstyp as T;

// -- the harness ----------------------------------------------------------------------------

/// The service provider, the network owner and, in a provider switch, the losing provider.
struct World {
    fall: GeschaeftsfallCode,
    order: Auftrag,
    ag: Process,
    le: Process,
    ab: Option<Process>,
    now: Timestamp,
}

fn sender_of(typ: Meldungstyp) -> Role {
    match typ {
        T::ErlmK | T::ZwmAg | T::TbkAg => Role::Auftraggeber,
        T::RuemPv => Role::AbgebenderProvider,
        _ => Role::Leistungserbringer,
    }
}

fn receiver_of(typ: Meldungstyp) -> Role {
    match typ {
        T::ErlmK | T::ZwmAg | T::TbkAg | T::RuemPv => Role::Leistungserbringer,
        T::AkmPv | T::AbmPv | T::ErlmPv | T::EntmPv | T::AbbmPv => Role::AbgebenderProvider,
        _ => Role::Auftraggeber,
    }
}

fn route(msg: &Message) -> (Role, Role) {
    match msg {
        Message::Auftrag(_) => (Role::Auftraggeber, Role::Leistungserbringer),
        Message::Meldung(m) => (sender_of(m.typ()), receiver_of(m.typ())),
        Message::Teq(_) => panic!("use World::teq"),
    }
}

impl World {
    /// The order is sent and acknowledged.
    fn start(order: Auftrag) -> Self {
        let mut w = Self::sent(order);
        w.teq(Role::Leistungserbringer, Role::Auftraggeber, "OK");
        w
    }

    /// The order is sent, not yet acknowledged.
    fn sent(order: Auftrag) -> Self {
        let now = ts("2026-11-23T08:00:00Z");
        let ag = Process::new(Role::Auftraggeber, &order, Direction::Sent, now).unwrap();
        let le = Process::new(Role::Leistungserbringer, &order, Direction::Received, now).unwrap();
        assert_eq!(ag.state(), &State::Sent);
        Self { fall: order.geschaeftsfall.code(), order, ag, le, ab: None, now }
    }

    fn tick(&mut self) -> Timestamp {
        self.now += jiff::SignedDuration::from_mins(1);
        self.now
    }

    fn get(&mut self, role: Role) -> Option<&mut Process> {
        match role {
            Role::Auftraggeber => Some(&mut self.ag),
            Role::Leistungserbringer => Some(&mut self.le),
            Role::AbgebenderProvider => self.ab.as_mut(),
        }
    }

    fn m(&self, inhalt: impl Into<Meldungsinhalt>) -> Meldung {
        self.m_aekz(inhalt, Aenderungskennzeichen::Standard)
    }

    fn m_aekz(&self, inhalt: impl Into<Meldungsinhalt>, aekz: Aenderungskennzeichen) -> Meldung {
        let art = self.order.geschaeftsfall_art;
        let mut m = meldung(self.fall, art, inhalt);
        m.auftragstyp.aenderungskennzeichen = aekz;
        m
    }

    fn storno(&self) -> Auftrag {
        Auftrag { aenderungskennzeichen: Aenderungskennzeichen::Storno, ..self.order.clone() }
    }

    fn tv(&self, termin: &str) -> Auftrag {
        Auftrag {
            aenderungskennzeichen: Aenderungskennzeichen::Terminverschiebung,
            termin_neu: Some(date(termin)),
            ..self.order.clone()
        }
    }

    /// Sends without a receipt; both sides must accept.
    fn send_no_teq(&mut self, msg: impl Into<Message>) {
        let msg = msg.into();
        let (from, to) = route(&msg);
        let at = self.tick();
        self.get(from).unwrap().apply(Direction::Sent, at, msg.clone()).unwrap();
        if let Message::Meldung(m) = &msg
            && m.typ() == T::AkmPv
        {
            self.ab = Some(Process::from_akm_pv(m, at).unwrap());
        } else if let Some(p) = self.get(to) {
            p.apply(Direction::Received, at, msg).unwrap();
        }
    }

    /// Sends and the receiver acknowledges with OK.
    fn send(&mut self, msg: impl Into<Message>) {
        let msg = msg.into();
        let (from, to) = route(&msg);
        self.send_no_teq(msg);
        self.teq(to, from, "OK");
    }

    /// `by` acknowledges to `to` with `code`.
    fn teq(&mut self, by: Role, to: Role, code: &str) {
        let at = self.tick();
        let teq = Teq::new(Meldungscode::parse(code), Some(ext()), None);
        if let Some(p) = self.get(by) {
            p.apply(Direction::Sent, at, teq.clone()).unwrap();
        }
        if let Some(p) = self.get(to) {
            p.apply(Direction::Received, at, teq).unwrap();
        }
    }

    /// Both sides refuse and stay unchanged; returns the sender's reason.
    fn refuse(&mut self, msg: impl Into<Message>) -> String {
        let msg = msg.into();
        let (from, to) = route(&msg);
        let at = self.tick();
        let mut reason = String::new();
        for (role, direction) in [(from, Direction::Sent), (to, Direction::Received)] {
            if let Some(p) = self.get(role) {
                let before = p.clone();
                let r = p.apply(direction, at, msg.clone()).expect_err("must be refused");
                assert_eq!(*p, before, "a refusal changed the process");
                if reason.is_empty() {
                    reason = r.reason;
                }
            }
        }
        reason
    }

    /// The network owner refuses, though the losing provider cannot tell (it sees the leg only).
    /// The ABBM code the network owner answers a refused request with.
    fn answer_le(&mut self, msg: impl Into<Message>) -> Option<Meldungscode> {
        let at = self.tick();
        let before = self.le.clone();
        let r = self.le.apply(Direction::Received, at, msg).expect_err("must be refused");
        assert_eq!(self.le, before);
        r.answer
    }

    fn refuse_le(&mut self, msg: impl Into<Message>, direction: Direction) -> String {
        let at = self.tick();
        let before = self.le.clone();
        let r = self.le.apply(direction, at, msg).expect_err("must be refused");
        assert_eq!(self.le, before);
        r.reason
    }

    fn state(&self) -> &State {
        assert_eq!(self.ag.state(), self.le.state(), "the two sides disagree");
        self.ag.state()
    }

    fn qeb(&mut self) {
        let q = self.m(qeb());
        self.send(q);
    }

    fn abm(&mut self, termin: &str, codes: &[&str]) {
        let a = self.m(abm_at(termin, codes, line_id()));
        self.send(a);
    }

    fn abbm(&mut self, code: &str) {
        let a = self.m(abbm_c(code));
        self.send(a);
    }

    fn erlm(&mut self, code: &str) {
        let e = self.m(erlm_c(code));
        self.send(e);
    }

    fn entm(&mut self) {
        let e = self.m(entm());
        self.send(e);
    }

    fn tam(&mut self) {
        let t = self.m(Meldungsinhalt::Tam(tam()));
        self.send(t);
    }

    fn mtam(&mut self) {
        let t = self.m(Meldungsinhalt::Mtam(tam()));
        self.send(t);
    }

    fn erlm_k(&mut self) {
        let e = self.m(erlm_k());
        self.send(e);
    }

    fn vzm(&mut self) {
        let v = self.m(vzm());
        self.send(v);
    }
}

fn abm_at(termin: &str, codes: &[&str], line: LineId) -> Abm {
    let mut a = abm();
    a.verbindlicher_liefertermin = Wunschtermin::from(date(termin));
    a.positionen = codes.iter().map(|c| PositionAbm::from(pos(c))).collect();
    a.anschluss.line_id = line;
    a
}

fn abbm_c(code: &str) -> Abbm {
    Abbm { positionen: vec![wita_pos(code)], ..abbm() }
}

fn erlm_c(code: &str) -> Erlm {
    Erlm { positionen: vec![wita_pos(code)], ..erlm() }
}

fn ruem(zustimmung: JaNein, antwortcode: &str) -> RuemPv {
    let mut r = ruem_pv();
    r.abgebender_provider.zustimmung = zustimmung;
    r.abgebender_provider.antwortcode = Some(Antwortcode::new(antwortcode).unwrap());
    r
}

fn abbm_pv_m() -> AbbmPv {
    abbm_pv()
}

/// A NEU up to the ABM for 15.01.2027.
fn neu_confirmed() -> World {
    let mut w = World::start(neu());
    w.qeb();
    w.abm("2027-01-15", &["0000"]);
    w
}

// -- the technical receipt (Anl. 7 EPK V.13 p.10) -------------------------------------------

#[test]
fn a_positive_teq_delivers_the_order() {
    let mut w = World::sent(neu());
    assert_eq!(w.le.next_actions(Role::Leistungserbringer), [Action::Acknowledge]);
    w.teq(Role::Leistungserbringer, Role::Auftraggeber, "OK");
    assert_eq!(w.state(), &State::Received);
    assert!(!w.le.next_actions(Role::Leistungserbringer).contains(&Action::Acknowledge));
}

#[test]
fn teq_0991_is_a_retry_of_the_same_order() {
    let mut w = World::sent(neu());
    w.teq(Role::Leistungserbringer, Role::Auftraggeber, "0991");
    assert_eq!(w.state(), &State::Rejected { code: Meldungscode::UEBERLASTET });
    assert!(w.ag.next_actions(Role::Auftraggeber).contains(&Action::Resend));
    // Nothing of the network owner's comes before the resend.
    let q = w.m(qeb());
    assert!(w.refuse(q).contains("not delivered"));
    let again = w.order.clone();
    w.send(again);
    assert_eq!(w.state(), &State::Received);
    assert!(!w.ag.next_actions(Role::Auftraggeber).contains(&Action::Resend));
}

#[test]
fn a_hard_teq_error_asks_for_a_correction() {
    for code in ["0990", "0993", "0995", "0999"] {
        let mut w = World::sent(neu());
        w.teq(Role::Leistungserbringer, Role::Auftraggeber, code);
        assert_eq!(w.state(), &State::Rejected { code: Meldungscode::parse(code) });
        assert!(w.ag.next_actions(Role::Auftraggeber).contains(&Action::Correct), "{code}");
    }
}

#[test]
fn a_negative_teq_undoes_a_storno() {
    let mut w = World::start(neu());
    w.qeb();
    let (ag, le) = (w.ag.clone(), w.le.clone());
    let s = w.storno();
    w.send_no_teq(s);
    assert_eq!(w.ag.pending().len(), 1);
    w.teq(Role::Leistungserbringer, Role::Auftraggeber, "0995");
    assert_eq!(w.ag.state(), ag.state());
    assert_eq!(w.ag.pending(), ag.pending());
    assert_eq!(w.le.pending(), le.pending());
    assert!(w.ag.next_actions(Role::Auftraggeber).contains(&Action::Correct));
    // The Storno can be sent again.
    let s = w.storno();
    w.send(s);
    assert_eq!(w.ag.pending().len(), 1);
}

#[test]
fn a_negative_teq_to_the_network_owner_counts_as_delivered() {
    let mut w = World::start(neu());
    let q = w.m(qeb());
    w.send_no_teq(q);
    w.teq(Role::Auftraggeber, Role::Leistungserbringer, "0999");
    assert_eq!(w.state(), &State::Accepted);
}

#[test]
fn a_teq_without_anything_awaiting_is_refused() {
    let mut w = World::start(neu());
    let before = w.ag.clone();
    let err = w.ag.apply(Direction::Received, w.now, teq_ok()).unwrap_err();
    assert!(err.reason.contains("TEQ"));
    assert_eq!(w.ag, before);
}

#[test]
fn a_later_message_proves_delivery() {
    let mut w = World::sent(neu());
    w.qeb();
    assert_eq!(w.state(), &State::Accepted);
    assert!(!w.ag.deadlines().iter().any(|d| d.kind == DeadlineKind::TeqResponse));
}

// -- NEU (Anl. 7 EPK V.13 p.1) --------------------------------------------------------------

#[test]
fn neu_vzm_before_and_after_the_qeb() {
    let mut w = World::start(neu());
    w.vzm();
    assert_eq!(w.state(), &State::Received);
    w.qeb();
    w.vzm();
    assert_eq!(w.state(), &State::Accepted);
}

#[test]
fn neu_qeb_then_abm() {
    let mut w = World::start(neu());
    w.qeb();
    assert_eq!(w.state(), &State::Accepted);
    let q = w.m(qeb());
    assert!(w.refuse(q).contains("QEB"));
    w.abm("2027-01-15", &["0000"]);
    assert_eq!(w.state(), &State::Confirmed { liefertermin: date("2027-01-15") });
    assert_eq!(w.ag.liefertermin(), Some(date("2027-01-15")));
}

#[test]
fn neu_abbm_before_the_qeb() {
    let mut w = World::start(neu());
    w.abbm("1001");
    assert_eq!(w.state(), &State::Aborted { code: Meldungscode::C1001 });
    assert_eq!(abbm_c("1001").outcome(G::Neu, AbbmPhase::BeforeQeb), AbbmOutcome::Resend);
    assert_eq!(abbm_c("1009").outcome(G::Neu, AbbmPhase::BeforeQeb), AbbmOutcome::Closed);
    let v = w.m(vzm());
    assert!(w.refuse(v).contains("ended"));
}

#[test]
fn neu_abbm_after_the_qeb() {
    let mut w = World::start(neu());
    w.qeb();
    w.abbm("8510");
    assert_eq!(w.state(), &State::Aborted { code: Meldungscode::C8510 });
}

#[test]
fn neu_abbm_after_the_abm() {
    let mut w = neu_confirmed();
    w.abbm("2015");
    assert_eq!(w.state(), &State::Aborted { code: Meldungscode::parse("2015") });
    assert_eq!(abbm_c("2015").outcome(G::Neu, AbbmPhase::AfterAbm), AbbmOutcome::Closed);
}

#[test]
fn neu_repeated_abm_replaces_the_binding_date() {
    for code in ["6012", "8500", "8501", "8502"] {
        let mut w = neu_confirmed();
        w.abm("2027-01-22", &[code]);
        assert_eq!(w.state(), &State::Confirmed { liefertermin: date("2027-01-22") }, "{code}");
    }
    let mut w = neu_confirmed();
    let a = w.m(abm_at("2027-01-22", &["0000"], line_id()));
    assert!(w.refuse(a).contains("repeated ABM"));
}

#[test]
fn neu_vzm_and_zwm_after_the_abm() {
    let mut w = neu_confirmed();
    w.vzm();
    let z = w.m(zwm_le());
    w.send(z);
    let z = w.m(zwm_ag());
    w.send(z);
    assert_eq!(w.state(), &State::Confirmed { liefertermin: date("2027-01-15") });
}

#[test]
fn neu_zwm_needs_the_qeb() {
    let mut w = World::start(neu());
    let z = w.m(zwm_le());
    assert!(w.refuse(z).contains("ZWM-LE"));
    let z = w.m(zwm_ag());
    assert!(w.refuse(z).contains("ZWM-AG"));
}

#[test]
fn neu_erlm_then_entm() {
    let mut w = neu_confirmed();
    let e = w.m(entm());
    assert!(w.refuse(e).contains("ENTM"));
    w.erlm("0010");
    assert_eq!(w.state(), &State::Completed { erledigungstermin: date("2027-01-15") });
    assert_eq!(w.le.next_actions(Role::Leistungserbringer), [Action::Send(T::Entm)]);
    w.entm();
    assert_eq!(w.state(), &State::Closed);
    assert!(w.state().is_terminal());
}

#[test]
fn neu_erlm_needs_the_abm() {
    let mut w = World::start(neu());
    w.qeb();
    let e = w.m(erlm_c("0010"));
    assert!(w.refuse(e).contains("ERLM"));
}

#[test]
fn neu_tam_erlm_k_erlm() {
    let mut w = neu_confirmed();
    w.tam();
    assert!(matches!(w.state(), State::TamOpen { .. }));
    assert!(w.ag.next_actions(Role::Auftraggeber).contains(&Action::Send(T::ErlmK)));
    assert_eq!(w.le.next_actions(Role::Leistungserbringer), [Action::Send(T::Mtam)]);
    let e = w.m(erlm_c("0010"));
    assert!(w.refuse(e).contains("ERLM"));
    w.erlm_k();
    assert_eq!(w.state(), &State::ErlmKCheck);
    w.erlm("0010");
    w.entm();
    assert_eq!(w.state(), &State::Closed);
}

#[test]
fn neu_erlm_k_check_fails_and_the_tam_comes_again() {
    let mut w = neu_confirmed();
    w.tam();
    w.erlm_k();
    w.tam();
    assert!(matches!(w.state(), State::TamOpen { .. }));
}

#[test]
fn neu_erlm_k_needs_a_tam() {
    let mut w = neu_confirmed();
    let e = w.m(erlm_k());
    assert!(w.refuse(e).contains("ERLM-K"));
}

#[test]
fn neu_mtam_then_erlm_k() {
    let mut w = neu_confirmed();
    let m = w.m(Meldungsinhalt::Mtam(tam()));
    assert!(w.refuse(m).contains("MTAM"));
    w.tam();
    w.mtam();
    assert!(matches!(w.state(), State::MtamOpen { .. }));
    w.erlm_k();
    assert_eq!(w.state(), &State::ErlmKCheck);
}

#[test]
fn neu_mtam_then_abbm_1198() {
    let mut w = neu_confirmed();
    w.tam();
    let a = w.m(abbm_c("1198"));
    assert!(w.refuse(a).contains("1198"));
    w.mtam();
    assert_eq!(w.le.next_actions(Role::Leistungserbringer), [Action::Abort(Meldungscode::C1198)]);
    w.abbm("1198");
    assert_eq!(w.state(), &State::Aborted { code: Meldungscode::C1198 });
}

#[test]
fn neu_tam_then_terminverschiebung_and_a_new_abm() {
    let mut w = neu_confirmed();
    w.tam();
    let tv = w.tv("2027-02-01");
    w.send(tv);
    assert!(matches!(w.ag.pending(), [Pending::Terminverschiebung { .. }]));
    w.abm("2027-02-01", &["0000"]);
    assert_eq!(w.state(), &State::Confirmed { liefertermin: date("2027-02-01") });
    assert!(w.ag.pending().is_empty());
}

#[test]
fn neu_tam_then_storno() {
    let mut w = neu_confirmed();
    w.tam();
    // After a TAM the Storno is possible even after the binding date (AH 2.9.5 §3.5).
    w.now = ts("2027-01-20T08:00:00Z");
    let s = w.storno();
    w.send(s);
    let e = w.m_aekz(erlm_c("0012"), Aenderungskennzeichen::Storno);
    w.send(e);
    assert_eq!(w.state(), &State::Cancelled);
    w.entm();
    assert_eq!(w.state(), &State::Closed);
}

// -- NEU Konnektivität (Anl. 7 EPK V.13 p.2) ------------------------------------------------

#[test]
fn konnektivitaet_is_detected_from_a_dummy_line_id() {
    let mut w = World::start(neu());
    w.qeb();
    assert!(!w.ag.is_konnektivitaet());
    let a = w.m(abm_at("2027-01-15", &["0000"], "DEU.NB01.0000000000".parse().unwrap()));
    w.send(a);
    assert!(w.ag.is_konnektivitaet() && w.le.is_konnektivitaet());
    // The build-out ABM does not lead to completion …
    let e = w.m(erlm_c("0010"));
    assert!(w.refuse(e).contains("Konnektivität"));
    // … the TAM 6022 asks for a TV with the real date, the second ABM activates.
    let t = w.m(Meldungsinhalt::Tam(Terminanforderung { positionen: vec![wita_pos("6022")], ..tam() }));
    w.send(t);
    let tv = w.tv("2027-02-01");
    w.send(tv);
    w.abm("2027-02-01", &["0000"]);
    w.erlm("0010");
    assert!(matches!(w.state(), State::Completed { .. }));
}

#[test]
fn konnektivitaet_has_no_tv_before_the_first_abm() {
    let now = ts("2026-11-23T08:00:00Z");
    let order = neu();
    let mut ag = Process::new(Role::Auftraggeber, &order, Direction::Sent, now).unwrap().with_konnektivitaet();
    let art = order.geschaeftsfall_art;
    ag.apply(Direction::Received, now, Teq::new(Meldungscode::Ok, None, None)).unwrap();
    ag.apply(Direction::Received, now, meldung(G::Neu, art, qeb())).unwrap();
    let tv = Auftrag {
        aenderungskennzeichen: Aenderungskennzeichen::Terminverschiebung,
        termin_neu: Some(date("2027-02-01")),
        ..order
    };
    let before = ag.clone();
    let err = ag.apply(Direction::Sent, now, tv.clone()).unwrap_err();
    assert!(err.reason.contains("Konnektivität"), "{err}");
    assert_eq!(ag, before);
    assert!(!ag.next_actions(Role::Auftraggeber).contains(&Action::Terminverschiebung));
    // Without the mark the same TV just replaces the requested date.
    let mut w = World::start(neu());
    w.qeb();
    w.send(tv);
    w.abm("2027-02-01", &["0000"]);
    assert!(w.ag.pending().is_empty());
}

// -- KUE-AG (Anl. 7 EPK V.13 p.3) -----------------------------------------------------------

#[test]
fn kue_ag_runs_qeb_abm_erlm_entm() {
    let mut w = World::start(kue_ag());
    w.vzm();
    w.qeb();
    w.abm("2027-02-01", &["0000"]);
    w.abm("2027-02-02", &["8501"]);
    assert_eq!(w.state(), &State::Confirmed { liefertermin: date("2027-02-02") });
    let t = w.m(Meldungsinhalt::Tam(tam()));
    assert!(w.refuse(t).contains("TAM"));
    let a = w.m(abm_at("2027-02-03", &["8502"], line_id()));
    assert!(w.refuse(a).contains("repeated ABM"), "8502 is not drawn for KUE-AG");
    w.erlm("0010");
    w.entm();
    assert_eq!(w.state(), &State::Closed);
}

#[test]
fn kue_ag_terminverschiebung_and_storno() {
    let mut w = World::start(kue_ag());
    w.qeb();
    w.abm("2027-02-01", &["0000"]);
    let tv = w.tv("2027-02-15");
    w.send(tv);
    w.abm("2027-02-15", &["0000"]);
    let s = w.storno();
    w.send(s);
    let e = w.m_aekz(erlm_c("0010"), Aenderungskennzeichen::Storno);
    w.send(e);
    assert_eq!(w.state(), &State::Cancelled);
}

// -- LAE (Anl. 7 EPK V.13 p.6) --------------------------------------------------------------

#[test]
fn lae_runs_with_tv_and_the_tam_loop_but_no_zwm() {
    let mut w = World::start(lae());
    w.qeb();
    let z = w.m(zwm_le());
    assert!(w.refuse(z).contains("ZWM-LE"));
    w.abm("2027-01-15", &["0011"]);
    // The EPK draws a TV in LAE (S/PRI 4.4 §5.3.2.4 says otherwise; the EPK is followed).
    let tv = w.tv("2027-01-29");
    w.send(tv);
    w.abm("2027-01-29", &["0000"]);
    w.tam();
    w.mtam();
    w.abbm("1198");
    assert_eq!(w.state(), &State::Aborted { code: Meldungscode::C1198 });
}

#[test]
fn lae_abbm_2015_after_the_abm() {
    let mut w = World::start(lae());
    w.qeb();
    w.abm("2027-01-15", &["0000"]);
    w.abbm("2015");
    assert!(matches!(w.state(), State::Aborted { .. }));
}

// -- AEN-LMAE (Anl. 7 EPK V.13 p.5) ---------------------------------------------------------

#[test]
fn aen_lmae_has_storno_but_no_tv_and_no_tam() {
    let mut w = World::start(aen_lmae());
    w.qeb();
    w.abm("2027-01-15", &["0005"]);
    let a = w.m(abm_at("2027-01-22", &["6012"], line_id()));
    assert!(w.refuse(a).contains("repeated ABM"));
    let t = w.m(Meldungsinhalt::Tam(tam()));
    assert!(w.refuse(t).contains("TAM"));
    let tv = w.tv("2027-01-29");
    assert!(w.refuse(tv).contains("no Terminverschiebung"));
    let s = w.storno();
    w.send(s);
    let a = w.m_aekz(abbm_c("1087"), Aenderungskennzeichen::Storno);
    w.send(a);
    assert!(w.ag.pending().is_empty());
    w.erlm("0010");
    w.entm();
    assert_eq!(w.state(), &State::Closed);
}

#[test]
fn aen_lmae_abbm_2015() {
    let mut w = World::start(aen_lmae());
    w.qeb();
    w.abm("2027-01-15", &["0000"]);
    w.abbm("2015");
    assert!(matches!(w.state(), State::Aborted { .. }));
}

// -- EST (Anl. 7 EPK V.13 p.8) --------------------------------------------------------------

fn est_accepted() -> World {
    let mut w = World::start(est());
    let q = w.m(Qeb { stoerungs_nr_leistungserbringer: Some(StoerungsNr::new("LE-9").unwrap()), ..qeb() });
    w.send(q);
    w
}

#[test]
fn est_loop_then_erlm_and_entm() {
    let mut w = est_accepted();
    for inhalt in [
        Meldungsinhalt::TbkLe(tbk()),
        Meldungsinhalt::ZwmLe(zwm_le()),
        Meldungsinhalt::ZwmAg(zwm_ag()),
        Meldungsinhalt::Vzm(vzm()),
    ] {
        let m = w.m(inhalt);
        w.send(m);
    }
    assert_eq!(w.state(), &State::Accepted);
    let a = w.m(abm());
    assert!(w.refuse(a).contains("no ABM"));
    w.erlm("0010");
    assert!(matches!(w.state(), State::Completed { .. }));
    // The ENTM is drawn (p.8) but optional (S/PRI 4.4 §8.2.4): the AG may complain already.
    assert!(w.ag.next_actions(Role::Auftraggeber).contains(&Action::Reklamation));
    w.entm();
    assert_eq!(w.state(), &State::Closed);
}

#[test]
fn est_tam_then_tbk_ag() {
    let mut w = est_accepted();
    w.tam();
    assert!(w.ag.next_actions(Role::Auftraggeber).contains(&Action::Send(T::TbkAg)));
    let t = w.m(Meldungsinhalt::TbkAg(tbk()));
    w.send(t);
    assert_eq!(w.state(), &State::Accepted);
}

#[test]
fn est_tam_then_abbm_7015() {
    let mut w = est_accepted();
    let a = w.m(abbm_c("7015"));
    assert!(w.refuse(a).contains("7015"));
    w.tam();
    assert_eq!(w.le.next_actions(Role::Leistungserbringer), [Action::Abort(Meldungscode::C7015)]);
    w.abbm("7015");
    assert_eq!(w.state(), &State::Aborted { code: Meldungscode::C7015 });
}

#[test]
fn est_storno_without_vzm_and_no_tv() {
    let mut w = est_accepted();
    let tv = w.tv("2026-11-25");
    assert!(w.refuse(tv).contains("no Terminverschiebung"));
    let s = w.storno();
    w.send(s);
    let v = w.m_aekz(vzm(), Aenderungskennzeichen::Storno);
    assert!(w.refuse(v).contains("no VZM to a Storno in EST"));
    let a = w.m(abbm_c("1086"));
    w.send(a);
    assert!(w.ag.pending().is_empty());
    assert_eq!(w.state(), &State::Accepted);
}

#[test]
fn est_reklamation_reopens_the_order() {
    let mut w = est_accepted();
    w.erlm("0010");
    let again = w.order.clone();
    w.send(again);
    assert_eq!(w.state(), &State::Received);
    w.qeb();
    assert_eq!(w.state(), &State::Accepted);
    // Without a Reklamation the same order number is refused; with another ticket too.
    let mut w = est_accepted();
    w.erlm("0010");
    w.entm();
    let Geschaeftsfall::Est(mut e) = w.order.geschaeftsfall.clone() else { panic!() };
    e.reklamation.as_mut().unwrap().stoerungs_nr_leistungserbringer = StoerungsNr::new("LE-1").unwrap();
    let other = Auftrag { geschaeftsfall: Geschaeftsfall::Est(e.clone()), ..w.order.clone() };
    assert!(w.refuse(other).contains("Reklamation"));
    e.reklamation = None;
    let plain = Auftrag { geschaeftsfall: Geschaeftsfall::Est(e), ..w.order.clone() };
    assert!(w.refuse(plain).contains("Reklamation"));
}

// -- DIAG (Anl. 7 EPK V.13 p.9) -------------------------------------------------------------

#[test]
fn diag_get_ends_with_erlm() {
    let mut w = World::start(get());
    let q = w.m(qeb());
    assert!(w.refuse(q).contains("no QEB"));
    let v = w.m(vzm());
    assert!(w.refuse(v).contains("no VZM"));
    let s = w.storno();
    assert!(w.refuse(s).contains("no Storno"));
    w.erlm("0010");
    assert_eq!(w.state(), &State::Closed);
    let e = w.m(entm());
    assert!(w.refuse(e).contains("ENTM"));
}

#[test]
fn diag_set_ends_with_abbm() {
    let mut w = World::start(set());
    w.abbm("7504");
    assert_eq!(w.state(), &State::Aborted { code: Meldungscode::C7504 });
    assert_eq!(abbm_c("7504").outcome(G::Set, AbbmPhase::BeforeQeb), AbbmOutcome::Closed);
}

// -- KUE-LE (Anl. 7 EPK V.13 p.4) -----------------------------------------------------------

#[test]
fn kue_le_kdle_erlm_entm() {
    let at = ts("2026-11-23T08:00:00Z");
    let k = meldung(G::KueLe, GeschaeftsfallArt::Kuendigung, kdle());
    let mut le = Process::from_kdle(Role::Leistungserbringer, &k, Direction::Sent, at).unwrap();
    let mut ag = Process::from_kdle(Role::Auftraggeber, &k, Direction::Received, at).unwrap();
    assert!(Process::from_kdle(Role::Auftraggeber, &k, Direction::Sent, at).is_err());
    assert_eq!(ag.next_actions(Role::Auftraggeber), [Action::Acknowledge]);
    // A negative TEQ to a KDLE is only a hint (AH 2.9.5 §2.6).
    let nok = Teq::new(Meldungscode::C0999, None, None);
    ag.apply(Direction::Sent, at, nok.clone()).unwrap();
    le.apply(Direction::Received, at, nok).unwrap();
    assert_eq!(ag.state(), &State::Confirmed { liefertermin: date("2027-06-30") });
    for (p, dir) in [(&mut ag, Direction::Received), (&mut le, Direction::Sent)] {
        let before = p.clone();
        assert!(p.apply(dir, at, meldung(G::KueLe, GeschaeftsfallArt::Kuendigung, abbm())).is_err());
        assert!(p.apply(dir, at, meldung(G::KueLe, GeschaeftsfallArt::Kuendigung, qeb())).is_err());
        assert_eq!(*p, before);
        p.apply(dir, at, meldung(G::KueLe, GeschaeftsfallArt::Kuendigung, erlm())).unwrap();
        p.apply(dir, at, meldung(G::KueLe, GeschaeftsfallArt::Kuendigung, entm())).unwrap();
        assert_eq!(p.state(), &State::Closed);
    }
    // No Storno or TV in KUE-LE: an order of another business case does not fit the process.
    let mut ag = Process::from_kdle(Role::Auftraggeber, &k, Direction::Received, at).unwrap();
    let storno = Auftrag { aenderungskennzeichen: Aenderungskennzeichen::Storno, ..kue_ag() };
    assert!(ag.apply(Direction::Sent, at, storno).is_err());
    // The shutdown date lies in the future.
    let late =
        meldung(G::KueLe, GeschaeftsfallArt::Kuendigung, Kdle { abschaltungstermin: date("2026-11-23"), ..kdle() });
    assert!(Process::from_kdle(Role::Auftraggeber, &late, Direction::Received, at).is_err());
}

#[test]
fn kdle_aborts_other_open_orders_except_est_and_diag() {
    let w = neu_confirmed();
    assert!(w.ag.is_aborted_by_kdle());
    assert!(!est_accepted().ag.is_aborted_by_kdle());
    assert!(!World::start(get()).ag.is_aborted_by_kdle());
    let mut w = neu_confirmed();
    w.erlm("0010");
    assert!(!w.ag.is_aborted_by_kdle());
}

// -- Storno and Terminverschiebung (Anl. 7 EPK V.13 p.11, p.12) ----------------------------

#[test]
fn storno_needs_the_qeb() {
    let mut w = World::start(neu());
    let s = w.storno();
    assert!(w.refuse(s).contains("after the QEB"));
}

#[test]
fn storno_fails_and_the_order_continues() {
    for code in ["1086", "1087"] {
        let mut w = neu_confirmed();
        let s = w.storno();
        w.send(s);
        assert!(w.le.next_actions(Role::Leistungserbringer).contains(&Action::AnswerStorno));
        // A VZM to the Storno keeps it open.
        let v = w.m_aekz(vzm(), Aenderungskennzeichen::Storno);
        w.send(v);
        assert_eq!(w.ag.pending().len(), 1);
        w.abbm(code);
        assert!(w.ag.pending().is_empty());
        assert_eq!(w.state(), &State::Confirmed { liefertermin: date("2027-01-15") });
        assert_eq!(abbm_c(code).outcome(G::Neu, AbbmPhase::Storno), AbbmOutcome::Continues);
    }
}

#[test]
fn storno_succeeds_before_the_abm() {
    let mut w = World::start(neu());
    w.qeb();
    let s = w.storno();
    w.send(s);
    w.erlm("0012");
    assert_eq!(w.state(), &State::Cancelled);
    let a = w.m(abm());
    assert!(w.refuse(a).contains("ABM"));
    w.entm();
    assert_eq!(w.state(), &State::Closed);
}

#[test]
fn erlm_0012_needs_an_open_storno() {
    let mut w = neu_confirmed();
    let e = w.m(erlm_c("0012"));
    assert!(w.refuse(e).contains("Storno"));
}

#[test]
fn storno_window_closes_36_hours_before_the_binding_date() {
    let mut w = neu_confirmed();
    let window = w.ag.deadlines().into_iter().find(|d| d.kind == DeadlineKind::StornoLatest).unwrap();
    assert_eq!(window.due, ts("2027-01-13T11:00:00Z"));
    w.now = ts("2027-01-13T10:58:00Z");
    let s = w.storno();
    w.send(s);
    w.abbm("1087");
    let s = w.storno();
    assert!(w.refuse(s).contains("too late"));
    let tv = w.tv("2027-02-01");
    assert!(w.refuse(tv).contains("too late"));
}

#[test]
fn terminverschiebung_fails_and_the_order_continues() {
    let mut w = neu_confirmed();
    let tv = w.tv("2027-02-01");
    w.send(tv);
    assert!(w.le.next_actions(Role::Leistungserbringer).contains(&Action::AnswerTerminverschiebung));
    let v = w.m_aekz(vzm(), Aenderungskennzeichen::Terminverschiebung);
    w.send(v);
    w.abbm("1085");
    assert!(w.ag.pending().is_empty());
    assert_eq!(w.state(), &State::Confirmed { liefertermin: date("2027-01-15") });
    // A TV is not answered with an ERLM.
    let tv = w.tv("2027-02-01");
    w.send(tv);
    let e = w.m_aekz(erlm_c("0010"), Aenderungskennzeichen::Terminverschiebung);
    assert!(w.refuse(e).contains("not answered with an ERLM"));
}

#[test]
fn terminverschiebung_rules() {
    let mut w = World::start(neu());
    let tv = w.tv("2027-02-01");
    assert!(w.refuse(tv).contains("after the QEB"));
    w.qeb();
    w.abm("2027-01-15", &["0000"]);
    let same = w.tv("2027-01-15");
    assert!(w.refuse(same).contains("new date"));
    let tv = w.tv("2027-02-01");
    w.send(tv);
    let tv = w.tv("2027-02-08");
    assert!(w.refuse(tv).contains("already open"));
    // An ABM in answer carries Standard.
    let a = w.m_aekz(abm_at("2027-02-01", &["0000"], line_id()), Aenderungskennzeichen::Terminverschiebung);
    assert!(w.refuse(a).contains("Standard"));
}

#[test]
fn storno_and_tv_may_be_open_together() {
    let mut w = neu_confirmed();
    let tv = w.tv("2027-02-01");
    w.send(tv);
    let s = w.storno();
    w.send(s);
    assert_eq!(w.ag.pending().len(), 2);
    // With both open, 1007 under Standard cannot be attributed; the ÄKZ decides.
    let a = w.m_aekz(abbm_c("1007"), Aenderungskennzeichen::Storno);
    w.send(a);
    assert!(matches!(w.ag.pending(), [Pending::Terminverschiebung { .. }]));
}

// -- PV with the losing provider (Anl. 7 EPK V.13 p.7) -------------------------------------

fn pv_agreed() -> World {
    let mut w = World::start(pv());
    w.qeb();
    let akm = w.m(akm_pv());
    w.send(akm);
    let r = w.m(ruem_pv());
    w.send(r);
    w
}

#[test]
fn pv_runs_with_the_losing_provider() {
    let mut w = World::start(pv());
    w.qeb();
    assert_eq!(w.le.next_actions(Role::Leistungserbringer), [Action::Send(T::AkmPv)]);
    let akm = w.m(akm_pv());
    w.send(akm);
    assert_eq!(w.le.pv(), Some(&PvLeg::AwaitRuem { antwortfrist: date("2026-12-01") }));
    let ab = w.ab.as_ref().unwrap();
    assert_eq!(ab.role(), Role::AbgebenderProvider);
    assert_eq!(ab.next_actions(Role::AbgebenderProvider), [Action::Send(T::RuemPv)]);
    assert!(ab.deadlines().iter().any(|d| d.kind == DeadlineKind::RuemPv));
    assert_eq!(w.ag.pv(), None, "the service provider does not see the leg");
    let r = w.m(ruem_pv());
    w.send(r);
    assert_eq!(w.le.pv(), Some(&PvLeg::Agreed));
    w.abm("2027-01-15", &["0000"]);
    assert!(w.le.next_actions(Role::Leistungserbringer).contains(&Action::Send(T::AbmPv)));
    let a = w.m(abm_pv());
    w.send(a);
    assert_eq!(w.le.pv(), Some(&PvLeg::Confirmed { uebernahmedatum: date("2027-01-15") }));
    assert_eq!(w.ab.as_ref().unwrap().state(), &State::Confirmed { liefertermin: date("2027-01-15") });
    w.erlm("0010");
    assert!(w.le.next_actions(Role::Leistungserbringer).contains(&Action::Send(T::ErlmPv)));
    let e = w.m(erlm_pv());
    w.send(e);
    w.entm();
    let e = w.m(entm_pv());
    w.send(e);
    assert_eq!(w.state(), &State::Closed);
    assert_eq!(w.le.pv(), Some(&PvLeg::Closed));
    assert_eq!(w.ab.as_ref().unwrap().state(), &State::Closed);
}

#[test]
fn pv_abm_needs_the_positive_ruem_pv() {
    let mut w = World::start(pv());
    w.qeb();
    let akm = w.m(akm_pv());
    w.send(akm);
    let at = w.tick();
    let before = w.le.clone();
    let err = w.le.apply(Direction::Sent, at, w.m(abm())).unwrap_err();
    assert!(err.reason.contains("RUEM-PV"), "{err}");
    assert_eq!(w.le, before);
    let a = w.m(abm_pv());
    assert!(w.refuse(a).contains("ABM-PV"));
    let akm = w.m(akm_pv());
    let at = w.tick();
    assert!(w.le.apply(Direction::Sent, at, akm).unwrap_err().reason.contains("one AKM-PV"));
}

#[test]
fn pv_abm_pv_comes_with_an_abm() {
    let mut w = pv_agreed();
    let a = w.m(abm_pv());
    assert!(w.refuse_le(a, Direction::Sent).contains("together with an ABM"));
}

#[test]
fn pv_negative_ruem_pv() {
    for (zustimmung, code) in [(JaNein::Nein, "0021"), (JaNein::Ja, "LNB"), (JaNein::Ja, "KVE"), (JaNein::Ja, "TAV")] {
        let mut w = World::start(pv());
        w.qeb();
        let akm = w.m(akm_pv());
        w.send(akm);
        let r = w.m(ruem(zustimmung, code));
        w.send(r);
        assert!(matches!(w.le.pv(), Some(PvLeg::Refused { .. })), "{code}");
        let le = w.le.next_actions(Role::Leistungserbringer);
        assert!(le.contains(&Action::Abort(Meldungscode::C0041)) && le.contains(&Action::Send(T::AbbmPv)));
        w.abbm("0041");
        let a = w.m(abbm_pv_m());
        w.send(a);
        assert_eq!(w.state(), &State::Aborted { code: Meldungscode::C0041 });
        assert_eq!(w.le.pv(), Some(&PvLeg::Aborted { answer: PvAnswer::Negative }));
        assert!(matches!(w.ab.as_ref().unwrap().state(), State::Aborted { .. }));
    }
}

#[test]
fn pv_missed_answer() {
    let mut w = World::start(pv());
    w.qeb();
    let akm = w.m(akm_pv());
    w.send(akm);
    // Before the antwortfrist ends, ABBM 0042 is refused.
    let a = w.m(abbm_c("0042"));
    let at = w.tick();
    assert!(w.le.apply(Direction::Sent, at, a.clone()).unwrap_err().reason.contains("0042"));
    let end = ts("2026-12-01T23:00:00Z"); // 02.12. 0:00 Berlin
    assert!(w.le.overdue(ts("2026-12-01T22:59:59Z")).is_empty());
    let overdue = w.le.overdue(end);
    assert_eq!(overdue.len(), 1);
    assert_eq!(overdue[0].kind, DeadlineKind::RuemPv);
    assert!(overdue[0].rule.text.contains("ABBM 0042"));
    // A late RUEM-PV is refused.
    w.now = end;
    let r = w.m(ruem_pv());
    assert!(w.refuse(r).contains("antwortfrist"));
    w.abbm("0042");
    assert_eq!(w.le.pv(), Some(&PvLeg::Missed));
    assert!(w.le.next_actions(Role::Leistungserbringer).contains(&Action::Send(T::AbbmPv)));
    let a = w.m(abbm_pv_m());
    w.send(a);
    assert_eq!(w.le.pv(), Some(&PvLeg::Aborted { answer: PvAnswer::Missing }));
}

#[test]
fn pv_abbm_pv_may_come_first_after_the_antwortfrist() {
    let mut w = World::start(pv());
    w.qeb();
    let akm = w.m(akm_pv());
    w.send(akm);
    w.now = ts("2026-12-02T08:00:00Z");
    let a = w.m(abbm_pv_m());
    w.send(a);
    w.abbm("0042");
    assert_eq!(w.state(), &State::Aborted { code: Meldungscode::C0042 });
}

#[test]
fn pv_storno_after_the_akm_pv_informs_the_losing_provider() {
    let mut w = pv_agreed();
    let a = w.m(abbm_pv_m());
    assert!(w.refuse_le(a, Direction::Sent).contains("ABBM-PV"));
    let s = w.storno();
    w.send(s);
    w.erlm("0012");
    assert_eq!(w.state(), &State::Cancelled);
    assert!(w.le.next_actions(Role::Leistungserbringer).contains(&Action::Send(T::AbbmPv)));
    // The losing provider's messages are discarded now.
    let a = w.m(abbm_pv_m());
    w.send(a);
    assert_eq!(w.le.pv(), Some(&PvLeg::Aborted { answer: PvAnswer::Positive }));
}

#[test]
fn pv_storno_before_the_akm_pv_needs_no_abbm_pv() {
    let mut w = World::start(pv());
    w.qeb();
    let s = w.storno();
    w.send(s);
    w.erlm("0012");
    assert_eq!(w.le.pv(), None);
    assert!(!w.le.next_actions(Role::Leistungserbringer).contains(&Action::Send(T::AbbmPv)));
    let akm = w.m(akm_pv());
    let at = w.tick();
    assert!(w.le.apply(Direction::Sent, at, akm).is_err());
}

#[test]
fn pv_ruem_pv_after_the_order_ended_is_discarded() {
    let mut w = World::start(pv());
    w.qeb();
    let akm = w.m(akm_pv());
    w.send(akm);
    let s = w.storno();
    w.send(s);
    w.erlm("0012");
    let r = w.m(ruem_pv());
    assert!(w.refuse_le(r, Direction::Received).contains("discarded"));
}

#[test]
fn pv_abbm_2015_and_abbm_pv_after_the_abm() {
    let mut w = pv_agreed();
    w.abm("2027-01-15", &["0000"]);
    let a = w.m(abm_pv());
    w.send(a);
    w.abbm("2015");
    let a = w.m(abbm_pv_m());
    w.send(a);
    assert_eq!(w.le.pv(), Some(&PvLeg::Aborted { answer: PvAnswer::Positive }));
}

#[test]
fn pv_erlm_k_directly_after_the_abm_and_the_tam_loop() {
    let mut w = pv_agreed();
    w.abm("2027-01-15", &["0000"]);
    let a = w.m(abm_pv());
    w.send(a);
    w.erlm_k();
    assert_eq!(w.state(), &State::ErlmKCheck);
    w.tam();
    w.mtam();
    w.abbm("1198");
    // The EPK does not draw the ABBM-PV on the TAM path; it is expected anyway.
    assert!(w.le.next_actions(Role::Leistungserbringer).contains(&Action::Send(T::AbbmPv)));
}

#[test]
fn pv_tv_moves_later_only_and_repeats_abm_and_abm_pv() {
    let mut w = pv_agreed();
    w.abm("2027-01-15", &["0000"]);
    let a = w.m(abm_pv());
    w.send(a);
    let tv = w.tv("2027-01-10");
    assert!(w.refuse(tv).contains("earlier"));
    let tv = w.tv("2027-01-29");
    w.send(tv);
    w.abm("2027-01-29", &["0000"]);
    let a = w.m(AbmPv {
        aufnehmender_provider: ProviderwechselBestaetigung {
            uebernahmedatum_verbindlich: date("2027-01-29"),
            ..abm_pv().aufnehmender_provider
        },
        ..abm_pv()
    });
    w.send(a);
    assert_eq!(w.le.pv(), Some(&PvLeg::Confirmed { uebernahmedatum: date("2027-01-29") }));
}

#[test]
fn pv_abbm_0041_needs_a_refusal() {
    let mut w = pv_agreed();
    let a = w.m(abbm_c("0041"));
    let at = w.tick();
    assert!(w.le.apply(Direction::Sent, at, a).unwrap_err().reason.contains("0041"));
}

// -- deadlines -------------------------------------------------------------------------------

#[test]
fn deadlines_follow_the_state() {
    let w = World::sent(neu());
    let d = w.ag.deadlines();
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].kind, DeadlineKind::TeqResponse);
    // The NEU carries an attachment: 120 seconds.
    assert_eq!(d[0].due, ts("2026-11-23T08:02:00Z"));
    assert_eq!(d[0].party, Role::Leistungserbringer);
    assert_eq!(w.ag.overdue(ts("2026-11-23T08:02:00Z")).len(), 1);
    let w = World::sent(lae());
    assert_eq!(w.ag.deadlines()[0].due, ts("2026-11-23T08:00:30Z"));

    let mut w = neu_confirmed();
    let kinds: Vec<_> = w.ag.deadlines().iter().map(|d| d.kind).collect();
    assert_eq!(kinds, [DeadlineKind::StornoLatest, DeadlineKind::TerminverschiebungLatest]);
    assert!(w.ag.overdue(ts("2027-02-01T00:00:00Z")).is_empty(), "windows are not obligations");
    w.now = ts("2026-11-27T10:00:00Z"); // Friday
    w.tam();
    let d = w.le.deadlines();
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].kind, DeadlineKind::MtamDue);
    // 10 Werktage after Fr 27.11.: Sa 28.11. … Mi 09.12.
    assert_eq!(d[0].due, ts("2026-12-09T23:00:00Z"));
    assert!(d[0].explanation.counted.contains(&date("2026-11-28")));
    w.mtam();
    assert_eq!(w.le.deadlines()[0].kind, DeadlineKind::Abbm1198Due);
    let closed = Calendar::werktage().with_closed(date("2026-11-28"));
    let p = w.le.clone().with_calendar(closed);
    assert!(p.deadlines()[0].due > w.le.deadlines()[0].due);
}

#[test]
fn refusals_name_the_rule_without_values() {
    let mut w = World::start(neu());
    let a = w.m(abm());
    let reason = w.refuse(a);
    assert_eq!(reason, "the QEB comes before the ABM (S/PRI 4.4 §8.2.2)");
    for secret in ["A-2026-0001", "4711", "V123456", "DEU.NB01", "2027-01-15"] {
        assert!(!reason.contains(secret), "{reason}");
    }
    let text = format!("{:?}", w.ag);
    for secret in ["Erika", "Mustermann", "Hauptstraße", "1234567"] {
        assert!(!text.contains(secret), "Debug leaks {secret}");
    }
}

#[test]
fn a_process_starts_from_a_standard_order_of_the_service_provider() {
    let at = ts("2026-11-23T08:00:00Z");
    assert!(Process::new(Role::Leistungserbringer, &neu(), Direction::Sent, at).is_err());
    assert!(Process::new(Role::AbgebenderProvider, &neu(), Direction::Received, at).is_err());
    let storno = Auftrag { aenderungskennzeichen: Aenderungskennzeichen::Storno, ..neu() };
    assert!(Process::new(Role::Auftraggeber, &storno, Direction::Sent, at).is_err());
    let m = meldung(G::Neu, GeschaeftsfallArt::Bereitstellung, qeb());
    assert!(Process::from_akm_pv(&m, at).is_err());
    assert!(Process::from_kdle(Role::Auftraggeber, &m, Direction::Received, at).is_err());
    // Another order number does not belong to the process.
    let mut w = World::start(neu());
    let other = Auftrag {
        externe_auftragsnummer: ExterneAuftragsnummer::new("A-2026-0002").unwrap(),
        aenderungskennzeichen: Aenderungskennzeichen::Storno,
        ..neu()
    };
    assert!(w.refuse(other).contains("another order number"));
    let again = w.order.clone();
    assert!(w.refuse(again).contains("already sent"));
}

// -- security --------------------------------------------------------------------------------

// spri-lifecycle/SR-001: out-of-order, wrong-sender and wrong-case events are refused and the
// process keeps its value.
#[test]
fn out_of_order_and_wrong_sender_are_refused_unchanged() {
    // ABM before the QEB.
    let mut w = World::start(neu());
    let before = (w.ag.clone(), w.le.clone());
    let a = w.m(abm());
    assert!(w.refuse(a).contains("QEB comes before the ABM"));
    assert_eq!((w.ag.clone(), w.le.clone()), before);

    // An ABM sent by the ordering party.
    let mut w = World::start(neu());
    w.qeb();
    let p = w.ag.clone();
    let before = p.clone();
    let mut p2 = p;
    let err = p2.apply(Direction::Sent, w.now, w.m(abm())).unwrap_err();
    assert!(err.reason.contains("sent by the Leistungserbringer"), "{err}");
    assert_eq!(p2, before);
    // … and a QEB received by the network owner.
    let before = w.le.clone();
    assert!(w.le.apply(Direction::Received, w.now, w.m(qeb())).is_err());
    assert_eq!(w.le, before);

    // Anything after the ENTM.
    let mut w = neu_confirmed();
    w.erlm("0010");
    w.entm();
    let msgs: Vec<Message> = vec![
        w.m(qeb()).into(),
        w.m(abm()).into(),
        w.m(abbm_c("2015")).into(),
        w.m(erlm_c("0010")).into(),
        w.m(entm()).into(),
        w.m(vzm()).into(),
        w.m(Meldungsinhalt::Tam(tam())).into(),
        w.m(erlm_k()).into(),
        w.m(zwm_ag()).into(),
        w.storno().into(),
        w.tv("2027-02-01").into(),
        w.order.clone().into(),
    ];
    for m in msgs {
        let (ag, le) = (w.ag.clone(), w.le.clone());
        w.refuse(m);
        assert_eq!(w.ag, ag);
        assert_eq!(w.le, le);
    }

    // A second open Storno.
    let mut w = neu_confirmed();
    let s = w.storno();
    w.send(s);
    let p = w.ag.clone();
    let s = w.storno();
    assert!(w.refuse(s).contains("already open"));
    assert_eq!(w.ag, p);

    // A TV in AEN-LMAE.
    let mut w = World::start(aen_lmae());
    w.qeb();
    let p = w.ag.clone();
    let tv = w.tv("2027-02-01");
    assert!(w.refuse(tv).contains("no Terminverschiebung in AEN-LMAE"));
    assert_eq!(w.ag, p);

    // A message of another business case or order number.
    let mut w = World::start(neu());
    let other = meldung(G::Lae, GeschaeftsfallArt::Aenderung, qeb());
    assert!(w.refuse(other).contains("another business case"));
    let q = w.m(Qeb { externe_auftragsnummer: ExterneAuftragsnummer::new("A-2026-0002").unwrap(), ..qeb() });
    assert!(w.refuse(q).contains("another order number"));

    // The service provider never sees the provider-switch leg.
    let mut w = World::start(pv());
    w.qeb();
    let before = w.ag.clone();
    assert!(w.ag.apply(Direction::Received, w.now, w.m(akm_pv())).is_err());
    assert!(w.ag.apply(Direction::Sent, w.now, w.m(ruem_pv())).is_err());
    assert_eq!(w.ag, before);
}

// spri-lifecycle/SR-002: a provider switch's history keeps every accepted event in order, with
// time, direction and sender.
#[test]
fn pv_history_is_complete_and_ordered() {
    let mut w = World::start(pv());
    w.qeb();
    let akm = w.m(akm_pv());
    w.send(akm);
    let r = w.m(ruem_pv());
    w.send(r);
    w.abm("2027-01-15", &["0000"]);
    let a = w.m(abm_pv());
    w.send(a);
    // A refused event leaves no trace.
    let e = w.m(entm());
    w.refuse(e);

    let summary = |h: &[Event]| -> Vec<(Direction, Role, String)> {
        h.iter()
            .map(|e| {
                let what = match &e.message {
                    Message::Auftrag(_) => "PV".to_owned(),
                    Message::Meldung(m) => m.typ().to_string(),
                    Message::Teq(t) => format!("TEQ {}", t.positionen[0].code.as_str()),
                };
                (e.direction, e.sender, what)
            })
            .collect()
    };
    use Direction::{Received as R, Sent as S};
    use Role::{AbgebenderProvider as AB, Auftraggeber as AG, Leistungserbringer as LE};
    let le = summary(w.le.history());
    let expected = [
        (R, AG, "PV"),
        (S, LE, "TEQ OK"),
        (S, LE, "QEB"),
        (R, AG, "TEQ OK"),
        (S, LE, "AKM-PV"),
        (R, AB, "TEQ OK"),
        (R, AB, "RUEM-PV"),
        (S, LE, "TEQ OK"),
        (S, LE, "ABM"),
        (R, AG, "TEQ OK"),
        (S, LE, "ABM-PV"),
        (R, AB, "TEQ OK"),
    ];
    assert_eq!(le, expected.map(|(d, s, w)| (d, s, w.to_owned())));
    let times: Vec<Timestamp> = w.le.history().iter().map(|e| e.at).collect();
    assert!(times.windows(2).all(|p| p[0] < p[1]), "in order of time");
    assert_eq!(times[0], ts("2026-11-23T08:00:00Z"));

    // The service provider's view holds its own exchange only.
    let ag: Vec<String> = summary(w.ag.history()).into_iter().map(|(_, _, w)| w).collect();
    assert_eq!(ag, ["PV", "TEQ OK", "QEB", "TEQ OK", "ABM", "TEQ OK"]);
    // The losing provider's view holds the leg only.
    let ab = summary(w.ab.as_ref().unwrap().history());
    assert_eq!(
        ab,
        [
            (R, LE, "AKM-PV"),
            (S, AB, "TEQ OK"),
            (S, AB, "RUEM-PV"),
            (R, LE, "TEQ OK"),
            (R, LE, "ABM-PV"),
            (S, AB, "TEQ OK")
        ]
        .map(|(d, s, w)| (d, s, w.to_owned()))
    );
}

/// A refused Storno or TV tells the network owner which ABBM to answer with.
#[test]
fn refused_requests_name_the_abbm_to_answer_with() {
    let mut w = World::start(neu());
    let tv = w.tv("2027-02-01");
    assert_eq!(w.answer_le(tv), Some(Meldungscode::C1085), "TV before the QEB");
    w.qeb();
    w.abm("2027-01-15", &["0000"]);
    let s = w.storno();
    w.send(s);
    let s = w.storno();
    assert_eq!(w.answer_le(s), Some(Meldungscode::C1086), "a second Storno");
}
