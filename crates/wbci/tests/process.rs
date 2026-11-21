//! The process of one pre-agreement through every path of Anl. 1 V5.4.1 §2.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::*;
use wbci::deadline::DeadlineKind;
use wbci::process::{Action, Message, State, Takeover};
use wbci::*;

fn at(s: &str) -> Timestamp {
    let dt: jiff::civil::DateTime = s.parse().unwrap();
    dt.to_zoned_datetime(calendar::berlin()).unwrap().timestamp()
}

/// The gaining provider's view of a VA-KUE-MRN sent on Monday 23.11.2026 09:00, acknowledged.
fn auf() -> Process {
    let va = va_kue_mrn();
    let mut p = Process::new(Role::EkpAuf, va.clone(), Direction::Sent, at("2026-11-23T09:00")).unwrap();
    p.apply(Direction::Received, at("2026-11-23T09:00:02"), Teq::for_anfrage(&va.into(), Meldungscode::Ok)).unwrap();
    p
}

/// The losing provider's view of the same request, acknowledged.
fn abg() -> Process {
    let va = va_kue_mrn();
    let mut p = Process::new(Role::EkpAbg, va.clone(), Direction::Received, at("2026-11-23T09:00")).unwrap();
    p.apply(Direction::Sent, at("2026-11-23T09:00:02"), Teq::for_anfrage(&va.into(), Meldungscode::Ok)).unwrap();
    p
}

fn agreed(mut p: Process) -> Process {
    let ruem = RuemVa::zustimmung(&va_kue_mrn(), date("2027-01-15"));
    let dir = if p.role() == Role::EkpAuf { Direction::Received } else { Direction::Sent };
    p.apply(dir, at("2026-11-24T10:00"), Meldung::from(ruem)).unwrap();
    p
}

#[test]
fn accepted_switch_with_line_takeover() {
    let mut p = auf();
    assert_eq!(p.state(), &State::Requested);
    let dl = p.deadlines();
    assert_eq!(dl.len(), 1);
    assert_eq!(dl[0].kind, DeadlineKind::VaAnswer);
    assert_eq!(dl[0].party, Role::EkpAbg);

    let ruem = RuemVa::zustimmung(&va_kue_mrn(), date("2027-01-15"));
    p.apply(Direction::Received, at("2026-11-24T10:00"), Meldung::from(ruem.clone())).unwrap();
    assert_eq!(p.state(), &State::Agreed { wechseltermin: date("2027-01-15") });
    assert!(p.next_actions(Role::EkpAuf).contains(&Action::Acknowledge));
    assert!(p.next_actions(Role::EkpAuf).contains(&Action::SendAkmTr));
    let kinds: Vec<_> = p.deadlines().iter().map(|d| d.kind).collect();
    assert!(kinds.contains(&DeadlineKind::AkmTrReminder));
    assert!(kinds.contains(&DeadlineKind::AkmTrEscalation));
    assert!(kinds.contains(&DeadlineKind::TerminverschiebungLatest));

    p.apply(Direction::Sent, at("2026-11-24T10:00:01"), Teq::for_meldung(&ruem.clone().into(), Meldungscode::Ok))
        .unwrap();
    let leitung = UebernahmeLeitung::LineId("DEU.EKP2.1754639821".parse().unwrap());
    let akm = AkmTr::uebernahme(&ruem, vec![leitung], false);
    p.apply(Direction::Sent, at("2026-11-25T08:00"), Meldung::from(akm)).unwrap();
    assert_eq!(p.takeover(), Some(&Takeover::Announced { uebernahme: true, sicherer_hafen: false }));
    assert!(!p.next_actions(Role::EkpAuf).contains(&Action::SendAkmTr));
    assert!(p.deadlines().iter().all(|d| d.kind != DeadlineKind::AkmTrReminder));
}

#[test]
fn rejection_ends_the_process() {
    let mut p = abg();
    let abbm = Abbm::ablehnung(&va_kue_mrn().into(), Meldungscode::Kni);
    p.apply(Direction::Sent, at("2026-11-24T10:00"), Meldung::from(abbm)).unwrap();
    assert_eq!(p.state(), &State::Rejected { code: Meldungscode::Kni });
    assert!(p.state().is_terminal());
    let tvs = tvs();
    assert!(p.apply(Direction::Received, at("2026-11-25T10:00"), Anfrage::from(tvs)).is_err());
}

#[test]
fn cancellation_by_the_gaining_provider_before_the_answer() {
    let mut p = auf();
    let storno = str_auf_auf();
    p.apply(Direction::Sent, at("2026-11-23T11:00"), Anfrage::from(storno.clone())).unwrap();
    assert_eq!(p.open().len(), 1);
    p.apply(Direction::Received, at("2026-11-23T11:00:01"), Teq::for_anfrage(&storno.clone().into(), Meldungscode::Ok))
        .unwrap();
    let answer = p.deadlines().into_iter().find(|d| d.kind == DeadlineKind::Answer24h).unwrap();
    assert_eq!(answer.party, Role::EkpAbg);
    assert_eq!(answer.due, at("2026-11-24T11:00:01"));
    p.apply(Direction::Received, at("2026-11-23T15:00"), Meldung::from(Erlm::storno_ok(&storno))).unwrap();
    assert_eq!(p.state(), &State::Cancelled);
}

#[test]
fn cancellation_by_the_losing_provider_needs_an_agreement() {
    let mut p = abg();
    let storno = str_auf_abg();
    let err = p.apply(Direction::Sent, at("2026-11-23T11:00"), Anfrage::from(storno.clone())).unwrap_err();
    assert_eq!(err.code, Meldungscode::StornoAbgelehnt);
    let mut p = agreed(abg());
    p.apply(Direction::Sent, at("2026-11-26T11:00"), Anfrage::from(storno.clone())).unwrap();
    assert!(p.next_actions(Role::EkpAuf).contains(&Action::AnswerStorno));
    // The EKPauf refuses it; the agreement stands.
    let abbm = Abbm::ablehnung(&storno.into(), Meldungscode::StornoAbgelehnt);
    p.apply(Direction::Received, at("2026-11-26T15:00"), Meldung::from(abbm)).unwrap();
    assert_eq!(p.state(), &State::Agreed { wechseltermin: date("2027-01-15") });
    assert_eq!(p.open(), []);
}

#[test]
fn change_ends_with_a_new_pre_agreement() {
    let mut p = agreed(auf());
    let storno = str_aen_auf();
    p.apply(Direction::Sent, at("2026-12-01T09:00"), Anfrage::from(storno.clone())).unwrap();
    p.apply(Direction::Received, at("2026-12-01T12:00"), Meldung::from(Erlm::storno_ok(&storno))).unwrap();
    assert_eq!(p.state(), &State::Changed { wechseltermin: Some(date("2027-01-15")) });
    assert_eq!(p.next_actions(Role::EkpAuf), vec![Action::Acknowledge, Action::SubmitNewVorabstimmung]);
    assert!(p.deadlines().iter().any(|d| d.kind == DeadlineKind::NewVorabstimmung));
}

#[test]
fn postponement_accepted() {
    let mut p = agreed(auf());
    let t = tvs();
    p.apply(Direction::Sent, at("2026-12-01T09:00"), Anfrage::from(t.clone())).unwrap();
    // A second open postponement is refused.
    let mut second = t.clone();
    second.aenderungs_id = AenderungsId::allocate(&cc("DEU.EKP1"), 2).unwrap();
    let err = p.apply(Direction::Sent, at("2026-12-01T09:05"), Anfrage::from(second)).unwrap_err();
    assert_eq!(err.code, Meldungscode::TerminverschiebungAbgelehnt);
    p.apply(Direction::Received, at("2026-12-01T12:00"), Meldung::from(Erlm::terminverschiebung_ok(&t))).unwrap();
    assert_eq!(p.state(), &State::Agreed { wechseltermin: date("2027-01-29") });
}

#[test]
fn postponement_refused_keeps_the_date() {
    let mut p = agreed(abg());
    let t = tvs();
    p.apply(Direction::Received, at("2026-12-01T09:00"), Anfrage::from(t.clone())).unwrap();
    assert!(p.next_actions(Role::EkpAbg).contains(&Action::AnswerTerminverschiebung));
    let mut abbm = Abbm::ablehnung(&t.into(), Meldungscode::TerminverschiebungAbgelehnt);
    abbm.wechseltermin = Some(date("2027-01-15"));
    p.apply(Direction::Sent, at("2026-12-01T12:00"), Meldung::from(abbm)).unwrap();
    assert_eq!(p.state(), &State::Agreed { wechseltermin: date("2027-01-15") });
}

#[test]
fn postponement_rules() {
    let mut p = agreed(auf());
    let mut earlier = tvs();
    earlier.neuer_kundenwunschtermin = date("2027-01-08");
    assert_eq!(
        p.apply(Direction::Sent, at("2026-12-01T09:00"), Anfrage::from(earlier)).unwrap_err().code,
        Meldungscode::TerminverschiebungAbgelehnt
    );
    // Too late: 3 AT before Fri 15.01.2027 is Tue 12.01.; the request must arrive by Mon 11.01. 23:59.
    let late = p.apply(Direction::Sent, at("2027-01-12T00:00"), Anfrage::from(tvs())).unwrap_err();
    assert!(late.reason.starts_with("too late"), "{late}");
    p.apply(Direction::Sent, at("2027-01-11T23:59"), Anfrage::from(tvs())).unwrap();
}

#[test]
fn porting_only_has_no_takeover() {
    let va = va_rrnp();
    let mut p = Process::new(Role::EkpAuf, va.clone(), Direction::Sent, at("2026-11-23T09:00")).unwrap();
    let mut ruem = RuemVa::zustimmung(&va, va.kundenwunschtermin);
    ruem.kopf.geschaeftsfall = Geschaeftsfall::VaRrnp;
    p.apply(Direction::Received, at("2026-11-24T09:00"), Meldung::from(ruem.clone())).unwrap();
    assert!(!p.next_actions(Role::EkpAuf).contains(&Action::SendAkmTr));
    let akm = AkmTr::uebernahme(&ruem, vec![], false);
    assert!(p.apply(Direction::Sent, at("2026-11-24T10:00"), Meldung::from(akm)).is_err());
}

#[test]
fn negative_receipt_allows_a_resend_with_the_same_id() {
    let va = va_kue_mrn();
    let mut p = Process::new(Role::EkpAuf, va.clone(), Direction::Sent, at("2026-11-23T09:00")).unwrap();
    let anfrage: Anfrage = va.clone().into();
    p.apply(
        Direction::Received,
        at("2026-11-23T09:00:02"),
        Teq::for_anfrage(&anfrage, Meldungscode::DatenstrukturUngueltig),
    )
    .unwrap();
    assert!(p.deadlines().is_empty(), "nothing runs for an undelivered request");
    p.apply(Direction::Sent, at("2026-11-23T09:30"), anfrage.clone()).unwrap();
    p.apply(Direction::Received, at("2026-11-23T09:30:02"), Teq::for_anfrage(&anfrage, Meldungscode::Ok)).unwrap();
    assert_eq!(p.deadlines()[0].kind, DeadlineKind::VaAnswer);
}

#[test]
fn missing_receipt_escalates_after_two_hours() {
    let p = Process::new(Role::EkpAuf, va_kue_mrn(), Direction::Sent, at("2026-11-23T09:00")).unwrap();
    assert_eq!(p.overdue(at("2026-11-23T10:59")), vec![]);
    let overdue = p.overdue(at("2026-11-23T11:00"));
    assert_eq!(overdue.len(), 1);
    assert_eq!(overdue[0].kind, DeadlineKind::TeqEscalation);
}

// wbci-switch/SR-006: a party not entitled to an event is refused and nothing changes.
#[test]
fn postponement_from_the_losing_provider_is_refused() {
    let mut p = agreed(auf());
    let before = p.clone();
    // A TVS-VA is always sent by the EKPauf; receiving one means the other side sent it.
    let err = p.apply(Direction::Received, at("2026-12-01T09:00"), Anfrage::from(tvs())).unwrap_err();
    assert_eq!(err.code, Meldungscode::Sonstiges);
    assert_eq!(p, before);
}

// wbci-switch/SR-006: messages naming another EKP pair are refused.
#[test]
fn foreign_party_is_refused() {
    let mut p = abg();
    let before = p.clone();
    let mut ruem = RuemVa::zustimmung(&va_kue_mrn(), date("2027-01-15"));
    ruem.kopf.ekp.auf = cc("DEU.EKP9");
    assert!(p.apply(Direction::Sent, at("2026-11-24T10:00"), Meldung::from(ruem)).is_err());
    let mut wrong_sender = RuemVa::zustimmung(&va_kue_mrn(), date("2027-01-15"));
    wrong_sender.kopf.absender = cc("DEU.EKP1");
    assert!(p.apply(Direction::Sent, at("2026-11-24T10:00"), Meldung::from(wrong_sender)).is_err());
    assert_eq!(p, before);
}

// wbci-switch/SR-007: a duplicate id is refused with 8115 and leaves state and history alone.
#[test]
fn duplicate_ids_are_refused_with_8115() {
    let mut p = agreed(abg());
    let storno = str_auf_auf();
    p.apply(Direction::Received, at("2026-11-26T09:00"), Anfrage::from(storno.clone())).unwrap();
    p.apply(Direction::Sent, at("2026-11-26T12:00"), Meldung::from(Erlm::storno_ok(&storno))).unwrap();
    let mut p = agreed(abg());
    let va: Anfrage = va_kue_mrn().into();
    let before = p.clone();
    let err = p.apply(Direction::Received, at("2026-11-26T09:00"), va).unwrap_err();
    assert_eq!(err.code, Meldungscode::IdBereitsVerwendet);
    assert_eq!(p, before);
    let t = tvs();
    // The EKPabg's process receives a TVS, then the same TVS again.
    p.apply(Direction::Received, at("2026-12-01T09:00"), Anfrage::from(t.clone())).unwrap();
    p.apply(Direction::Sent, at("2026-12-01T10:00"), Meldung::from(Erlm::terminverschiebung_ok(&t))).unwrap();
    let history = p.history().len();
    let err = p.apply(Direction::Received, at("2026-12-01T11:00"), Anfrage::from(t)).unwrap_err();
    assert_eq!(err.code, Meldungscode::IdBereitsVerwendet);
    assert_eq!(p.history().len(), history);
}

// wbci-switch/SR-008: every accepted event is recorded in order with time, direction, sender.
#[test]
fn history_records_every_accepted_event() {
    let mut p = agreed(auf());
    let ruem = match &p.history()[2].message {
        Message::Meldung(Meldung::RuemVa(r)) => r.clone(),
        other => panic!("{other:?}"),
    };
    p.apply(Direction::Sent, at("2026-11-25T08:00"), Meldung::from(AkmTr::uebernahme(&ruem, vec![], true))).unwrap();
    let h = p.history();
    assert_eq!(h.len(), 4);
    let summary: Vec<_> = h.iter().map(|e| (e.direction, e.sender)).collect();
    assert_eq!(
        summary,
        vec![
            (Direction::Sent, Role::EkpAuf),
            (Direction::Received, Role::EkpAbg),
            (Direction::Received, Role::EkpAbg),
            (Direction::Sent, Role::EkpAuf),
        ]
    );
    assert!(h.windows(2).all(|w| w[0].at <= w[1].at));
    assert!(matches!(h[1].message, Message::Teq(_)));
}

#[test]
fn takeover_refused_and_announced_twice() {
    let mut p = agreed(abg());
    let ruem = RuemVa::zustimmung(&va_kue_mrn(), date("2027-01-15"));
    let akm = AkmTr::uebernahme(&ruem, vec![], false);
    p.apply(Direction::Received, at("2026-11-25T08:00"), Meldung::from(akm.clone())).unwrap();
    let err = p.apply(Direction::Received, at("2026-11-25T09:00"), Meldung::from(akm.clone())).unwrap_err();
    assert_eq!(err.code, Meldungscode::UebernahmeBereitsBestaetigt);
    p.apply(
        Direction::Sent,
        at("2026-11-25T10:00"),
        Meldung::from(AbbmTr::ablehnung(&akm, Meldungscode::LineIdUnbekannt)),
    )
    .unwrap();
    assert_eq!(p.takeover(), Some(&Takeover::Refused { code: Meldungscode::LineIdUnbekannt }));
}

#[test]
fn a_pre_agreement_is_sent_by_the_gaining_provider() {
    assert!(Process::new(Role::EkpAbg, va_kue_mrn(), Direction::Sent, at("2026-11-23T09:00")).is_err());
}
