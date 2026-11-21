//! The life of one pre-agreement, as one party sees it.
//!
//! A [`Process`] is created from a pre-agreement request and fed every request, message and
//! receipt that is sent or received for it, with its time. It checks that the event is
//! allowed — right party, right order, no reused id, within the lead times — and answers
//! with the code the protocol prescribes if not. It reports its state, what each side may do
//! next, the open deadlines and its history. It performs no I/O and reads no clock.
//!
//! Sources: Anl. 1 V5.4.1 §2.2–§2.11, AH4WBCI 2.10 §2, WBCI 2.0 §12.3.

use core::fmt;
use std::collections::BTreeSet;

use jiff::Timestamp;
use jiff::civil::Date;

use crate::deadline::{self, Deadline, DeadlineKind};
use crate::model::{Anfrage, Meldung, Role, StornoArt, Teq, Vorabstimmung};
use crate::{AenderungsId, Calendar, Ekp, Geschaeftsfall, Meldungscode, StornoId};

/// Whether an event was sent by the owner of the process or received from the other side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Sent by the owner.
    Sent,
    /// Received from the other side.
    Received,
}

/// What travels between the two parties.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum Message {
    /// A request.
    Anfrage(Anfrage),
    /// A business message.
    Meldung(Meldung),
    /// A technical receipt.
    Teq(Teq),
}

impl From<Anfrage> for Message {
    fn from(a: Anfrage) -> Self {
        Self::Anfrage(a)
    }
}
impl From<Meldung> for Message {
    fn from(m: Meldung) -> Self {
        Self::Meldung(m)
    }
}
impl From<Teq> for Message {
    fn from(t: Teq) -> Self {
        Self::Teq(t)
    }
}

/// One accepted event in the history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// When it was sent or received.
    pub at: Timestamp,
    /// Sent or received.
    pub direction: Direction,
    /// The party that sent it.
    pub sender: Role,
    /// What it was.
    pub message: Message,
}

/// Where a pre-agreement stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Requested; no answer yet.
    Requested,
    /// Agreed: the switch happens on `wechseltermin`.
    Agreed {
        /// The binding switch date.
        wechseltermin: Date,
    },
    /// Refused with an ABBM (terminal).
    Rejected {
        /// The first code of the ABBM.
        code: Meldungscode,
    },
    /// Cancelled for good (STR-AUF confirmed; terminal).
    Cancelled,
    /// Cancelled for change (STR-AEN confirmed): the EKPauf submits a new pre-agreement with
    /// a new id by 7 AT before `wechseltermin` (terminal for this id).
    Changed {
        /// The switch date that was agreed.
        wechseltermin: Option<Date>,
    },
}

impl State {
    /// Whether nothing more can happen in this process.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Rejected { .. } | Self::Cancelled | Self::Changed { .. })
    }
}

/// The takeover of the line (AKM-TR).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Takeover {
    /// The EKPauf sent an AKM-TR.
    Announced {
        /// Whether the line is taken over.
        uebernahme: bool,
        /// Whether the „sicherer Hafen" is requested.
        sicherer_hafen: bool,
    },
    /// The EKPabg refused it with an ABBM-TR.
    Refused {
        /// The first code of the ABBM-TR.
        code: Meldungscode,
    },
}

/// An open cancellation or postponement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Open {
    /// A cancellation awaiting ERLM or ABBM.
    Storno {
        /// Its id.
        id: StornoId,
        /// Aufhebung (`true`) or Änderung.
        aufhebung: bool,
        /// Who sent it.
        sender: Role,
        /// When its positive receipt came.
        teq_at: Option<Timestamp>,
    },
    /// A postponement awaiting ERLM or ABBM.
    Terminverschiebung {
        /// Its id.
        id: AenderungsId,
        /// The requested new date.
        termin: Date,
        /// When its positive receipt came.
        teq_at: Option<Timestamp>,
    },
}

/// Something a party may do next.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Action {
    /// Send the receipt (TEQ) for something received.
    Acknowledge,
    /// Answer the pre-agreement with RUEM-VA or ABBM.
    AnswerVorabstimmung,
    /// Announce the takeover with AKM-TR.
    SendAkmTr,
    /// Refuse the AKM-TR with ABBM-TR.
    RefuseAkmTr,
    /// Cancel for good (STR-AUF).
    SendStornoAufhebung,
    /// Cancel for change (STR-AEN).
    SendStornoAenderung,
    /// Postpone (TVS-VA).
    SendTerminverschiebung,
    /// Answer the open cancellation with ERLM or ABBM.
    AnswerStorno,
    /// Answer the open postponement with ERLM or ABBM.
    AnswerTerminverschiebung,
    /// Submit a new pre-agreement with a new id (after an agreed change).
    SubmitNewVorabstimmung,
}

/// Why an event was refused, with the code to answer with.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{code}: {reason}")]
pub struct Refusal {
    /// The code: for a received request, the ABBM code to answer with.
    pub code: Meldungscode,
    /// Why, without personal data.
    pub reason: String,
}

fn refuse<T>(code: Meldungscode, reason: impl Into<String>) -> Result<T, Refusal> {
    Err(Refusal { code, reason: reason.into() })
}

/// The mutable part, snapshotted before each sent item so a negative receipt can undo it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Core {
    state: State,
    va_teq_at: Option<Timestamp>,
    open: Vec<Open>,
    takeover: Option<Takeover>,
    ids: BTreeSet<String>,
}

/// An item awaiting its receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Awaiting {
    sender: Role,
    at: Timestamp,
    storno: Option<StornoId>,
    aenderung: Option<AenderungsId>,
    is_va: bool,
    before: Box<Core>,
}

/// One pre-agreement from the point of view of one party.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    role: Role,
    vorabstimmung: Vorabstimmung,
    calendar: Calendar,
    shared_network_operator: bool,
    core: Core,
    awaiting: Vec<Awaiting>,
    history: Vec<Event>,
}

impl Process {
    /// Starts the process of `vorabstimmung`, sent (by the EKPauf) or received (by the EKPabg)
    /// at `at`. `role` is the owner's side. Uses [`Calendar::wbci`].
    ///
    /// # Errors
    ///
    /// The direction contradicts the role: only the EKPauf sends a pre-agreement.
    pub fn new(role: Role, vorabstimmung: Vorabstimmung, direction: Direction, at: Timestamp) -> Result<Self, Refusal> {
        let sender = match direction {
            Direction::Sent => role,
            Direction::Received => role.other(),
        };
        if sender != Role::EkpAuf {
            return refuse(Meldungscode::Sonstiges, "a pre-agreement is sent by the EKPauf");
        }
        let core = Core {
            state: State::Requested,
            va_teq_at: None,
            open: Vec::new(),
            takeover: None,
            ids: BTreeSet::from([vorabstimmung.id.as_str().to_owned()]),
        };
        let awaiting =
            vec![Awaiting { sender, at, storno: None, aenderung: None, is_va: true, before: Box::new(core.clone()) }];
        let history = vec![Event {
            at,
            direction,
            sender,
            message: Message::Anfrage(Anfrage::Vorabstimmung(vorabstimmung.clone())),
        }];
        Ok(Self {
            role,
            vorabstimmung,
            calendar: Calendar::wbci(),
            shared_network_operator: false,
            core,
            awaiting,
            history,
        })
    }

    /// Uses `calendar` for lead times and deadlines.
    #[must_use]
    pub fn with_calendar(mut self, calendar: Calendar) -> Self {
        self.calendar = calendar;
        self
    }

    /// Marks the special case of Anl. 1 §2.11: EKPauf and EKPabg use the same network
    /// operator, so a postponement needs only 24 working hours of lead.
    #[must_use]
    pub const fn with_shared_network_operator(mut self) -> Self {
        self.shared_network_operator = true;
        self
    }

    /// The owner's side.
    #[must_use]
    pub const fn role(&self) -> Role {
        self.role
    }

    /// The pre-agreement request.
    #[must_use]
    pub const fn vorabstimmung(&self) -> &Vorabstimmung {
        &self.vorabstimmung
    }

    /// The state.
    #[must_use]
    pub const fn state(&self) -> &State {
        &self.core.state
    }

    /// The takeover of the line, if announced.
    #[must_use]
    pub const fn takeover(&self) -> Option<&Takeover> {
        self.core.takeover.as_ref()
    }

    /// Open cancellations and postponements.
    #[must_use]
    pub fn open(&self) -> &[Open] {
        &self.core.open
    }

    /// Every accepted event, in order.
    #[must_use]
    pub fn history(&self) -> &[Event] {
        &self.history
    }

    fn ekp(&self) -> &Ekp {
        &self.vorabstimmung.ekp
    }

    fn fall(&self) -> Geschaeftsfall {
        self.vorabstimmung.geschaeftsfall()
    }

    /// Applies `message`, sent or received at `at`.
    ///
    /// # Errors
    ///
    /// [`Refusal`] with the code to answer with; the process is unchanged.
    pub fn apply(&mut self, direction: Direction, at: Timestamp, message: impl Into<Message>) -> Result<(), Refusal> {
        let message = message.into();
        let sender = match direction {
            Direction::Sent => self.role,
            Direction::Received => self.role.other(),
        };
        let before = self.core.clone();
        let result = match &message {
            Message::Anfrage(a) => self.anfrage(a, sender, at),
            Message::Meldung(m) => self.meldung(m, sender, at),
            Message::Teq(t) => self.teq(t, sender, at),
        };
        match result {
            Ok(()) => {
                if !matches!(message, Message::Teq(_)) {
                    let (storno, aenderung) = match &message {
                        Message::Anfrage(Anfrage::Storno(s)) => (Some(s.storno_id.clone()), None),
                        Message::Anfrage(Anfrage::Terminverschiebung(t)) => (None, Some(t.aenderungs_id.clone())),
                        Message::Meldung(m) => (m.storno_id().cloned(), m.aenderungs_id().cloned()),
                        _ => (None, None),
                    };
                    let is_va = matches!(message, Message::Anfrage(Anfrage::Vorabstimmung(_)));
                    self.awaiting.push(Awaiting { sender, at, storno, aenderung, is_va, before: Box::new(before) });
                }
                self.history.push(Event { at, direction, sender, message });
                Ok(())
            }
            Err(e) => {
                self.core = before;
                Err(e)
            }
        }
    }

    fn check_parties(&self, ekp: &Ekp, va_id: &crate::VorabstimmungsId) -> Result<(), Refusal> {
        if ekp != self.ekp() {
            return refuse(Meldungscode::Sonstiges, "the Endkundenvertragspartner differ from the pre-agreement");
        }
        if *va_id != self.vorabstimmung.id {
            return refuse(Meldungscode::VorabstimmungsIdUnbekannt, "refers to another Vorabstimmungs-ID");
        }
        Ok(())
    }

    /// A reused id is answered with 8115 before any other check (WBCI 2.0 §12.3.3).
    fn unused(&self, id: &str) -> Result<(), Refusal> {
        if self.core.ids.contains(id) {
            return refuse(Meldungscode::IdBereitsVerwendet, "the id is already used in this pre-agreement");
        }
        Ok(())
    }

    fn new_id(&mut self, id: &str) -> Result<(), Refusal> {
        self.unused(id)?;
        self.core.ids.insert(id.to_owned());
        Ok(())
    }

    fn agreed_date(&self) -> Option<Date> {
        match self.core.state {
            State::Agreed { wechseltermin } => Some(wechseltermin),
            _ => None,
        }
    }

    fn anfrage(&mut self, a: &Anfrage, sender: Role, at: Timestamp) -> Result<(), Refusal> {
        if a.sender() != sender {
            return refuse(Meldungscode::Sonstiges, format!("a {} is sent by the {}", a.geschaeftsfall(), a.sender()));
        }
        match a {
            Anfrage::Vorabstimmung(va) => {
                if va.ekp != *self.ekp() {
                    return refuse(
                        Meldungscode::Sonstiges,
                        "the Endkundenvertragspartner differ from the pre-agreement",
                    );
                }
                if va.id == self.vorabstimmung.id {
                    // A resend after a negative receipt is allowed with the same id (WBCI 2.0
                    // §12.3.1); anything else is a duplicate (§12.3.3).
                    let undelivered = self.core.state == State::Requested
                        && self.core.va_teq_at.is_none()
                        && !self.awaiting.iter().any(|w| w.is_va);
                    if undelivered && *va == self.vorabstimmung {
                        return Ok(());
                    }
                    return refuse(Meldungscode::IdBereitsVerwendet, "the pre-agreement was already received");
                }
                refuse(
                    Meldungscode::Vae,
                    "a pre-agreement for this switch already exists; a new one starts a new process",
                )
            }
            Anfrage::Storno(s) => {
                self.unused(s.storno_id.as_str())?;
                let fall = s.art.is_aufhebung();
                let code = Meldungscode::StornoAbgelehnt;
                self.check_parties(&s.ekp, s.bezug.id()).map_err(|e| {
                    if e.code == Meldungscode::Sonstiges {
                        e
                    } else {
                        Refusal { code: Meldungscode::VorabstimmungsIdUnbekannt, ..e }
                    }
                })?;
                if self.core.state.is_terminal() {
                    return refuse(code, "the pre-agreement has ended");
                }
                if self.core.open.iter().any(|o| matches!(o, Open::Storno { .. })) {
                    return refuse(code, "a cancellation is already open");
                }
                let agreed = self.agreed_date();
                match (&s.art, agreed) {
                    (StornoArt::AufhebungAuf, None) if self.core.va_teq_at.is_none() => {
                        return refuse(
                            code,
                            "the EKPauf cancels after the receipt of the pre-agreement (Anl. 1 §2.9.1)",
                        );
                    }
                    (StornoArt::AufhebungAuf, _) => {}
                    (_, None) => {
                        return refuse(code, "only after the RUEM-VA (Anl. 1 §2.9.2, §2.10)");
                    }
                    _ => {}
                }
                if let Some(x) = agreed {
                    let latest = if fall {
                        deadline::storno_aufhebung_latest(x, sender, &self.calendar)
                    } else {
                        deadline::storno_aenderung_latest(x, sender, &self.calendar)
                    };
                    if latest.is_overdue(at) {
                        return refuse(code, format!("too late: {latest}"));
                    }
                }
                self.new_id(s.storno_id.as_str())?;
                self.core.open.push(Open::Storno { id: s.storno_id.clone(), aufhebung: fall, sender, teq_at: None });
                Ok(())
            }
            Anfrage::Terminverschiebung(t) => {
                self.unused(t.aenderungs_id.as_str())?;
                let code = Meldungscode::TerminverschiebungAbgelehnt;
                self.check_parties(&t.ekp, t.bezug.id())?;
                let Some(x) = self.agreed_date() else {
                    return refuse(code, "a postponement needs an agreed switch date (Anl. 1 §2.11)");
                };
                if self.core.open.iter().any(|o| matches!(o, Open::Terminverschiebung { .. })) {
                    return refuse(code, "a postponement is already open (Anl. 1 §2.11)");
                }
                if t.neuer_kundenwunschtermin <= x {
                    return refuse(code, "a postponement moves the date later only (Anl. 1 §2.11)");
                }
                let latest = deadline::terminverschiebung_latest(x, self.shared_network_operator, &self.calendar);
                if latest.is_overdue(at) {
                    return refuse(code, format!("too late: {latest}"));
                }
                self.new_id(t.aenderungs_id.as_str())?;
                self.core.open.push(Open::Terminverschiebung {
                    id: t.aenderungs_id.clone(),
                    termin: t.neuer_kundenwunschtermin,
                    teq_at: None,
                });
                Ok(())
            }
        }
    }

    fn take_open(&mut self, storno: Option<&StornoId>, aenderung: Option<&AenderungsId>) -> Option<Open> {
        let pos = self.core.open.iter().position(|o| match o {
            Open::Storno { id, .. } => Some(id) == storno,
            Open::Terminverschiebung { id, .. } => Some(id) == aenderung,
        })?;
        Some(self.core.open.remove(pos))
    }

    fn meldung(&mut self, m: &Meldung, sender: Role, at: Timestamp) -> Result<(), Refusal> {
        let k = m.kopf();
        self.check_parties(&k.ekp, &k.vorabstimmungs_id)?;
        if *self.ekp().of(sender) != k.absender {
            return refuse(Meldungscode::Sonstiges, "the Absender is not the sending party");
        }
        let other = Meldungscode::Sonstiges;
        let fall = k.geschaeftsfall;
        if fall.is_vorabstimmung() && fall != self.fall() {
            return refuse(other, format!("answers {fall} but the pre-agreement is {}", self.fall()));
        }
        match m {
            Meldung::RuemVa(r) => {
                if sender != Role::EkpAbg || self.core.state != State::Requested {
                    return refuse(other, "a RUEM-VA answers an unanswered pre-agreement and comes from the EKPabg");
                }
                self.delivered(at);
                self.core.state = State::Agreed { wechseltermin: r.wechseltermin };
            }
            Meldung::Abbm(a) => match fall {
                f if f.is_vorabstimmung() => {
                    if sender != Role::EkpAbg || self.core.state != State::Requested {
                        return refuse(
                            other,
                            "an ABBM to the pre-agreement answers an unanswered pre-agreement and comes from the EKPabg",
                        );
                    }
                    self.delivered(at);
                    let code = a.positionen.first().map_or(Meldungscode::Sonstiges, |p| p.position.code.clone());
                    self.core.state = State::Rejected { code };
                }
                _ => {
                    let open = self.take_open(a.storno_id.as_ref(), a.aenderungs_id.as_ref());
                    if !answers(open.as_ref(), sender, fall) {
                        return refuse(
                            other,
                            "no open cancellation or postponement with this id for the sender to refuse",
                        );
                    }
                }
            },
            Meldung::Erlm(e) => {
                let open = self.take_open(e.storno_id.as_ref(), e.aenderungs_id.as_ref());
                if !answers(open.as_ref(), sender, fall) {
                    return refuse(
                        other,
                        "no open cancellation or postponement with this id for the sender to confirm",
                    );
                }
                match open {
                    Some(Open::Storno { aufhebung: true, .. }) => self.core.state = State::Cancelled,
                    Some(Open::Storno { aufhebung: false, .. }) => {
                        self.core.state = State::Changed { wechseltermin: self.agreed_date() }
                    }
                    Some(Open::Terminverschiebung { termin, .. }) => {
                        self.core.state = State::Agreed { wechseltermin: e.wechseltermin.unwrap_or(termin) };
                    }
                    None => {}
                }
            }
            Meldung::AkmTr(t) => {
                if sender != Role::EkpAuf || !self.fall().is_kuendigung() {
                    return refuse(other, "an AKM-TR comes from the EKPauf in VA-KUE-MRN or VA-KUE-ORN");
                }
                if self.agreed_date().is_none() {
                    return refuse(
                        Meldungscode::VorabstimmungsIdUnbekannt,
                        "an AKM-TR follows an agreed pre-agreement",
                    );
                }
                if matches!(self.core.takeover, Some(Takeover::Announced { .. })) {
                    return refuse(Meldungscode::UebernahmeBereitsBestaetigt, "the takeover was already announced");
                }
                self.core.takeover =
                    Some(Takeover::Announced { uebernahme: t.ressourcenuebernahme, sicherer_hafen: t.sicherer_hafen });
            }
            Meldung::AbbmTr(t) => {
                if sender != Role::EkpAbg || !matches!(self.core.takeover, Some(Takeover::Announced { .. })) {
                    return refuse(other, "an ABBM-TR answers an AKM-TR and comes from the EKPabg");
                }
                let code = t.positionen.first().map_or(Meldungscode::Sonstiges, |p| p.code.clone());
                self.core.takeover = Some(Takeover::Refused { code });
            }
        }
        Ok(())
    }

    /// A business answer proves delivery of the pre-agreement (WBCI 2.0 §12.3.3).
    fn delivered(&mut self, at: Timestamp) {
        if self.core.va_teq_at.is_none() {
            self.core.va_teq_at = Some(at);
        }
        self.awaiting.retain(|w| !w.is_va);
    }

    fn teq(&mut self, t: &Teq, sender: Role, at: Timestamp) -> Result<(), Refusal> {
        if t.absender != *self.ekp().of(sender) {
            return refuse(Meldungscode::Sonstiges, "the Absender is not the sending party");
        }
        if t.vorabstimmungs_id != self.vorabstimmung.id {
            return refuse(Meldungscode::VorabstimmungsIdUnbekannt, "refers to another Vorabstimmungs-ID");
        }
        let Some(pos) = self
            .awaiting
            .iter()
            .position(|w| w.sender != sender && w.storno == t.storno_id && w.aenderung == t.aenderungs_id)
        else {
            return refuse(Meldungscode::Sonstiges, "nothing awaits this receipt");
        };
        let item = self.awaiting.remove(pos);
        if t.is_ok() {
            if item.is_va {
                self.core.va_teq_at = Some(at);
            }
            for o in &mut self.core.open {
                match o {
                    Open::Storno { id, teq_at, .. } if Some(&*id) == item.storno.as_ref() => *teq_at = Some(at),
                    Open::Terminverschiebung { id, teq_at, .. } if Some(&*id) == item.aenderung.as_ref() => {
                        *teq_at = Some(at);
                    }
                    _ => {}
                }
            }
        } else {
            // Not delivered: undo it and everything after it; a resend with the same id is
            // allowed (WBCI 2.0 §12.3.1).
            self.core = *item.before;
            self.awaiting.truncate(pos);
        }
        Ok(())
    }

    /// What `role` may do next.
    #[must_use]
    pub fn next_actions(&self, role: Role) -> Vec<Action> {
        let mut out = Vec::new();
        if self.awaiting.iter().any(|w| w.sender != role) {
            out.push(Action::Acknowledge);
        }
        let state = &self.core.state;
        let storno_open = self.core.open.iter().any(|o| matches!(o, Open::Storno { .. }));
        for o in &self.core.open {
            match o {
                Open::Storno { sender, .. } if *sender != role => out.push(Action::AnswerStorno),
                Open::Terminverschiebung { .. } if role == Role::EkpAbg => out.push(Action::AnswerTerminverschiebung),
                _ => {}
            }
        }
        match (state, role) {
            (State::Requested, Role::EkpAbg) => out.push(Action::AnswerVorabstimmung),
            (State::Requested, Role::EkpAuf) if self.core.va_teq_at.is_some() && !storno_open => {
                out.push(Action::SendStornoAufhebung);
            }
            (State::Agreed { .. }, _) => {
                if role == Role::EkpAuf && self.fall().is_kuendigung() && self.core.takeover.is_none() {
                    out.push(Action::SendAkmTr);
                }
                if role == Role::EkpAbg && matches!(self.core.takeover, Some(Takeover::Announced { .. })) {
                    out.push(Action::RefuseAkmTr);
                }
                if !storno_open {
                    out.push(Action::SendStornoAufhebung);
                    out.push(Action::SendStornoAenderung);
                }
                if role == Role::EkpAuf && !self.core.open.iter().any(|o| matches!(o, Open::Terminverschiebung { .. }))
                {
                    out.push(Action::SendTerminverschiebung);
                }
            }
            (State::Changed { .. }, Role::EkpAuf) => out.push(Action::SubmitNewVorabstimmung),
            _ => {}
        }
        out
    }

    /// The deadlines running now: receipts, answers, the AKM-TR and the last moments for
    /// cancellation and postponement.
    #[must_use]
    pub fn deadlines(&self) -> Vec<Deadline> {
        let cal = &self.calendar;
        let mut out: Vec<Deadline> = self.awaiting.iter().map(|w| deadline::teq_escalation(w.at, w.sender)).collect();
        match &self.core.state {
            State::Requested => {
                if let Some(teq) = self.core.va_teq_at {
                    out.push(deadline::va_answer(teq, cal));
                }
            }
            State::Agreed { wechseltermin } => {
                let x = *wechseltermin;
                if self.fall().is_kuendigung() && self.core.takeover.is_none() {
                    out.push(deadline::akm_tr_reminder(x, cal));
                    out.push(deadline::akm_tr_escalation(x, cal));
                }
                out.push(deadline::storno_aufhebung_latest(x, self.role, cal));
                out.push(deadline::storno_aenderung_latest(x, self.role, cal));
                if self.role == Role::EkpAuf {
                    out.push(deadline::terminverschiebung_latest(x, self.shared_network_operator, cal));
                }
            }
            State::Changed { wechseltermin: Some(x) } => out.push(deadline::new_vorabstimmung(*x, cal)),
            _ => {}
        }
        for o in &self.core.open {
            match o {
                Open::Storno { sender, teq_at: Some(t), .. } => out.push(deadline::answer_24h(*t, sender.other(), cal)),
                Open::Terminverschiebung { teq_at: Some(t), .. } => {
                    out.push(deadline::answer_24h(*t, Role::EkpAbg, cal));
                }
                _ => {}
            }
        }
        out.sort_by_key(|d| d.due);
        out
    }

    /// The obligations overdue at `now` (receipts, answers, AKM-TR, new pre-agreement); the
    /// last moments for cancellation and postponement are windows, not obligations.
    #[must_use]
    pub fn overdue(&self, now: Timestamp) -> Vec<Deadline> {
        self.deadlines()
            .into_iter()
            .filter(|d| {
                !matches!(
                    d.kind,
                    DeadlineKind::StornoAufhebungLatest
                        | DeadlineKind::StornoAenderungLatest
                        | DeadlineKind::TerminverschiebungLatest
                ) && d.is_overdue(now)
            })
            .collect()
    }
}

/// Whether a message from `sender` in case `fall` answers `open`.
fn answers(open: Option<&Open>, sender: Role, fall: Geschaeftsfall) -> bool {
    match open {
        Some(Open::Storno { sender: s, aufhebung, .. }) => {
            *s != sender && fall == if *aufhebung { Geschaeftsfall::StrAuf } else { Geschaeftsfall::StrAen }
        }
        Some(Open::Terminverschiebung { .. }) => sender == Role::EkpAbg && fall == Geschaeftsfall::TvsVa,
        None => false,
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Requested => f.write_str("requested"),
            Self::Agreed { wechseltermin } => write!(f, "agreed for {wechseltermin}"),
            Self::Rejected { code } => write!(f, "rejected ({code})"),
            Self::Cancelled => f.write_str("cancelled"),
            Self::Changed { .. } => f.write_str("changed — submit a new pre-agreement"),
        }
    }
}
