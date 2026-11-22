//! A simulated losing provider (EKPabg), to test a gaining provider's side without a partner.
//!
//! [`Ekpabg`] is a value: give it what your system sends with the moment it arrives
//! ([`Ekpabg::receive`], or [`Ekpabg::receive_xml`] for the bytes), and it returns what the
//! losing provider sends back, each [`Outbound`] with its time. Its later acts — the answer to
//! the pre-agreement, the confirmation of a cancellation or postponement — fall due after a
//! [`Latency`]: [`Ekpabg::next_due`] says when, [`Ekpabg::advance`] returns them once you advance
//! it that far. It reads no clock and has no randomness: the same inputs give the same outputs.
//!
//! The peer is driven by a [`Process`] in the role EKPabg: it sends only what that process
//! accepts, and answers what it refuses with what the protocol prescribes. One value plays one
//! pre-agreement; keep one per Vorabstimmungs-ID.
//!
//! ```
//! use wbci::sim::{Antwort, Ekpabg};
//! use wbci::*;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let xml = include_str!("../tests/fixtures/va-kue-orn.xml");
//! let payload = Payload::from_xml(xml.as_bytes())?; // what your system sends
//! let signatur = SignaturId { issuer: SignaturText::new("CN=Sandbox")?, serial: SignaturText::new("1")? };
//! let mut peer = Ekpabg::new(Antwort::Zustimmung, signatur);
//!
//! let sent: Timestamp = "2026-11-23T08:00:00Z".parse()?;
//! let reply = peer.receive(&payload, sent);
//! assert!(matches!(reply.outbound[0].payload, Payload::AnfrageResponse { .. })); // the TEQ, 2 s later
//!
//! // One hour later the RUEM-VA is due.
//! let due = peer.next_due().ok_or("nothing due")?;
//! let answer = peer.advance(due);
//! let Payload::MeldungRequest { meldung: Meldung::RuemVa(ruem), .. } = &answer[0].payload else {
//!     return Err("no agreement".into());
//! };
//! assert_eq!(ruem.positionen[0].position.code, Meldungscode::Zwa); // 8001: the requested date
//! # Ok(())
//! # }
//! ```
//!
//! ## What the peer answers
//!
//! | Inbound | Answer | Source |
//! |---|---|---|
//! | pre-agreement | TEQ `OK`; after [`Latency::answer`] the [`Antwort`]: from your record ([`pruefung::check`]), or agreement (8001/8002 with the earliest permitted date, [`deadline::earliest_wechseltermin`]), or refusal with a code | Anl. 1 V5.4.1 §2.3–§2.6 |
//! | cancellation, postponement | TEQ `OK`; ERLM 8004 / 8003 | Anl. 1 V5.4.1 §2.9–§2.11 |
//! | AKM-TR, receipts | TEQ `OK` for the AKM-TR; nothing for a receipt | Anl. 1 V5.4.1 §2.6.2; WBCI 2.0 §12.3 |
//! | a request the process refuses | TEQ `OK`, then ABBM with the process's code where it applies to an ABBM of that case, else 8116 with the reason | WBCI 2.0 §11.5, §12.3.3 |
//! | a cancellation for no known pre-agreement | TEQ `OK`, ABBM 8140; a postponement: 8116 (8140 does not apply to TVS-VA) | Anl. 1 V5.4.1 §2.9–§2.11 |
//! | an AKM-TR the process refuses | TEQ `OK`, and ABBM-TR where the code applies to it (8140, 8148) | Anl. 1 V5.4.1 §2.6.2 |
//! | bytes that do not read | nothing; [`Rejection::Unreadable`] names the receipt code | WBCI 2.0 §12.3.1 |
//!
//! Interpretations (the documents set the deadlines, not the moment inside them): an answer goes
//! one [`Latency::answer`] after the last event, but never later than one second before its
//! deadline; the receipt goes [`Latency::receipt`] after the call, at most 2 hours (the
//! escalation, Anl. 1 V5.4.1 §2.6.1). A readable call always gets a positive receipt: the receipt
//! confirms the technical receipt, a refusal is a business message (WBCI 2.0 §12.3). With the
//! forced [`Antwort::Zustimmung`], a request whose validation predicts a refusal code is refused
//! with it.
//!
//! Replies, refusals and errors carry codes, ids and the processes' value-free reasons, never a
//! name, address or number.

use core::fmt;

use jiff::Timestamp;

use crate::deadline::{self, DeadlineKind};
use crate::process::{Direction, Message, Open, Process, Refusal, State};
use crate::pruefung::{self, Bestand};
use crate::validate::Severity;
use crate::xml::ReadError;
use crate::{
    Abbm, AbbmTr, Anfrage, Calendar, Control, Erlm, Freitext, Geschaeftsfall, Meldung, Meldungscode, Meldungstyp,
    Payload, Role, RuemVa, SignaturId, Teq, Vorabstimmung,
};

#[doc(inline)]
pub use carrier_core::sim::Latency;
/// The duration type of [`Latency`].
pub use jiff::SignedDuration;

/// The most acts one call applies; a bound, never reached by a working peer.
const MAX_ACTS: usize = 1_000;

/// The receipt's deadline: without it after 2 hours the sender escalates (Anl. 1 V5.4.1 §2.6.1).
const RECEIPT_MAX: SignedDuration = SignedDuration::from_hours(2);

/// How the losing provider answers a pre-agreement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Antwort {
    /// Decide from your record of the contract, as [`pruefung::check`] does.
    Pruefung(Box<Bestand>),
    /// Agree: 8001 for the requested date or 8002 for the earliest permitted later one.
    Zustimmung,
    /// Refuse with this code (8116 carries a reason).
    Ablehnung(Meldungscode),
}

/// A payload the peer sends, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outbound {
    /// The moment it is sent (also its control timestamp).
    pub at: Timestamp,
    /// The payload.
    pub payload: Payload,
}

/// Why the peer did not accept an inbound call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// The bytes do not read; [`ReadError::teq_code`] is the receipt code to answer with.
    Unreadable(ReadError),
    /// The process refuses it; the code is the one the peer answered with.
    Refused(Refusal),
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable(e) => write!(f, "unreadable: {e} (TEQ {})", e.teq_code().as_str()),
            Self::Refused(r) => write!(f, "refused: {r}"),
        }
    }
}

/// What the peer returns for one inbound call.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Reply {
    /// What it sends: acts that fell due before the call, its receipt and, for a refused
    /// request, the refusal.
    pub outbound: Vec<Outbound>,
    /// Why the call was not accepted, if it was not.
    pub rejection: Option<Rejection>,
}

/// The simulated losing provider of one pre-agreement.
///
/// `Debug` shows the configuration's kind, the state and the number of events, never the
/// record or a request's values.
#[derive(Clone, PartialEq, Eq)]
pub struct Ekpabg {
    antwort: Antwort,
    signatur: SignaturId,
    latency: Latency,
    calendar: Calendar,
    process: Option<Process>,
    stopped: Option<Refusal>,
}

impl fmt::Debug for Ekpabg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let antwort = match &self.antwort {
            Antwort::Pruefung(_) => "Pruefung".to_owned(),
            Antwort::Zustimmung => "Zustimmung".to_owned(),
            Antwort::Ablehnung(c) => format!("Ablehnung({})", c.as_str()),
        };
        f.debug_struct("Ekpabg")
            .field("antwort", &antwort)
            .field("latency", &self.latency)
            .field("state", &self.process.as_ref().map(|p| p.state().to_string()))
            .field("events", &self.process.as_ref().map_or(0, |p| p.history().len()))
            .field("stopped", &self.stopped)
            .finish_non_exhaustive()
    }
}

impl Ekpabg {
    /// A peer answering with `antwort`, signing its control data with `signatur`, with the
    /// default [`Latency`] and [`Calendar::wbci`].
    #[must_use]
    pub fn new(antwort: Antwort, signatur: SignaturId) -> Self {
        Self {
            antwort,
            signatur,
            latency: Latency::default(),
            calendar: Calendar::wbci(),
            process: None,
            stopped: None,
        }
    }

    /// Uses `latency` for receipts and answers.
    #[must_use]
    pub const fn with_latency(mut self, latency: Latency) -> Self {
        self.latency = latency;
        self
    }

    /// Counts working days with `calendar`.
    #[must_use]
    pub fn with_calendar(mut self, calendar: Calendar) -> Self {
        self.calendar = calendar;
        self
    }

    /// The process of the pre-agreement, once one was received.
    #[must_use]
    pub const fn process(&self) -> Option<&Process> {
        self.process.as_ref()
    }

    /// Why the peer stopped, if its process refused one of its own acts (never in a working
    /// peer); a stopped peer plans nothing more.
    #[must_use]
    pub const fn stopped(&self) -> Option<&Refusal> {
        self.stopped.as_ref()
    }

    /// When the next act is due, if any.
    #[must_use]
    pub fn next_due(&self) -> Option<Timestamp> {
        self.plan().map(|(at, _)| at)
    }

    /// Sends every act due by `now`, in time order.
    pub fn advance(&mut self, now: Timestamp) -> Vec<Outbound> {
        let mut out = Vec::new();
        for _ in 0..MAX_ACTS {
            let Some((at, meldung)) = self.plan().filter(|(at, _)| *at <= now) else { break };
            let Some(process) = self.process.as_mut() else { break };
            match process.apply(Direction::Sent, at, meldung.clone()) {
                Ok(()) => out.push(self.meldung_out(at, meldung)),
                Err(e) => {
                    self.stopped = Some(e);
                    break;
                }
            }
        }
        out
    }

    /// Receives `payload` at `at`: first sends what fell due before, then answers it.
    pub fn receive(&mut self, payload: &Payload, at: Timestamp) -> Reply {
        let mut reply = Reply { outbound: self.advance(at), rejection: None };
        let receipt_at = at.saturating_add(self.latency.receipt_within(RECEIPT_MAX)).unwrap_or(at);
        let rejection = match payload {
            Payload::AnfrageRequest { anfrage, .. } => self.anfrage(anfrage, at, receipt_at, &mut reply.outbound),
            Payload::MeldungRequest { meldung, .. } => self.meldung(meldung, at, receipt_at, &mut reply.outbound),
            Payload::AnfrageResponse { quittung, .. } | Payload::MeldungResponse { quittung, .. } => {
                self.teq(quittung, at)
            }
        };
        reply.rejection = rejection;
        reply
    }

    /// Reads `xml` and receives it at `at`; bytes that do not read are rejected and answered
    /// with nothing (their operation and ids are unknown).
    pub fn receive_xml(&mut self, xml: &[u8], at: Timestamp) -> Reply {
        match Payload::from_xml(xml) {
            Ok(payload) => self.receive(&payload, at),
            Err(e) => Reply { outbound: self.advance(at), rejection: Some(Rejection::Unreadable(e)) },
        }
    }

    fn control(&self, at: Timestamp) -> Control {
        Control::new(at, self.signatur.clone())
    }

    fn meldung_out(&self, at: Timestamp, meldung: Meldung) -> Outbound {
        Outbound { at, payload: Payload::MeldungRequest { control: self.control(at), meldung } }
    }

    fn answer_at(&self, at: Timestamp) -> Timestamp {
        at.saturating_add(self.latency.answer()).unwrap_or(at)
    }

    fn anfrage(
        &mut self,
        anfrage: &Anfrage,
        at: Timestamp,
        receipt_at: Timestamp,
        out: &mut Vec<Outbound>,
    ) -> Option<Rejection> {
        let receipt = Teq::for_anfrage(anfrage, Meldungscode::Ok);
        let receipt_out = Outbound {
            at: receipt_at,
            payload: Payload::AnfrageResponse { control: self.control(receipt_at), quittung: receipt.clone() },
        };
        let accepted = match (&mut self.process, anfrage) {
            (None, Anfrage::Vorabstimmung(va)) => {
                Process::new(Role::EkpAbg, va.clone(), Direction::Received, at).map(|p| {
                    self.process = Some(p.with_calendar(self.calendar.clone()));
                })
            }
            (None, _) => Err(Refusal {
                code: if matches!(anfrage, Anfrage::Storno(_)) {
                    Meldungscode::VorabstimmungsIdUnbekannt
                } else {
                    Meldungscode::Sonstiges
                },
                reason: "no pre-agreement with this Vorabstimmungs-ID was received".to_owned(),
            }),
            (Some(p), _) => p.apply(Direction::Received, at, anfrage.clone()),
        };
        match accepted {
            Ok(()) => {
                self.own_receipt(receipt_at, receipt);
                out.push(receipt_out);
                None
            }
            Err(refusal) => {
                out.push(receipt_out);
                let mut abbm = refusal_abbm(anfrage, &refusal);
                if let (Anfrage::Terminverschiebung(_), Some(State::Agreed { wechseltermin })) =
                    (anfrage, self.process.as_ref().map(Process::state))
                {
                    // A refused postponement returns the date that stays valid (Anl. 1 V5.4.1 §2.11).
                    abbm.wechseltermin = Some(*wechseltermin);
                }
                out.push(self.meldung_out(self.answer_at(at), abbm.into()));
                Some(Rejection::Refused(refusal))
            }
        }
    }

    fn meldung(
        &mut self,
        meldung: &Meldung,
        at: Timestamp,
        receipt_at: Timestamp,
        out: &mut Vec<Outbound>,
    ) -> Option<Rejection> {
        let receipt = Teq::for_meldung(meldung, Meldungscode::Ok);
        out.push(Outbound {
            at: receipt_at,
            payload: Payload::MeldungResponse { control: self.control(receipt_at), quittung: receipt.clone() },
        });
        let result = match &mut self.process {
            None => Err(Refusal {
                code: Meldungscode::VorabstimmungsIdUnbekannt,
                reason: "no pre-agreement with this Vorabstimmungs-ID was received".to_owned(),
            }),
            Some(p) => p.apply(Direction::Received, at, meldung.clone()),
        };
        match result {
            Ok(()) => {
                self.own_receipt(receipt_at, receipt);
                None
            }
            Err(refusal) => {
                if let Meldung::AkmTr(t) = meldung
                    && refusal.code.applies(Meldungstyp::AbbmTr, t.kopf.geschaeftsfall)
                {
                    let abbm_tr = AbbmTr::ablehnung(t, refusal.code.clone());
                    out.push(self.meldung_out(self.answer_at(at), abbm_tr.into()));
                }
                Some(Rejection::Refused(refusal))
            }
        }
    }

    fn teq(&mut self, teq: &Teq, at: Timestamp) -> Option<Rejection> {
        let result = match &mut self.process {
            None => Err(Refusal {
                code: Meldungscode::VorabstimmungsIdUnbekannt,
                reason: "a receipt, but no pre-agreement was received".to_owned(),
            }),
            Some(p) => p.apply(Direction::Received, at, teq.clone()),
        };
        result.err().map(Rejection::Refused)
    }

    /// Applies the peer's own receipt; a refusal stops the peer.
    fn own_receipt(&mut self, at: Timestamp, receipt: Teq) {
        if let Some(p) = self.process.as_mut()
            && let Err(e) = p.apply(Direction::Sent, at, receipt)
        {
            self.stopped = Some(e);
        }
    }

    /// The next act and its time, derived from the process state.
    fn plan(&self) -> Option<(Timestamp, Meldung)> {
        if self.stopped.is_some() {
            return None;
        }
        let p = self.process.as_ref()?;
        let last = p.history().last()?.at;
        let deadlines = p.deadlines();
        let due = |kind: DeadlineKind| {
            deadlines.iter().filter(|d| d.kind == kind && d.party == Role::EkpAbg).map(|d| d.due).min()
        };
        let when = |deadline: Option<Timestamp>| {
            let at = self.answer_at(last);
            match deadline.and_then(|d| d.checked_sub(SignedDuration::from_secs(1)).ok()) {
                Some(latest) if latest < at => latest.max(last),
                _ => at,
            }
        };
        if *p.state() == State::Requested {
            // The answer follows the peer's positive receipt (Anl. 1 V5.4.1 §2.6.1).
            let deadline = due(DeadlineKind::VaAnswer)?;
            return Some((when(Some(deadline)), self.va_answer(p.vorabstimmung(), p)));
        }
        for open in p.open() {
            match open {
                Open::Storno { id, sender: Role::EkpAuf, teq_at: Some(_), .. } => {
                    let storno = p.history().iter().find_map(|e| match &e.message {
                        Message::Anfrage(Anfrage::Storno(s)) if s.storno_id == *id => Some(s),
                        _ => None,
                    })?;
                    return Some((when(due(DeadlineKind::Answer24h)), Erlm::storno_ok(storno).into()));
                }
                Open::Terminverschiebung { id, teq_at: Some(_), .. } => {
                    let tvs = p.history().iter().find_map(|e| match &e.message {
                        Message::Anfrage(Anfrage::Terminverschiebung(t)) if t.aenderungs_id == *id => Some(t),
                        _ => None,
                    })?;
                    return Some((when(due(DeadlineKind::Answer24h)), Erlm::terminverschiebung_ok(tvs).into()));
                }
                Open::Storno { .. } | Open::Terminverschiebung { .. } => {}
            }
        }
        None
    }

    /// The answer to the pre-agreement, decided by the configured [`Antwort`].
    fn va_answer(&self, va: &Vorabstimmung, p: &Process) -> Meldung {
        let received = p.history().first().map_or(Timestamp::UNIX_EPOCH, |e| e.at);
        let answered = p.history().last().map_or(received, |e| e.at);
        let anfrage = Anfrage::Vorabstimmung(va.clone());
        match &self.antwort {
            Antwort::Pruefung(bestand) => pruefung::check(va, bestand, received, &self.calendar).antwort,
            Antwort::Zustimmung => {
                let codes: Vec<Meldungscode> = anfrage
                    .issues_at(received, &self.calendar)
                    .into_iter()
                    .filter(|i| i.severity == Severity::Error)
                    .filter_map(|i| i.code)
                    .filter(|c| c.applies(Meldungstyp::Abbm, va.geschaeftsfall()))
                    .collect();
                if let Some((first, rest)) = codes.split_first() {
                    let mut abbm = Abbm::ablehnung(&anfrage, first.clone());
                    for c in rest {
                        abbm = abbm.with_position(crate::Position::new(c.clone()));
                    }
                    return with_reason(abbm, "the request does not meet the validation rules").into();
                }
                let date = if va.geschaeftsfall() == Geschaeftsfall::VaRrnp {
                    va.kundenwunschtermin
                } else {
                    let at = self.answer_at(answered);
                    deadline::earliest_wechseltermin(at, va.kundenwunschtermin, &self.calendar).date
                };
                RuemVa::zustimmung(va, date).into()
            }
            Antwort::Ablehnung(code) => {
                with_reason(Abbm::ablehnung(&anfrage, code.clone()), "refused by the simulated losing provider").into()
            }
        }
    }
}

/// The ABBM for a refused request: the process's code where it applies to an ABBM of the
/// request's case, else 8116 with the reason (WBCI 2.0 §11.5).
fn refusal_abbm(anfrage: &Anfrage, refusal: &Refusal) -> Abbm {
    let fall = anfrage.geschaeftsfall();
    if refusal.code.applies(Meldungstyp::Abbm, fall) {
        with_reason(Abbm::ablehnung(anfrage, refusal.code.clone()), &refusal.reason)
    } else {
        with_reason(Abbm::ablehnung(anfrage, Meldungscode::Sonstiges), &refusal.reason)
    }
}

/// 8116 needs a reason (`Begruendung`); the reasons are value-free.
fn with_reason(abbm: Abbm, reason: &str) -> Abbm {
    if abbm.positionen.iter().any(|p| p.position.code == Meldungscode::Sonstiges) {
        let text = Freitext::new(reason).or_else(|_| Freitext::new("Abgelehnt"));
        match text {
            Ok(t) => abbm.with_begruendung(t),
            Err(_) => abbm,
        }
    } else {
        abbm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_receipt_never_leaves_its_deadline() {
        let slow = Latency { receipt: SignedDuration::from_hours(5), answer: SignedDuration::from_hours(1) };
        assert_eq!(slow.receipt_within(RECEIPT_MAX), RECEIPT_MAX);
    }

    #[test]
    fn a_new_peer_plans_nothing() {
        let signatur = SignaturId {
            issuer: crate::SignaturText::new("CN=Sandbox").unwrap(),
            serial: crate::SignaturText::new("1").unwrap(),
        };
        let mut peer = Ekpabg::new(Antwort::Zustimmung, signatur);
        assert_eq!(peer.next_due(), None);
        assert!(peer.advance(Timestamp::MAX).is_empty());
        assert!(peer.process().is_none() && peer.stopped().is_none());
    }
}
