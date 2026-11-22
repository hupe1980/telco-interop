//! Simulated S/PRI peers, to test a service provider's side without a partner: the network
//! owner ([`Leistungserbringer`]) and, in a provider switch, the losing provider
//! ([`AbgebenderProvider`]).
//!
//! A peer is a value: give it what arrives with the moment it arrives ([`Leistungserbringer::receive`],
//! or `receive_xml` for the bytes), and it returns its technical receipt — plus anything that
//! fell due before. Its business messages fall due after a [`Latency`] or on a date (the ERLM on
//! the binding date): `next_due` says when, `advance(now)` returns them. Every [`Outbound`]
//! names its time and its receiver. The peers read no clock and have no randomness.
//!
//! Each peer is driven by a [`Process`] in its role: it sends only what that process accepts, so
//! it walks the path the EPKs of Anlage 7 V.13 draw for the business case; it answers what its
//! process refuses with what the protocol prescribes. One value plays one order; keep one per
//! order number.
//!
//! ```
//! use spri::process::{Role, State};
//! use spri::sim::Leistungserbringer;
//! use spri::*;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let xml = include_str!("../tests/fixtures/auftrag-lae.xml");
//! let order = Payload::from_xml(xml.as_bytes())?; // what your system sends
//! let signatur = SignaturId { issuer: SignaturText::new("CN=Sandbox")?, serial: SignaturText::new("1")? };
//! let mut network_owner = Leistungserbringer::new(signatur);
//!
//! let sent: Timestamp = "2026-11-23T08:00:00Z".parse()?;
//! let reply = network_owner.receive(&order, sent);
//! assert!(matches!(reply.outbound[0].payload, Payload::AuftragResponse { .. })); // TEQ OK
//!
//! // Let time pass to after the binding date: QEB, ABM, ERLM, ENTM.
//! let later: Timestamp = "2027-02-01T00:00:00Z".parse()?;
//! let kinds: Vec<_> = network_owner.advance(later).iter().map(|o| o.payload.kind()).collect();
//! assert_eq!(kinds, ["QEB", "ABM", "ERLM", "ENTM"]);
//! assert_eq!(*network_owner.process().ok_or("no order")?.state(), State::Closed);
//! # Ok(())
//! # }
//! ```
//!
//! ## The network owner's path
//!
//! | State | Next message | Source |
//! |---|---|---|
//! | order received | TEQ `OK`; ABBM with the codes its validation predicts, else QEB `0000` (GET/SET: ERLM `0010`) | Anl. 7 EPK V.13 p.10, p.1–9; S/PRI 4.4 §8.2.1 |
//! | QEB sent | ABM `0000` for the requested date (EST: ERLM); PV: first AKM-PV `0020` to the losing provider | Anl. 7 EPK V.13 p.1, p.3, p.5–7 |
//! | PV, RUEM-PV consents | ABM and ABM-PV `0030` | Anl. 7 EPK V.13 p.7; S/PRI 4.4 §8.2.13 |
//! | PV, RUEM-PV refuses / none by the antwortfrist | ABBM `0041` / `0042` and ABBM-PV `0040` | S/PRI 4.4 §5.5.1.1, §5.5.1.2 |
//! | ABM sent | with [`Leistungserbringer::with_terminanforderung`] a TAM `6001`; else ERLM `0010` on the binding date (PV: and ERLM-PV) | AH 2.9.5 §3.6; Anl. 7 EPK V.13 p.1 |
//! | TAM unanswered | MTAM, then ABBM `1198` | AH 2.9.5 §3.6 |
//! | ERLM-K received | ERLM `0010` | AH 2.9.5 §3.6 |
//! | Storno received | ERLM `0012` (or ABBM `1087` with [`Leistungserbringer::with_storno_abgelehnt`]); PV: ABBM-PV `0040` | Anl. 7 EPK V.13 p.11 |
//! | Terminverschiebung received | a new ABM for the new date (or ABBM `1085`); PV: and ABM-PV | Anl. 7 EPK V.13 p.12 |
//! | ERLM sent | ENTM `0010` (PV: and ENTM-PV) | S/PRI 4.4 §8.2.3, §8.2.4 |
//!
//! Interpretations (the documents set the deadlines, not the moment inside them): a message goes
//! one [`Latency::answer`] after the last event; the ERLM at 10:00 German time on the binding
//! date (or later, after the last event); MTAM and ABBM 1198 one answer latency before their
//! 10-Werktage deadline ends; ABBM 0042 one answer latency after the antwortfrist; the
//! companion messages of the PV leg one second after their message (they are sent together,
//! S/PRI 4.4 §8.2.13); the antwortfrist of the AKM-PV is 3 Werktage after its day (S/PRI 4.4
//! §5.5.1.1 leaves it to the contract); the receipt [`Latency::receipt`] after the call, at most
//! 30 seconds (S/PRI 4.4 §11.2.3). A readable call always gets a positive receipt; the receipt
//! confirms the technical receipt (S/PRI 4.4 §11.2.3.1), a refusal is a message.
//!
//! Replies, refusals, errors and `Debug` carry codes, ids, dates and states, never a name,
//! address or number.

use core::fmt;

use carrier_core::calendar::berlin;
use carrier_core::count::day_of;
use jiff::Timestamp;
use jiff::civil::Date;

use crate::deadline::{self, DeadlineKind};
use crate::process::{Direction, Message, Pending, Process, PvLeg, Refusal, Role, State};
use crate::validate::Severity;
use crate::xml::ReadError;
use crate::{
    A10nsp, Abbm, AbbmPv, Abm, AbmPv, Aenderungskennzeichen, AkmPv, Antwortcode, Auftrag, Auftraggebernummer,
    Auftragstyp, Calendar, Control, Entm, EntmPv, Erlm, ErlmPv, FttxAnschluss, Geschaeftsfall, GeschaeftsfallCode,
    JaNein, LineId, Meldung, Meldungscode, Meldungsinhalt, Meldungstyp, Payload, Port, Position, PositionWita,
    Providername, ProviderwechselAnkuendigung, ProviderwechselBestaetigung, ProviderwechselRueckmeldung, Qeb, RuemPv,
    SignaturId, Teq, Terminanforderung, UebergabepunktKunde, Vertragsnummer, Wunschtermin,
};

#[doc(inline)]
pub use carrier_core::sim::Latency;
/// The duration type of [`Latency`].
pub use jiff::SignedDuration;

/// The most acts one call applies; a bound, never reached by a working peer.
const MAX_ACTS: usize = 1_000;

/// The receipt's deadline (S/PRI 4.4 §11.2.3).
const RECEIPT_MAX: SignedDuration = SignedDuration::from_secs(30);

/// Companion messages of the PV leg follow their message after one second.
const TOGETHER: SignedDuration = SignedDuration::from_secs(1);

/// The antwortfrist of the AKM-PV, in Werktage after its day (an interpretation).
const ANTWORTFRIST_WERKTAGE: u32 = 3;

/// A payload a peer sends, when, and to whom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outbound {
    /// The moment it is sent (also its control timestamp).
    pub at: Timestamp,
    /// The receiver.
    pub to: Role,
    /// The payload.
    pub payload: Payload,
}

/// Why a peer did not accept an inbound call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// The bytes do not read; [`ReadError::teq_code`] is the receipt code to answer with.
    Unreadable(ReadError),
    /// The process refuses it.
    Refused(Refusal),
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable(e) => write!(f, "unreadable: {e} (TEQ {})", e.teq_code().as_str()),
            Self::Refused(r) => match &r.answer {
                Some(code) => write!(f, "refused: {r} (ABBM {})", code.as_str()),
                None => write!(f, "refused: {r}"),
            },
        }
    }
}

/// What a peer returns for one inbound call.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Reply {
    /// What it sends: acts that fell due before the call, its receipt and, for a refused
    /// Storno or Terminverschiebung, the ABBM.
    pub outbound: Vec<Outbound>,
    /// Why the call was not accepted, if it was not.
    pub rejection: Option<Rejection>,
}

/// The network owner's own data, synthetic by default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Netzdaten {
    /// The contract number of a new line (orders that name one keep theirs).
    pub vertragsnummer: Vertragsnummer,
    /// The handover point (`a10nsp`) of the ABM.
    pub a10nsp: A10nsp,
    /// The port of the ABM.
    pub port: Port,
    /// The LineID of a new line (orders that name one keep theirs).
    pub line_id: LineId,
    /// In a PV: the losing provider's Auftraggebernummer with the network owner.
    pub abgebender_auftraggebernummer: Auftraggebernummer,
    /// In a PV: the losing provider's contract number.
    pub abgebender_vertragsnummer: Vertragsnummer,
    /// In a PV: the losing provider's name, for ABBM 0042.
    pub abgebender_providername: Providername,
    /// In a PV: the gaining provider's name, for AKM-PV and ABM-PV.
    pub aufnehmender_providername: Providername,
}

impl Netzdaten {
    /// Synthetic values for every field (an error only if a value did not meet its type, which a
    /// unit test rules out).
    ///
    /// # Errors
    ///
    /// A value does not meet its type's facets.
    pub fn synthetic() -> Result<Self, crate::ValueError> {
        Ok(Self {
            vertragsnummer: Vertragsnummer::new("V900001")?,
            a10nsp: A10nsp::new("A10-SANDBOX")?,
            port: Port::new("P1")?,
            line_id: "DEU.NB01.4200000001".parse()?,
            abgebender_auftraggebernummer: Auftraggebernummer::new("0815")?,
            abgebender_vertragsnummer: Vertragsnummer::new("V900999")?,
            abgebender_providername: Providername::new("Sandbox Abgebend GmbH")?,
            aufnehmender_providername: Providername::new("Sandbox Aufnehmend GmbH")?,
        })
    }
}

/// What a peer is about to send.
struct Act {
    at: Timestamp,
    to: Role,
    meldung: Meldung,
}

fn control(signatur: &SignaturId, at: Timestamp) -> Control {
    Control::new(at, signatur.clone())
}

fn later(at: Timestamp, by: SignedDuration) -> Timestamp {
    at.saturating_add(by).unwrap_or(at)
}

/// 10:00 German time on `day`, or the start of the day if it does not exist.
fn ten_oclock(day: Date) -> Timestamp {
    day.at(10, 0, 0, 0)
        .to_zoned_datetime(berlin())
        .map_or_else(|_| carrier_core::count::start_of(day), |z| z.timestamp())
}

fn wunschtermin(g: &Geschaeftsfall) -> Option<Wunschtermin> {
    match g {
        Geschaeftsfall::Neu(x) => Some(x.wunschtermin),
        Geschaeftsfall::KueAg(x) => Some(x.wunschtermin),
        Geschaeftsfall::Lae(x) => Some(x.wunschtermin),
        Geschaeftsfall::AenLmae(x) => Some(x.wunschtermin),
        Geschaeftsfall::Est(x) => Some(x.wunschtermin),
        Geschaeftsfall::Pv(x) => Some(x.wunschtermin),
        Geschaeftsfall::Get(_) | Geschaeftsfall::Set(_) => None,
    }
}

fn vertragsnummer(g: &Geschaeftsfall) -> Option<&Vertragsnummer> {
    match g {
        Geschaeftsfall::KueAg(x) => Some(&x.vertragsnummer),
        Geschaeftsfall::Lae(x) => Some(&x.vertragsnummer),
        Geschaeftsfall::AenLmae(x) => Some(&x.vertragsnummer),
        Geschaeftsfall::Est(x) => Some(&x.vertragsnummer),
        Geschaeftsfall::Get(x) | Geschaeftsfall::Set(x) => Some(&x.vertragsnummer),
        Geschaeftsfall::Neu(_) | Geschaeftsfall::Pv(_) => None,
    }
}

fn wita(code: Meldungscode) -> PositionWita {
    Position::new(code).into()
}

fn process_debug(f: &mut fmt::DebugStruct<'_, '_>, p: Option<&Process>) {
    f.field("geschaeftsfall", &p.map(Process::geschaeftsfall))
        .field("state", &p.map(|p| p.state().to_string()))
        .field("pv", &p.and_then(Process::pv))
        .field("events", &p.map_or(0, |p| p.history().len()));
}

// -- the network owner -------------------------------------------------------------------------

/// The simulated network owner (Leistungserbringer) of one order.
///
/// `Debug` shows the configuration, the state and the number of events, never an order's values.
#[derive(Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // independent switches of the simulated path
pub struct Leistungserbringer {
    signatur: SignaturId,
    latency: Latency,
    calendar: Calendar,
    netz: Option<Netzdaten>,
    terminanforderung: bool,
    storno_abgelehnt: bool,
    terminverschiebung_abgelehnt: bool,
    process: Option<Process>,
    refuse_with: Vec<Meldungscode>,
    stopped: Option<Refusal>,
}

impl fmt::Debug for Leistungserbringer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("Leistungserbringer");
        d.field("latency", &self.latency)
            .field("terminanforderung", &self.terminanforderung)
            .field("storno_abgelehnt", &self.storno_abgelehnt)
            .field("terminverschiebung_abgelehnt", &self.terminverschiebung_abgelehnt);
        process_debug(&mut d, self.process.as_ref());
        d.field("stopped", &self.stopped).finish_non_exhaustive()
    }
}

// The message builders are methods for symmetry, whether or not they read `self`.
#[allow(clippy::unused_self)]
impl Leistungserbringer {
    /// A network owner signing with `signatur`, with the default [`Latency`], [`Netzdaten::synthetic`] and
    /// [`Calendar::werktage`], confirming the requested date, a Storno and a Terminverschiebung.
    #[must_use]
    pub fn new(signatur: SignaturId) -> Self {
        Self {
            signatur,
            latency: Latency::default(),
            calendar: Calendar::werktage(),
            netz: Netzdaten::synthetic().ok(),
            terminanforderung: false,
            storno_abgelehnt: false,
            terminverschiebung_abgelehnt: false,
            process: None,
            refuse_with: Vec::new(),
            stopped: None,
        }
    }

    /// Uses `latency` for receipts and messages.
    #[must_use]
    pub const fn with_latency(mut self, latency: Latency) -> Self {
        self.latency = latency;
        self
    }

    /// Uses `netz` for the network owner's own data.
    #[must_use]
    pub fn with_netzdaten(mut self, netz: Netzdaten) -> Self {
        self.netz = Some(netz);
        self
    }

    /// Counts Werktage with `calendar`.
    #[must_use]
    pub fn with_calendar(mut self, calendar: Calendar) -> Self {
        self.calendar = calendar;
        self
    }

    /// After the ABM, asks for a new appointment (TAM `6001`); unanswered, it sends the MTAM
    /// and aborts with ABBM `1198` (AH 2.9.5 §3.6). NEU, LAE and PV only.
    #[must_use]
    pub const fn with_terminanforderung(mut self) -> Self {
        self.terminanforderung = true;
        self
    }

    /// Refuses a Storno with ABBM `1087`; the order continues (Anl. 7 EPK V.13 p.11).
    #[must_use]
    pub const fn with_storno_abgelehnt(mut self) -> Self {
        self.storno_abgelehnt = true;
        self
    }

    /// Refuses a Terminverschiebung with ABBM `1085`; the order continues (Anl. 7 EPK V.13 p.12).
    #[must_use]
    pub const fn with_terminverschiebung_abgelehnt(mut self) -> Self {
        self.terminverschiebung_abgelehnt = true;
        self
    }

    /// The order's process, once an order was received.
    #[must_use]
    pub const fn process(&self) -> Option<&Process> {
        self.process.as_ref()
    }

    /// Why the peer stopped, if its process refused one of its own acts (never in a working
    /// peer).
    #[must_use]
    pub const fn stopped(&self) -> Option<&Refusal> {
        self.stopped.as_ref()
    }

    /// When the next act is due, if any.
    #[must_use]
    pub fn next_due(&self) -> Option<Timestamp> {
        self.plan().map(|a| a.at)
    }

    /// Sends every act due by `now`, in time order.
    pub fn advance(&mut self, now: Timestamp) -> Vec<Outbound> {
        let mut out = Vec::new();
        for _ in 0..MAX_ACTS {
            let Some(act) = self.plan().filter(|a| a.at <= now) else { break };
            let Some(process) = self.process.as_mut() else { break };
            match process.apply(Direction::Sent, act.at, act.meldung.clone()) {
                Ok(()) => out.push(Outbound {
                    at: act.at,
                    to: act.to,
                    payload: Payload::MeldungRequest { control: control(&self.signatur, act.at), meldung: act.meldung },
                }),
                Err(e) => {
                    self.stopped = Some(e);
                    break;
                }
            }
        }
        out
    }

    /// Reads `xml` and receives it at `at`; bytes that do not read are rejected and answered
    /// with nothing.
    pub fn receive_xml(&mut self, xml: &[u8], at: Timestamp) -> Reply {
        match Payload::from_xml(xml) {
            Ok(payload) => self.receive(&payload, at),
            Err(e) => Reply { outbound: self.advance(at), rejection: Some(Rejection::Unreadable(e)) },
        }
    }

    /// Receives `payload` at `at`: first sends what fell due before, then answers it.
    pub fn receive(&mut self, payload: &Payload, at: Timestamp) -> Reply {
        let mut reply = Reply { outbound: self.advance(at), rejection: None };
        let receipt_at = later(at, self.latency.receipt_within(RECEIPT_MAX));
        reply.rejection = match payload {
            Payload::AuftragRequest { auftrag, .. } => self.auftrag(auftrag, at, receipt_at, &mut reply.outbound),
            Payload::MeldungRequest { meldung, .. } => self.meldung(meldung, at, receipt_at, &mut reply.outbound),
            Payload::AuftragResponse { quittung, .. } | Payload::MeldungResponse { quittung, .. } => {
                match self.process.as_mut() {
                    None => Some(Rejection::Refused(Refusal {
                        reason: "a TEQ, but no order was received (Anl. 7 EPK V.13 p.10)".to_owned(),
                        answer: None,
                    })),
                    Some(p) => p.apply(Direction::Received, at, quittung.clone()).err().map(Rejection::Refused),
                }
            }
        };
        reply
    }

    fn auftrag(
        &mut self,
        a: &Auftrag,
        at: Timestamp,
        receipt_at: Timestamp,
        out: &mut Vec<Outbound>,
    ) -> Option<Rejection> {
        let receipt = Teq::new(
            Meldungscode::Ok,
            Some(a.externe_auftragsnummer.clone()),
            Some(a.auftraggeber.auftraggebernummer.clone()),
        );
        out.push(Outbound {
            at: receipt_at,
            to: Role::Auftraggeber,
            payload: Payload::AuftragResponse {
                control: control(&self.signatur, receipt_at),
                quittung: receipt.clone(),
            },
        });
        let result = match self.process.as_mut() {
            None => Process::new(Role::Leistungserbringer, a, Direction::Received, at).map(|p| {
                let fall = a.geschaeftsfall.code();
                self.refuse_with = a
                    .issues_at(at, &self.calendar)
                    .into_iter()
                    .filter(|i| i.severity == Severity::Error)
                    .filter_map(|i| i.code)
                    .filter(|c| c.applies(Meldungstyp::Abbm, fall))
                    .collect();
                self.process = Some(p.with_calendar(self.calendar.clone()));
            }),
            Some(p) => p.apply(Direction::Received, at, a.clone()),
        };
        match result {
            Ok(()) => {
                self.own_receipt(receipt_at, receipt);
                None
            }
            Err(refusal) => {
                if let Some(code) = &refusal.answer {
                    let typ = Auftragstyp {
                        geschaeftsfall: a.geschaeftsfall.code(),
                        aenderungskennzeichen: a.aenderungskennzeichen,
                        geschaeftsfallart: a.geschaeftsfall_art,
                    };
                    let at = later(at, self.latency.answer());
                    let meldung = Meldung { auftragstyp: typ, inhalt: self.abbm(a, vec![code.clone()], None).into() };
                    out.push(Outbound {
                        at,
                        to: Role::Auftraggeber,
                        payload: Payload::MeldungRequest { control: control(&self.signatur, at), meldung },
                    });
                }
                Some(Rejection::Refused(refusal))
            }
        }
    }

    fn meldung(
        &mut self,
        m: &Meldung,
        at: Timestamp,
        receipt_at: Timestamp,
        out: &mut Vec<Outbound>,
    ) -> Option<Rejection> {
        let sender = if m.typ() == Meldungstyp::RuemPv { Role::AbgebenderProvider } else { Role::Auftraggeber };
        let ag = match &m.inhalt {
            Meldungsinhalt::RuemPv(r) => Some(r.auftraggebernummer.clone()),
            Meldungsinhalt::ErlmK(x) => Some(x.auftraggebernummer.clone()),
            Meldungsinhalt::ZwmAg(x) => Some(x.auftraggebernummer.clone()),
            Meldungsinhalt::TbkAg(x) => Some(x.auftraggebernummer.clone()),
            _ => None,
        };
        let receipt = Teq::new(Meldungscode::Ok, Some(m.externe_auftragsnummer().clone()), ag);
        out.push(Outbound {
            at: receipt_at,
            to: sender,
            payload: Payload::MeldungResponse {
                control: control(&self.signatur, receipt_at),
                quittung: receipt.clone(),
            },
        });
        let result = match self.process.as_mut() {
            None => Err(Refusal { reason: format!("a {}, but no order was received", m.typ()), answer: None }),
            Some(p) => p.apply(Direction::Received, at, m.clone()),
        };
        match result {
            Ok(()) => {
                self.own_receipt(receipt_at, receipt);
                None
            }
            Err(refusal) => Some(Rejection::Refused(refusal)),
        }
    }

    fn own_receipt(&mut self, at: Timestamp, receipt: Teq) {
        if let Some(p) = self.process.as_mut()
            && let Err(e) = p.apply(Direction::Sent, at, receipt)
        {
            self.stopped = Some(e);
        }
    }

    fn order(&self) -> Option<&Auftrag> {
        self.process.as_ref()?.history().iter().find_map(|e| match &e.message {
            Message::Auftrag(a) if a.aenderungskennzeichen == Aenderungskennzeichen::Standard => Some(a),
            _ => None,
        })
    }

    fn ruem_pv(&self) -> Option<&RuemPv> {
        self.process.as_ref()?.history().iter().find_map(|e| match &e.message {
            Message::Meldung(Meldung { inhalt: Meldungsinhalt::RuemPv(r), .. }) => Some(r),
            _ => None,
        })
    }

    fn meldung_for(&self, a: &Auftrag, aekz: Aenderungskennzeichen, inhalt: Meldungsinhalt) -> Meldung {
        Meldung {
            auftragstyp: Auftragstyp {
                geschaeftsfall: a.geschaeftsfall.code(),
                aenderungskennzeichen: aekz,
                geschaeftsfallart: a.geschaeftsfall_art,
            },
            inhalt,
        }
    }

    /// The network data; always present (the synthetic values are checked by a test).
    fn netz(&self) -> Option<&Netzdaten> {
        self.netz.as_ref()
    }

    fn vertragsnummer(&self, a: &Auftrag, netz: &Netzdaten) -> Vertragsnummer {
        vertragsnummer(&a.geschaeftsfall).unwrap_or(&netz.vertragsnummer).clone()
    }

    fn line_id(&self, a: &Auftrag, netz: &Netzdaten) -> LineId {
        a.geschaeftsfall.line_id().unwrap_or(&netz.line_id).clone()
    }

    fn abbm(&self, a: &Auftrag, codes: Vec<Meldungscode>, abgebend: Option<ProviderwechselRueckmeldung>) -> Abbm {
        Abbm {
            vertragsnummer: vertragsnummer(&a.geschaeftsfall).cloned(),
            externe_auftragsnummer: a.externe_auftragsnummer.clone(),
            auftraggebernummer: a.auftraggeber.auftraggebernummer.clone(),
            auftraggebernummer_besteller: a.besteller.as_ref().map(|b| b.auftraggebernummer.clone()),
            vorabstimmungs_id: a.geschaeftsfall.vorabstimmungs_id().cloned(),
            wiedervorlagetermin: None,
            abgebender_provider: abgebend,
            positionen: codes.into_iter().map(wita).collect(),
        }
    }

    fn abm(&self, a: &Auftrag, netz: &Netzdaten, datum: Date) -> Abm {
        let zeitfenster = wunschtermin(&a.geschaeftsfall).and_then(|w| w.zeitfenster);
        Abm {
            vertragsnummer: self.vertragsnummer(a, netz),
            externe_auftragsnummer: a.externe_auftragsnummer.clone(),
            auftraggebernummer: a.auftraggeber.auftraggebernummer.clone(),
            auftraggebernummer_besteller: a.besteller.as_ref().map(|b| b.auftraggebernummer.clone()),
            vorabstimmungs_id: a.geschaeftsfall.vorabstimmungs_id().cloned(),
            verbindlicher_liefertermin: Wunschtermin { datum, zeitfenster },
            abgebender_provider: self.ruem_pv().map(|r| r.abgebender_provider.clone()),
            anschluss: FttxAnschluss {
                a10nsp: netz.a10nsp.clone(),
                port: netz.port.clone(),
                s_vlan: None,
                c_vlan: None,
                line_id: self.line_id(a, netz),
                uebergabepunkt_kunde: UebergabepunktKunde::Rj45,
                wita_vertragsnummer: None,
            },
            positionen: vec![Position::new(Meldungscode::C0000).into()],
        }
    }

    fn erlm(&self, a: &Auftrag, netz: &Netzdaten, day: Date, code: Meldungscode) -> Erlm {
        Erlm {
            vertragsnummer: Some(self.vertragsnummer(a, netz)),
            externe_auftragsnummer: a.externe_auftragsnummer.clone(),
            auftraggebernummer: a.auftraggeber.auftraggebernummer.clone(),
            auftraggebernummer_besteller: a.besteller.as_ref().map(|b| b.auftraggebernummer.clone()),
            vorabstimmungs_id: a.geschaeftsfall.vorabstimmungs_id().cloned(),
            erledigungstermin: day,
            entstoer_ergebnis: None,
            anlage: None,
            diagnoseergebnisse: Vec::new(),
            positionen: vec![wita(code)],
        }
    }

    fn terminanforderung(&self, a: &Auftrag, netz: &Netzdaten) -> Terminanforderung {
        Terminanforderung {
            vertragsnummer: self.vertragsnummer(a, netz),
            externe_auftragsnummer: a.externe_auftragsnummer.clone(),
            auftraggebernummer: a.auftraggeber.auftraggebernummer.clone(),
            auftraggebernummer_besteller: a.besteller.as_ref().map(|b| b.auftraggebernummer.clone()),
            vorabstimmungs_id: a.geschaeftsfall.vorabstimmungs_id().cloned(),
            positionen: vec![wita(Meldungscode::C6001)],
        }
    }

    /// The next act and its time, derived from the process state.
    #[allow(clippy::too_many_lines)] // one table: state → next message
    fn plan(&self) -> Option<Act> {
        use Aenderungskennzeichen as K;
        use GeschaeftsfallCode as G;
        if self.stopped.is_some() {
            return None;
        }
        let p = self.process.as_ref()?;
        let a = self.order()?;
        let netz = self.netz()?;
        let fall = p.geschaeftsfall();
        let last_event = p.history().last()?;
        let last = last_event.at;
        let answer = later(last, self.latency.answer());
        let together = later(last, TOGETHER);
        let to_ag = |at: Timestamp, aekz: K, inhalt: Meldungsinhalt| Act {
            at,
            to: Role::Auftraggeber,
            meldung: self.meldung_for(a, aekz, inhalt),
        };
        let to_losing = |at: Timestamp, inhalt: Meldungsinhalt| Act {
            at,
            to: Role::AbgebenderProvider,
            meldung: self.meldung_for(a, K::Standard, inhalt),
        };
        let leg = p.pv();
        let leg_open = leg.is_some_and(|l| !matches!(l, PvLeg::Closed | PvLeg::Aborted { .. }));
        let state = p.state();

        // The PV leg's companion messages first: they go together with their message.
        if leg_open {
            let ended = matches!(state, State::Aborted { .. } | State::Cancelled);
            if ended {
                // The switch ends without taking place: ABBM-PV 0040 (S/PRI 4.4 §5.5.1.2).
                return Some(to_losing(together, self.abbm_pv(a, netz).into()));
            }
            match leg {
                Some(PvLeg::Agreed | PvLeg::Confirmed { .. })
                    if p.next_actions(Role::Leistungserbringer)
                        .contains(&crate::process::Action::Send(Meldungstyp::AbmPv)) =>
                {
                    let datum = p.liefertermin()?;
                    return Some(to_losing(together, self.abm_pv(a, netz, datum).into()));
                }
                Some(PvLeg::Confirmed { .. }) if matches!(state, State::Completed { .. } | State::Closed) => {
                    let day = match state {
                        State::Completed { erledigungstermin } => *erledigungstermin,
                        _ => day_of(last),
                    };
                    return Some(to_losing(together, self.erlm_pv(a, netz, day).into()));
                }
                Some(PvLeg::Completed) if *state == State::Closed => {
                    return Some(to_losing(together, self.entm_pv(a, netz, day_of(last)).into()));
                }
                _ => {}
            }
        }

        // An open Storno or Terminverschiebung is answered first (Anl. 7 EPK V.13 p.11, p.12).
        // A Storno before a Terminverschiebung when both are open.
        let pending = p.pending().iter().find(|x| matches!(x, Pending::Storno { .. })).or_else(|| p.pending().first());
        if let Some(pending) = pending {
            match pending {
                Pending::Storno { .. } if self.storno_abgelehnt => {
                    let code = if Meldungscode::C1087.applies(Meldungstyp::Abbm, fall) {
                        Meldungscode::C1087
                    } else {
                        Meldungscode::C1086
                    };
                    return Some(to_ag(answer, K::Storno, self.abbm(a, vec![code], None).into()));
                }
                Pending::Storno { .. } => {
                    return Some(to_ag(
                        answer,
                        K::Storno,
                        self.erlm(a, netz, day_of(answer), Meldungscode::C0012).into(),
                    ));
                }
                Pending::Terminverschiebung { .. } if self.terminverschiebung_abgelehnt => {
                    return Some(to_ag(
                        answer,
                        K::Terminverschiebung,
                        self.abbm(a, vec![Meldungscode::C1085], None).into(),
                    ));
                }
                Pending::Terminverschiebung { termin, .. } => {
                    let datum = termin.or_else(|| p.liefertermin())?;
                    return Some(to_ag(answer, K::Standard, self.abm(a, netz, datum).into()));
                }
            }
        }

        match state {
            State::Received if !self.refuse_with.is_empty() => {
                Some(to_ag(answer, K::Standard, self.abbm(a, self.refuse_with.clone(), None).into()))
            }
            State::Received if matches!(fall, G::Get | G::Set) => {
                Some(to_ag(answer, K::Standard, self.erlm(a, netz, day_of(answer), Meldungscode::C0010).into()))
            }
            State::Received => Some(to_ag(answer, K::Standard, self.qeb(a).into())),
            State::Accepted if fall == G::Est => {
                Some(to_ag(answer, K::Standard, self.erlm(a, netz, day_of(answer), Meldungscode::C0010).into()))
            }
            State::Accepted if fall == G::Pv => match leg {
                None => Some(to_losing(answer, self.akm_pv(a, netz, answer).into())),
                Some(PvLeg::Agreed) => {
                    let datum = wunschtermin(&a.geschaeftsfall)?.datum;
                    Some(to_ag(answer, K::Standard, self.abm(a, netz, datum).into()))
                }
                Some(PvLeg::Refused { .. }) => {
                    let abgebend = self.ruem_pv().map(|r| r.abgebender_provider.clone());
                    Some(to_ag(answer, K::Standard, self.abbm(a, vec![Meldungscode::C0041], abgebend).into()))
                }
                Some(PvLeg::AwaitRuem { antwortfrist }) => {
                    // No answer by the antwortfrist: abort (S/PRI 4.4 §5.5.1.1).
                    let at = later(deadline::ruem_pv(*antwortfrist).due, self.latency.answer()).max(answer);
                    let abgebend = ProviderwechselRueckmeldung {
                        providername: Some(netz.abgebender_providername.clone()),
                        zustimmung: JaNein::Nein,
                        antwortcode: None,
                        antworttext: None,
                    };
                    Some(to_ag(at, K::Standard, self.abbm(a, vec![Meldungscode::C0042], Some(abgebend)).into()))
                }
                _ => None,
            },
            State::Accepted => {
                let datum = wunschtermin(&a.geschaeftsfall)?.datum;
                Some(to_ag(answer, K::Standard, self.abm(a, netz, datum).into()))
            }
            State::Confirmed { liefertermin } => {
                let tam_sent = p
                    .history()
                    .iter()
                    .any(|e| matches!(&e.message, Message::Meldung(m) if m.typ() == Meldungstyp::Tam));
                if self.terminanforderung && matches!(fall, G::Neu | G::Lae | G::Pv) && !tam_sent {
                    return Some(to_ag(answer, K::Standard, Meldungsinhalt::Tam(self.terminanforderung(a, netz))));
                }
                let at = answer.max(ten_oclock(*liefertermin));
                Some(to_ag(at, K::Standard, self.erlm(a, netz, day_of(at), Meldungscode::C0010).into()))
            }
            State::ErlmKCheck => {
                Some(to_ag(answer, K::Standard, self.erlm(a, netz, day_of(answer), Meldungscode::C0010).into()))
            }
            State::TamOpen { since } => {
                let at = before(deadline::mtam_due(*since, &self.calendar).due, self.latency.answer(), last);
                Some(to_ag(at, K::Standard, Meldungsinhalt::Mtam(self.terminanforderung(a, netz))))
            }
            State::MtamOpen { since } => {
                let at = before(deadline::abbm_1198_due(*since, &self.calendar).due, self.latency.answer(), last);
                Some(to_ag(at, K::Standard, self.abbm(a, vec![Meldungscode::C1198], None).into()))
            }
            State::Completed { erledigungstermin } => {
                Some(to_ag(answer, K::Standard, self.entm(a, netz, (*erledigungstermin).max(day_of(answer))).into()))
            }
            State::Cancelled => Some(to_ag(answer, K::Standard, self.entm(a, netz, day_of(answer)).into())),
            State::Sent | State::Rejected { .. } | State::Closed | State::Aborted { .. } => None,
        }
    }

    fn qeb(&self, a: &Auftrag) -> Qeb {
        Qeb {
            externe_auftragsnummer: a.externe_auftragsnummer.clone(),
            auftraggebernummer: a.auftraggeber.auftraggebernummer.clone(),
            auftraggebernummer_besteller: a.besteller.as_ref().map(|b| b.auftraggebernummer.clone()),
            vorabstimmungs_id: a.geschaeftsfall.vorabstimmungs_id().cloned(),
            stoerungs_nr_leistungserbringer: None,
            positionen: vec![Position::new(Meldungscode::C0000).into()],
        }
    }

    fn entm(&self, a: &Auftrag, netz: &Netzdaten, day: Date) -> Entm {
        Entm {
            vertragsnummer: self.vertragsnummer(a, netz),
            externe_auftragsnummer: a.externe_auftragsnummer.clone(),
            auftraggebernummer: a.auftraggeber.auftraggebernummer.clone(),
            auftraggebernummer_besteller: a.besteller.as_ref().map(|b| b.auftraggebernummer.clone()),
            vorabstimmungs_id: a.geschaeftsfall.vorabstimmungs_id().cloned(),
            entgelttermin: day,
            anlage: None,
            positionen: vec![Position::new(Meldungscode::C0010)],
        }
    }

    fn akm_pv(&self, a: &Auftrag, netz: &Netzdaten, at: Timestamp) -> AkmPv {
        let geplant = wunschtermin(&a.geschaeftsfall).map_or_else(|| day_of(at), |w| w.datum);
        AkmPv {
            vertragsnummer: netz.abgebender_vertragsnummer.clone(),
            externe_auftragsnummer: a.externe_auftragsnummer.clone(),
            auftraggebernummer: netz.abgebender_auftraggebernummer.clone(),
            vorabstimmungs_id: a.geschaeftsfall.vorabstimmungs_id().cloned(),
            line_id: self.line_id(a, netz),
            anlage: None,
            aufnehmender_provider: ProviderwechselAnkuendigung {
                providername: netz.aufnehmender_providername.clone(),
                uebernahmedatum_geplant: geplant,
                antwortfrist: self.calendar.add_working_days(day_of(at), ANTWORTFRIST_WERKTAGE),
            },
            positionen: vec![Position::new(Meldungscode::C0020)],
        }
    }

    fn abm_pv(&self, a: &Auftrag, netz: &Netzdaten, datum: Date) -> AbmPv {
        AbmPv {
            vertragsnummer: netz.abgebender_vertragsnummer.clone(),
            externe_auftragsnummer: a.externe_auftragsnummer.clone(),
            auftraggebernummer: netz.abgebender_auftraggebernummer.clone(),
            line_id: self.line_id(a, netz),
            aufnehmender_provider: ProviderwechselBestaetigung {
                providername: netz.aufnehmender_providername.clone(),
                uebernahmedatum_verbindlich: datum,
            },
            positionen: vec![Position::new(Meldungscode::C0030)],
        }
    }

    fn erlm_pv(&self, a: &Auftrag, netz: &Netzdaten, day: Date) -> ErlmPv {
        ErlmPv {
            vertragsnummer: netz.abgebender_vertragsnummer.clone(),
            externe_auftragsnummer: a.externe_auftragsnummer.clone(),
            auftraggebernummer: netz.abgebender_auftraggebernummer.clone(),
            erledigungstermin: day,
            positionen: vec![Position::new(Meldungscode::C0010)],
        }
    }

    fn entm_pv(&self, a: &Auftrag, netz: &Netzdaten, day: Date) -> EntmPv {
        EntmPv {
            vertragsnummer: netz.abgebender_vertragsnummer.clone(),
            externe_auftragsnummer: a.externe_auftragsnummer.clone(),
            auftraggebernummer: netz.abgebender_auftraggebernummer.clone(),
            entgelttermin: day,
            positionen: vec![Position::new(Meldungscode::C0010)],
        }
    }

    fn abbm_pv(&self, a: &Auftrag, netz: &Netzdaten) -> AbbmPv {
        AbbmPv {
            vertragsnummer: Some(netz.abgebender_vertragsnummer.clone()),
            externe_auftragsnummer: a.externe_auftragsnummer.clone(),
            auftraggebernummer: netz.abgebender_auftraggebernummer.clone(),
            positionen: vec![Position::new(Meldungscode::C0040)],
        }
    }
}

/// One `latency` before `due`, but not before `start`.
fn before(due: Timestamp, latency: SignedDuration, start: Timestamp) -> Timestamp {
    due.checked_sub(latency).map_or(start, |t| t.max(start))
}

// -- the losing provider -----------------------------------------------------------------------

/// How the losing provider answers the AKM-PV.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rueckmeldung {
    /// RUEM-PV consenting (`zustimmungProviderwechsel` J).
    Zustimmung,
    /// RUEM-PV refusing with this reason (`LnB`, `KVE`, `TAV`, `VnB`, `Vnv`; S/PRI 4.4 §5.5.1.1).
    Ablehnung(Antwortcode),
    /// No RUEM-PV: the network owner aborts after the antwortfrist.
    Keine,
}

/// The simulated losing provider (abgebender Provider) of one provider switch.
///
/// `Debug` shows the configuration, the state and the number of events, never a message's values.
#[derive(Clone, PartialEq, Eq)]
pub struct AbgebenderProvider {
    rueckmeldung: Rueckmeldung,
    signatur: SignaturId,
    latency: Latency,
    providername: Option<Providername>,
    process: Option<Process>,
    stopped: Option<Refusal>,
}

impl fmt::Debug for AbgebenderProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("AbgebenderProvider");
        d.field("rueckmeldung", &self.rueckmeldung).field("latency", &self.latency);
        process_debug(&mut d, self.process.as_ref());
        d.field("stopped", &self.stopped).finish_non_exhaustive()
    }
}

impl AbgebenderProvider {
    /// A losing provider answering with `rueckmeldung`, signing with `signatur`, with the
    /// default [`Latency`] and the synthetic name of [`Netzdaten::synthetic`].
    #[must_use]
    pub fn new(rueckmeldung: Rueckmeldung, signatur: SignaturId) -> Self {
        Self {
            rueckmeldung,
            signatur,
            latency: Latency::default(),
            providername: Netzdaten::synthetic().ok().map(|n| n.abgebender_providername),
            process: None,
            stopped: None,
        }
    }

    /// Uses `latency` for receipts and the RUEM-PV.
    #[must_use]
    pub const fn with_latency(mut self, latency: Latency) -> Self {
        self.latency = latency;
        self
    }

    /// Names itself `providername` in the RUEM-PV.
    #[must_use]
    pub fn with_providername(mut self, providername: Providername) -> Self {
        self.providername = Some(providername);
        self
    }

    /// The process of the switch, once the AKM-PV was received.
    #[must_use]
    pub const fn process(&self) -> Option<&Process> {
        self.process.as_ref()
    }

    /// Why the peer stopped, if its process refused one of its own acts (never in a working
    /// peer).
    #[must_use]
    pub const fn stopped(&self) -> Option<&Refusal> {
        self.stopped.as_ref()
    }

    /// When the RUEM-PV is due, if it is.
    #[must_use]
    pub fn next_due(&self) -> Option<Timestamp> {
        self.plan().map(|(at, _)| at)
    }

    /// Sends the RUEM-PV if it is due by `now`.
    pub fn advance(&mut self, now: Timestamp) -> Vec<Outbound> {
        let mut out = Vec::new();
        for _ in 0..MAX_ACTS {
            let Some((at, meldung)) = self.plan().filter(|(at, _)| *at <= now) else { break };
            let Some(process) = self.process.as_mut() else { break };
            match process.apply(Direction::Sent, at, meldung.clone()) {
                Ok(()) => out.push(Outbound {
                    at,
                    to: Role::Leistungserbringer,
                    payload: Payload::MeldungRequest { control: control(&self.signatur, at), meldung },
                }),
                Err(e) => {
                    self.stopped = Some(e);
                    break;
                }
            }
        }
        out
    }

    /// Reads `xml` and receives it at `at`; bytes that do not read are rejected and answered
    /// with nothing.
    pub fn receive_xml(&mut self, xml: &[u8], at: Timestamp) -> Reply {
        match Payload::from_xml(xml) {
            Ok(payload) => self.receive(&payload, at),
            Err(e) => Reply { outbound: self.advance(at), rejection: Some(Rejection::Unreadable(e)) },
        }
    }

    /// Receives `payload` at `at`: first sends what fell due before, then answers it.
    pub fn receive(&mut self, payload: &Payload, at: Timestamp) -> Reply {
        let mut reply = Reply { outbound: self.advance(at), rejection: None };
        let receipt_at = later(at, self.latency.receipt_within(RECEIPT_MAX));
        reply.rejection = match payload {
            Payload::MeldungRequest { meldung, .. } => {
                let receipt = Teq::new(
                    Meldungscode::Ok,
                    Some(meldung.externe_auftragsnummer().clone()),
                    leg_auftraggebernummer(meldung),
                );
                reply.outbound.push(Outbound {
                    at: receipt_at,
                    to: Role::Leistungserbringer,
                    payload: Payload::MeldungResponse {
                        control: control(&self.signatur, receipt_at),
                        quittung: receipt.clone(),
                    },
                });
                let result = match self.process.as_mut() {
                    None => Process::from_akm_pv(meldung, at).map(|p| self.process = Some(p)),
                    Some(p) => p.apply(Direction::Received, at, meldung.clone()),
                };
                match result {
                    Ok(()) => {
                        if let Some(p) = self.process.as_mut()
                            && let Err(e) = p.apply(Direction::Sent, receipt_at, receipt)
                        {
                            self.stopped = Some(e);
                        }
                        None
                    }
                    Err(e) => Some(Rejection::Refused(e)),
                }
            }
            Payload::AuftragRequest { .. } => Some(Rejection::Refused(Refusal {
                reason: "an order is sent to the Leistungserbringer, not to the losing provider (S/PRI 4.4 §8.3)"
                    .to_owned(),
                answer: None,
            })),
            Payload::AuftragResponse { quittung, .. } | Payload::MeldungResponse { quittung, .. } => {
                match self.process.as_mut() {
                    None => Some(Rejection::Refused(Refusal {
                        reason: "a TEQ, but no AKM-PV was received (Anl. 7 EPK V.13 p.7)".to_owned(),
                        answer: None,
                    })),
                    Some(p) => p.apply(Direction::Received, at, quittung.clone()).err().map(Rejection::Refused),
                }
            }
        };
        reply
    }

    fn plan(&self) -> Option<(Timestamp, Meldung)> {
        if self.stopped.is_some() || self.rueckmeldung == Rueckmeldung::Keine {
            return None;
        }
        let p = self.process.as_ref()?;
        let Some(PvLeg::AwaitRuem { antwortfrist }) = p.pv() else { return None };
        // The RUEM-PV follows the peer's own receipt of the AKM-PV.
        if p.deadlines().iter().any(|d| d.kind == DeadlineKind::TeqResponse) {
            return None;
        }
        let akm = p.history().iter().find_map(|e| match &e.message {
            Message::Meldung(m) => match &m.inhalt {
                Meldungsinhalt::AkmPv(a) => Some((m.auftragstyp, a)),
                _ => None,
            },
            _ => None,
        })?;
        let last = p.history().last()?.at;
        let due = deadline::ruem_pv(*antwortfrist).due;
        let at = later(last, self.latency.answer()).min(due.checked_sub(TOGETHER).unwrap_or(due)).max(last);
        let (zustimmung, antwortcode) = match &self.rueckmeldung {
            Rueckmeldung::Ablehnung(code) => (JaNein::Nein, Some(code.clone())),
            Rueckmeldung::Zustimmung | Rueckmeldung::Keine => (JaNein::Ja, None),
        };
        let (typ, a) = akm;
        let ruem = RuemPv {
            vertragsnummer: a.vertragsnummer.clone(),
            externe_auftragsnummer: a.externe_auftragsnummer.clone(),
            auftraggebernummer: a.auftraggebernummer.clone(),
            vorabstimmungs_id: a.vorabstimmungs_id.clone(),
            line_id: a.line_id.clone(),
            abgebender_provider: ProviderwechselRueckmeldung {
                providername: self.providername.clone(),
                zustimmung,
                antwortcode,
                antworttext: None,
            },
            positionen: vec![Position::new(Meldungscode::C0021)],
        };
        Some((at, Meldung { auftragstyp: typ, inhalt: ruem.into() }))
    }
}

/// The Auftraggebernummer a leg message names: the losing provider's.
fn leg_auftraggebernummer(m: &Meldung) -> Option<Auftraggebernummer> {
    match &m.inhalt {
        Meldungsinhalt::AkmPv(x) => Some(x.auftraggebernummer.clone()),
        Meldungsinhalt::AbmPv(x) => Some(x.auftraggebernummer.clone()),
        Meldungsinhalt::ErlmPv(x) => Some(x.auftraggebernummer.clone()),
        Meldungsinhalt::EntmPv(x) => Some(x.auftraggebernummer.clone()),
        Meldungsinhalt::AbbmPv(x) => Some(x.auftraggebernummer.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn netzdaten_are_valid() {
        let n = Netzdaten::synthetic().unwrap();
        assert_eq!(n.line_id.as_str(), "DEU.NB01.4200000001");
        assert_eq!(n.abgebender_auftraggebernummer.as_str(), "0815");
    }

    #[test]
    fn ten_oclock_is_german_time() {
        let t = ten_oclock("2027-03-01".parse().unwrap());
        assert_eq!(t, "2027-03-01T09:00:00Z".parse().unwrap());
        let t = ten_oclock("2027-07-01".parse().unwrap());
        assert_eq!(t, "2027-07-01T08:00:00Z".parse().unwrap());
    }

    #[test]
    fn before_never_precedes_its_start() {
        let start: Timestamp = "2027-03-01T09:00:00Z".parse().unwrap();
        assert_eq!(before(start, SignedDuration::from_hours(1), start), start);
        assert_eq!(before(Timestamp::MIN, SignedDuration::from_hours(1), start), start);
    }
}
