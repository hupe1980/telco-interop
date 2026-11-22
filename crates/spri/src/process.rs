//! The life of one S/PRI order, as one party sees it.
//!
//! A [`Process`] is created from an order ([`Process::new`]), from the network owner's
//! cancellation announcement ([`Process::from_kdle`]) or, for the losing provider of a provider
//! switch, from the AKM-PV ([`Process::from_akm_pv`]). It is then fed every order, message and
//! technical receipt that is sent or received for it, with its time. It checks that the event
//! is allowed — right sender, right order, right moment for the business case — and refuses it
//! otherwise, leaving itself unchanged. It reports its [`State`], the open Storno and
//! Terminverschiebung ([`Pending`]), the provider-switch leg with the losing provider
//! ([`PvLeg`]), what each side may do next, the open deadlines and its history. It performs no
//! I/O and reads no clock.
//!
//! Sources: the EPKs of Anlage 7 V.13 (`Anl. 7 EPK V.13 p.N`: p.1 NEU, p.2 NEU Konnektivität,
//! p.3 KUE-AG, p.4 KUE-LE, p.5 AEN-LMAE, p.6 LAE, p.7 PV, p.8 EST, p.9 DIAG, p.10 TEQ, p.11 Storno,
//! p.12 Terminverschiebung), the Providervereinbarung S/PRI V4.4 (`S/PRI 4.4 §x`) and the
//! Arbeitshandbuch S/PRI 4.x V2.9.5 (`AH 2.9.5 §x`).
//!
//! ```
//! use spri::process::{Action, Direction, Process, Role, State};
//! use spri::*;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let ext = ExterneAuftragsnummer::new("K-2026-0001")?;
//! let ag = Auftraggebernummer::new("4711")?;
//! let kdle = Meldung {
//!     auftragstyp: Auftragstyp {
//!         geschaeftsfall: GeschaeftsfallCode::KueLe,
//!         aenderungskennzeichen: Aenderungskennzeichen::Standard,
//!         geschaeftsfallart: GeschaeftsfallArt::Kuendigung,
//!     },
//!     inhalt: Meldungsinhalt::Kdle(Kdle {
//!         vertragsnummer: Vertragsnummer::new("V123456")?,
//!         externe_auftragsnummer: ext.clone(),
//!         auftraggebernummer: ag.clone(),
//!         abschaltungstermin: "2027-06-30".parse()?,
//!         abschaltungsgrund: None,
//!         positionen: vec![Position::new(Meldungscode::C8006)],
//!     }),
//! };
//!
//! // The service provider receives the network owner's cancellation and acknowledges it.
//! let mut p = Process::from_kdle(Role::Auftraggeber, &kdle, Direction::Received, "2026-11-23T08:00:00Z".parse()?)?;
//! assert_eq!(p.next_actions(Role::Auftraggeber), [Action::Acknowledge]);
//! p.apply(Direction::Sent, "2026-11-23T08:00:01Z".parse()?, Teq::new(Meldungscode::Ok, Some(ext.clone()), Some(ag.clone())))?;
//! assert_eq!(*p.state(), State::Confirmed { liefertermin: "2027-06-30".parse()? });
//!
//! // An ENTM before the ERLM is refused, and nothing changes.
//! let entm = Meldung {
//!     auftragstyp: kdle.auftragstyp,
//!     inhalt: Meldungsinhalt::Entm(Entm {
//!         vertragsnummer: Vertragsnummer::new("V123456")?,
//!         externe_auftragsnummer: ext,
//!         auftraggebernummer: ag,
//!         auftraggebernummer_besteller: None,
//!         vorabstimmungs_id: None,
//!         entgelttermin: "2027-06-30".parse()?,
//!         anlage: None,
//!         positionen: vec![Position::new(Meldungscode::C0010)],
//!     }),
//! };
//! let before = p.clone();
//! let refusal = p.apply(Direction::Received, "2026-11-24T08:00:00Z".parse()?, entm).unwrap_err();
//! assert!(refusal.reason.contains("ENTM"));
//! assert_eq!(p, before);
//! # Ok(())
//! # }
//! ```
//!
//! ## Interpretations
//!
//! - The Storno and Terminverschiebung window is 36 hours of elapsed time before 0:00 of the
//!   binding date (AH 2.9.5 §3.4, §3.5); the code fact sheets say 3 Arbeitstage. After a TAM the
//!   window does not apply (AH 2.9.5 §3.4).
//! - A TEQ answers the oldest sent item still awaiting one in that direction (the receipt is
//!   the synchronous response of the call, S/PRI 4.4 §11.2.3), and a later message from the
//!   receiver proves delivery of what was sent to it before (§11.2.3.3: the order of messages
//!   is guaranteed).
//! - A negative TEQ to a message of the network owner does not undo it: the network owner
//!   counts it as delivered (S/PRI 4.4 §11.2.3.2; for KDLE also AH 2.9.5 §2.6). The same is
//!   assumed for its messages to the losing provider. A negative TEQ to anything the service
//!   provider or the losing provider sent restores the process to before that item.
//! - Where the EPK and the text disagree, the EPK is followed: a TV is allowed in LAE (Anl. 7
//!   EPK V.13 p.6, against S/PRI 4.4 §5.3.2.4), and an EST draws an ENTM (p.8) but also ends at
//!   the ERLM (S/PRI 4.4 §8.2.4).
//! - An ABBM is accepted from the receipt of the order until the ERLM (S/PRI 4.4 §8.2.5), also
//!   where an EPK draws none; codes with a fixed place are checked (`1198` after an MTAM, `7015`
//!   after an EST TAM, `0041`/`0042` after a negative or missing RUEM-PV).
//! - A message answers the open Storno or TV when its Änderungskennzeichen says so, or by its
//!   code when it carries Standard (`1086`, `1087`, ERLM `0012` for a Storno; `1085`,
//!   `7522`–`7524` for a TV; `1007` when only one of them is open). A new ABM answers the TV.
//! - The ABM-PV is applied after its ABM (they are sent together, S/PRI 4.4 §8.2.13). The
//!   ABBM-PV `0040` is expected whenever the switch ends without taking place after an AKM-PV
//!   (S/PRI 4.4 §5.5.1.2), also on the TAM path, which the EPK does not draw.
//! - A RUEM-PV after the antwortfrist is refused: the network owner aborts (S/PRI 4.4
//!   §5.5.1.1). A RUEM-PV is negative when `zustimmungProviderwechsel` is `N` or its
//!   `antwortcode` is one of `LnB`, `KVE`, `TAV`, `VnB`, `Vnv` (AH 2.9.5 §6.5; Anl. 7 EPK V.13 p.7
//!   writes `LNB`), compared ignoring ASCII case.
//! - A NEU is a Konnektivität (build-out) order when the caller says so
//!   ([`Process::with_konnektivitaet`]) or its first ABM carries a LineID whose line code is
//!   all zeros (S/PRI 4.4 §5.1.6). It completes only after an ABM with a real LineID.
//! - The effects of a KDLE on other orders of the line (S/PRI 4.4 §5.2.2.2) and of an
//!   Auftragsklammer are reported ([`Process::is_aborted_by_kdle`]), not enforced across
//!   processes.

use core::fmt;

use carrier_core::Calendar;
use carrier_core::count::day_of;
use jiff::Timestamp;
use jiff::civil::Date;

use crate::deadline::{self, Deadline};
use crate::{
    Aenderungskennzeichen, Antwortcode, Auftrag, ExterneAuftragsnummer, Geschaeftsfall, GeschaeftsfallCode, JaNein,
    LineId, Meldung, Meldungscode, Meldungsinhalt, Meldungstyp, StoerungsNr, Teq, VorabstimmungsId,
};

/// A party of an S/PRI order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// The service provider (EKP, in a provider switch the EKPauf): sends orders, Storno,
    /// Terminverschiebung, ERLM-K, ZWM-AG, TBK-AG.
    Auftraggeber,
    /// The network owner (NGA, NGAB): sends TEQ, QEB, ABM, ABBM, VZM, TAM, MTAM, ERLM, ENTM,
    /// ZWM-LE, TBK-LE, KDLE and the messages to the losing provider.
    Leistungserbringer,
    /// The losing provider of a provider switch (EKPab): answers the AKM-PV with a RUEM-PV.
    AbgebenderProvider,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Auftraggeber => "Auftraggeber",
            Self::Leistungserbringer => "Leistungserbringer",
            Self::AbgebenderProvider => "abgebender Provider",
        })
    }
}

/// Whether an event was sent by the owner of the process or received by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Sent by the owner.
    Sent,
    /// Received by the owner.
    Received,
}

/// What travels between the parties.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum Message {
    /// An order; with Änderungskennzeichen Storno or Terminverschiebung the Storno or TV of the
    /// order with the same order number.
    Auftrag(Auftrag),
    /// A business message.
    Meldung(Meldung),
    /// A technical receipt.
    Teq(Teq),
}

impl From<Auftrag> for Message {
    fn from(a: Auftrag) -> Self {
        Self::Auftrag(a)
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
    /// Sent or received by the owner.
    pub direction: Direction,
    /// The party that sent it.
    pub sender: Role,
    /// What it was.
    pub message: Message,
}

/// Where an order stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Sent; no technical receipt yet.
    Sent,
    /// Not delivered: the receipt was negative. `0991` asks to send the same order again
    /// later, any other code to correct it first (Anl. 7 EPK V.13 p.10).
    Rejected {
        /// The receipt's code.
        code: Meldungscode,
    },
    /// Delivered (positive TEQ); awaiting the QEB (DIAG: the ERLM).
    Received,
    /// Commercially admissible (QEB); awaiting the ABM (EST: the repair).
    Accepted,
    /// Confirmed for a binding date (ABM; KUE-LE: the KDLE's shutdown date).
    Confirmed {
        /// The binding date (verbindlicher Liefertermin).
        liefertermin: Date,
    },
    /// The network owner asked for a new appointment (TAM).
    TamOpen {
        /// When the TAM was sent.
        since: Timestamp,
    },
    /// The network owner asked again (MTAM).
    MtamOpen {
        /// When the MTAM was sent.
        since: Timestamp,
    },
    /// The service provider reported the service done (ERLM-K); the network owner checks.
    ErlmKCheck,
    /// Realised (ERLM); the ENTM follows (optional in EST).
    Completed {
        /// The completion date.
        erledigungstermin: Date,
    },
    /// Cancelled by a confirmed Storno (ERLM `0012`); the ENTM follows.
    Cancelled,
    /// Billed (ENTM) or, for DIAG, answered (ERLM): nothing more happens.
    Closed,
    /// Aborted with an ABBM: nothing more happens.
    Aborted {
        /// The first code of the ABBM.
        code: Meldungscode,
    },
}

impl State {
    /// Whether nothing more can happen to the order (the provider-switch leg may still owe
    /// its last message).
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Closed | Self::Aborted { .. })
    }

    /// Whether the order is still being worked on: after the receipt, before the ERLM.
    const fn is_open(&self) -> bool {
        matches!(
            self,
            Self::Received
                | Self::Accepted
                | Self::Confirmed { .. }
                | Self::TamOpen { .. }
                | Self::MtamOpen { .. }
                | Self::ErlmKCheck
        )
    }

    /// After the QEB and before the ERLM.
    const fn is_after_qeb(&self) -> bool {
        self.is_open() && !matches!(self, Self::Received)
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sent => f.write_str("sent"),
            Self::Rejected { code } => write!(f, "not delivered ({})", code.as_str()),
            Self::Received => f.write_str("received"),
            Self::Accepted => f.write_str("accepted (QEB)"),
            Self::Confirmed { liefertermin } => write!(f, "confirmed for {liefertermin}"),
            Self::TamOpen { .. } => f.write_str("appointment requested (TAM)"),
            Self::MtamOpen { .. } => f.write_str("appointment requested again (MTAM)"),
            Self::ErlmKCheck => f.write_str("reported done (ERLM-K)"),
            Self::Completed { erledigungstermin } => write!(f, "completed on {erledigungstermin}"),
            Self::Cancelled => f.write_str("cancelled"),
            Self::Closed => f.write_str("closed"),
            Self::Aborted { code } => write!(f, "aborted ({})", code.as_str()),
        }
    }
}

/// An open Storno or Terminverschiebung (at most one of each, Anl. 7 EPK V.13 p.11, p.12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pending {
    /// A Storno awaiting ERLM (cancelled) or ABBM (failed, the order continues).
    Storno {
        /// When it was sent.
        since: Timestamp,
    },
    /// A Terminverschiebung awaiting a new ABM (moved) or ABBM (failed, the order continues).
    Terminverschiebung {
        /// When it was sent.
        since: Timestamp,
        /// The requested new date.
        termin: Option<Date>,
    },
}

impl Pending {
    const fn is_storno(&self) -> bool {
        matches!(self, Self::Storno { .. })
    }
}

/// How the losing provider answered the AKM-PV.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PvAnswer {
    /// RUEM-PV with consent.
    Positive,
    /// RUEM-PV refusing the switch.
    Negative,
    /// No RUEM-PV by the end of the antwortfrist.
    Missing,
}

/// The provider-switch leg between the network owner and the losing provider (Anl. 7 EPK V.13
/// p.7; S/PRI 4.4 §5.5.1). The service provider does not see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PvLeg {
    /// AKM-PV sent; the RUEM-PV is due by the end of `antwortfrist`.
    AwaitRuem {
        /// The answer date of the AKM-PV.
        antwortfrist: Date,
    },
    /// The losing provider refused: ABBM `0041` to the service provider and ABBM-PV `0040`
    /// are due.
    Refused {
        /// The RUEM-PV's `antwortcode`.
        antwortcode: Option<Antwortcode>,
    },
    /// No answer in time: ABBM `0042` and ABBM-PV `0040` are due.
    Missed,
    /// The losing provider agreed (RUEM-PV `0021`).
    Agreed,
    /// The switch date was confirmed (ABM-PV).
    Confirmed {
        /// `uebernahmedatumVerbindlich`.
        uebernahmedatum: Date,
    },
    /// The line was taken over (ERLM-PV); the ENTM-PV follows.
    Completed,
    /// The losing provider's billing ends (ENTM-PV).
    Closed,
    /// The switch was aborted (ABBM-PV); the losing provider's contract stays.
    Aborted {
        /// What the losing provider had answered.
        answer: PvAnswer,
    },
}

impl PvLeg {
    const fn is_open(&self) -> bool {
        !matches!(self, Self::Closed | Self::Aborted { .. })
    }
}

/// Something a party may do next.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Action {
    /// Send the technical receipt (TEQ) for something received.
    Acknowledge,
    /// Send the same order or message again later: the receiver was overloaded (`0991`).
    Resend,
    /// Correct the order or message before sending it again (TEQ `0990`, `0993`, `0995`,
    /// `0999`).
    Correct,
    /// Send a message of this type.
    Send(Meldungstyp),
    /// Abort with an ABBM carrying this code (`0041`, `0042`, `1198`, `7015`).
    Abort(Meldungscode),
    /// Cancel the order (Storno).
    Storno,
    /// Move the date (Terminverschiebung).
    Terminverschiebung,
    /// Answer the open Storno (VZM, ABBM or ERLM).
    AnswerStorno,
    /// Answer the open Terminverschiebung (VZM, ABBM or a new ABM).
    AnswerTerminverschiebung,
    /// Complain about the repair (EST Reklamation with the same order number, S/PRI 4.4 §6.1.5).
    Reklamation,
}

/// Why an event was refused: the message type and the rule, without values.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{reason}")]
pub struct Refusal {
    /// Why, naming the message type and the rule's source.
    pub reason: String,
    /// For a Storno or Terminverschiebung the network owner receives: the ABBM code to answer
    /// with (1007 change marker not allowed, 1085 TV not possible, 1086 Storno already open,
    /// 1087 Storno not possible).
    pub answer: Option<Meldungscode>,
}

fn refuse<T>(reason: impl Into<String>) -> Result<T, Refusal> {
    Err(Refusal { reason: reason.into(), answer: None })
}

fn refuse_with<T>(answer: Meldungscode, reason: impl Into<String>) -> Result<T, Refusal> {
    Err(Refusal { reason: reason.into(), answer: Some(answer) })
}

/// What an awaited receipt is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Order,
    Storno,
    Tv,
    Meldung(Meldungstyp),
}

/// Something sent by a party that could not be delivered.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Undelivered {
    from: Role,
    code: Meldungscode,
}

/// The mutable part, snapshotted before each sent item so a negative receipt can undo it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // independent facts of one order
struct Core {
    state: State,
    liefertermin: Option<Date>,
    wunschtermin: Option<Date>,
    konnektivitaet: bool,
    activated: bool,
    abm_seen: bool,
    pending: Vec<Pending>,
    pv: Option<PvLeg>,
    pv_ext: Option<ExterneAuftragsnummer>,
    abm_pv_owed: bool,
    undelivered: Option<Undelivered>,
    stoerungs_nr: Option<StoerungsNr>,
}

impl Core {
    fn new(state: State) -> Self {
        Self {
            state,
            liefertermin: None,
            wunschtermin: None,
            konnektivitaet: false,
            activated: false,
            abm_seen: false,
            pending: Vec::new(),
            pv: None,
            pv_ext: None,
            abm_pv_owed: false,
            undelivered: None,
            stoerungs_nr: None,
        }
    }
}

/// An item awaiting its receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Awaiting {
    from: Role,
    to: Role,
    at: Timestamp,
    attachment: bool,
    item: Item,
    before: Box<Core>,
}

/// What a handler accepted: the sender, and the item that now awaits a receipt.
struct Accepted {
    sender: Role,
    hop: Option<(Role, Item, bool)>,
}

/// One S/PRI order from the point of view of one party.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    role: Role,
    fall: GeschaeftsfallCode,
    ext: ExterneAuftragsnummer,
    calendar: Calendar,
    core: Core,
    awaiting: Vec<Awaiting>,
    history: Vec<Event>,
}

const fn sender_of(typ: Meldungstyp) -> Role {
    match typ {
        Meldungstyp::ErlmK | Meldungstyp::ZwmAg | Meldungstyp::TbkAg => Role::Auftraggeber,
        Meldungstyp::RuemPv => Role::AbgebenderProvider,
        _ => Role::Leistungserbringer,
    }
}

const fn is_leg(typ: Meldungstyp) -> bool {
    matches!(
        typ,
        Meldungstyp::AkmPv
            | Meldungstyp::RuemPv
            | Meldungstyp::AbmPv
            | Meldungstyp::ErlmPv
            | Meldungstyp::EntmPv
            | Meldungstyp::AbbmPv
    )
}

const fn receiver_of(typ: Meldungstyp) -> Role {
    match typ {
        Meldungstyp::ErlmK | Meldungstyp::ZwmAg | Meldungstyp::TbkAg | Meldungstyp::RuemPv => Role::Leistungserbringer,
        t if is_leg(t) => Role::AbgebenderProvider,
        _ => Role::Auftraggeber,
    }
}

const fn meldung_has_attachment(m: &Meldungsinhalt) -> bool {
    match m {
        Meldungsinhalt::Erlm(x) => x.anlage.is_some(),
        Meldungsinhalt::Entm(x) => x.anlage.is_some(),
        Meldungsinhalt::ZwmLe(x) => x.anlage.is_some(),
        Meldungsinhalt::ZwmAg(x) => x.anlage.is_some(),
        Meldungsinhalt::AkmPv(x) => x.anlage.is_some(),
        _ => false,
    }
}

const fn order_has_attachment(g: &Geschaeftsfall) -> bool {
    match g {
        Geschaeftsfall::Neu(x) => x.anlage.is_some(),
        Geschaeftsfall::KueAg(x) => x.anlage.is_some(),
        Geschaeftsfall::Lae(x) => x.anlage.is_some(),
        Geschaeftsfall::AenLmae(x) => x.anlage.is_some(),
        Geschaeftsfall::Est(x) => x.anlage.is_some(),
        Geschaeftsfall::Pv(x) => x.anlage.is_some(),
        Geschaeftsfall::Get(_) | Geschaeftsfall::Set(_) => false,
    }
}

const fn wunschtermin_of(g: &Geschaeftsfall) -> Option<Date> {
    match g {
        Geschaeftsfall::Neu(x) => Some(x.wunschtermin.datum),
        Geschaeftsfall::KueAg(x) => Some(x.wunschtermin.datum),
        Geschaeftsfall::Lae(x) => Some(x.wunschtermin.datum),
        Geschaeftsfall::AenLmae(x) => Some(x.wunschtermin.datum),
        Geschaeftsfall::Est(x) => Some(x.wunschtermin.datum),
        Geschaeftsfall::Pv(x) => Some(x.wunschtermin.datum),
        Geschaeftsfall::Get(_) | Geschaeftsfall::Set(_) => None,
    }
}

/// A LineID whose line code (the part after the carrier code) is all zeros: the dummy LineID
/// of a Konnektivität order's first ABM (S/PRI 4.4 §5.1.6).
fn is_dummy(line: &LineId) -> bool {
    line.as_str().rsplit('.').next().is_some_and(|c| !c.is_empty() && c.bytes().all(|b| b == b'0'))
}

fn has_code(m: &Meldungsinhalt, codes: &[&str]) -> bool {
    m.codes().iter().any(|c| codes.contains(&c.as_str()))
}

/// The cases with a Storno (all but DIAG and KUE-LE, Anl. 7 EPK V.13 p.4, p.9).
pub(crate) const fn has_storno(fall: GeschaeftsfallCode) -> bool {
    !matches!(fall, GeschaeftsfallCode::Get | GeschaeftsfallCode::Set | GeschaeftsfallCode::KueLe)
}

/// The cases with a Terminverschiebung (NEU, KUE-AG, LAE, PV; Anl. 7 EPK V.13 p.1–3, p.6, p.7;
/// none in AEN-LMAE, S/PRI 4.4 §5.3.1.3, EST, §6.1.3, DIAG and KUE-LE).
pub(crate) const fn has_tv(fall: GeschaeftsfallCode) -> bool {
    matches!(
        fall,
        GeschaeftsfallCode::Neu | GeschaeftsfallCode::KueAg | GeschaeftsfallCode::Lae | GeschaeftsfallCode::Pv
    )
}

/// The cases with the TAM/MTAM/ERLM-K loop (AH 2.9.5 §3.6).
const fn has_tam_loop(fall: GeschaeftsfallCode) -> bool {
    matches!(fall, GeschaeftsfallCode::Neu | GeschaeftsfallCode::Lae | GeschaeftsfallCode::Pv)
}

/// The losing provider's refusal reasons (S/PRI 4.4 §5.5.1.1; AH 2.9.5 §6.5: `LnB`, `KVE`,
/// `TAV`, `VnB`, `Vnv`). Compared ignoring ASCII case: the handbook writes `LnB`, the EPK `LNB`
/// (interpretation).
pub(crate) const REFUSAL_CODES: [&str; 5] = ["LnB", "KVE", "TAV", "VnB", "Vnv"];

/// Whether `code` is one of [`REFUSAL_CODES`].
pub(crate) fn is_refusal_code(code: &Antwortcode) -> bool {
    REFUSAL_CODES.iter().any(|c| c.eq_ignore_ascii_case(code.as_str()))
}

/// A negative RUEM-PV (S/PRI 4.4 §5.5.1.1; Anl. 7 EPK V.13 p.7).
fn ruem_negative(r: &crate::RuemPv) -> bool {
    r.abgebender_provider.zustimmung == JaNein::Nein
        || r.abgebender_provider.antwortcode.as_ref().is_some_and(is_refusal_code)
}

impl Process {
    fn start(role: Role, fall: GeschaeftsfallCode, ext: ExterneAuftragsnummer, core: Core) -> Self {
        Self { role, fall, ext, calendar: Calendar::werktage(), core, awaiting: Vec::new(), history: Vec::new() }
    }

    /// Starts the process of `auftrag` (ÄKZ Standard), sent by the service provider
    /// (`role` [`Role::Auftraggeber`], [`Direction::Sent`]) or received by the network owner
    /// (`role` [`Role::Leistungserbringer`], [`Direction::Received`]) at `at`. Counts Werktage
    /// with [`Calendar::werktage`].
    ///
    /// # Errors
    ///
    /// The direction contradicts the role (only the service provider sends orders), the role
    /// is the losing provider (use [`Process::from_akm_pv`]), or the order is a Storno or
    /// Terminverschiebung (they continue an existing process).
    pub fn new(role: Role, auftrag: &Auftrag, direction: Direction, at: Timestamp) -> Result<Self, Refusal> {
        let expected = match direction {
            Direction::Sent => Role::Auftraggeber,
            Direction::Received => Role::Leistungserbringer,
        };
        if role != expected {
            return refuse("an order is sent by the Auftraggeber to the Leistungserbringer (S/PRI 4.4 §8.3)");
        }
        if auftrag.aenderungskennzeichen != Aenderungskennzeichen::Standard {
            return refuse(
                "a Storno or Terminverschiebung repeats the order number of an existing order (S/PRI 4.4 §5.1.3, §5.1.4)",
            );
        }
        let mut core = Core::new(State::Sent);
        core.wunschtermin = wunschtermin_of(&auftrag.geschaeftsfall);
        let fall = auftrag.geschaeftsfall.code();
        let mut p = Self::start(role, fall, auftrag.externe_auftragsnummer.clone(), core);
        p.awaiting.push(Awaiting {
            from: Role::Auftraggeber,
            to: Role::Leistungserbringer,
            at,
            attachment: order_has_attachment(&auftrag.geschaeftsfall),
            item: Item::Order,
            before: Box::new(p.core.clone()),
        });
        p.history.push(Event { at, direction, sender: Role::Auftraggeber, message: Message::Auftrag(auftrag.clone()) });
        Ok(p)
    }

    /// Starts the process of the network owner's cancellation (KUE-LE, Anl. 7 EPK V.13 p.4):
    /// `kdle` sent by the network owner or received by the service provider at `at`. The
    /// state is [`State::Confirmed`] for the shutdown date; ERLM and ENTM follow.
    ///
    /// The KDLE also aborts the other open orders of the line except EST and DIAG (S/PRI 4.4
    /// §5.2.2.2) and a provider switch (§5.5.1.4); see [`Process::is_aborted_by_kdle`].
    ///
    /// # Errors
    ///
    /// The message is not a KDLE of business case KUE-LE, the direction contradicts the role,
    /// or the shutdown date is not in the future (S/PRI 4.4 §8.2.11).
    pub fn from_kdle(role: Role, kdle: &Meldung, direction: Direction, at: Timestamp) -> Result<Self, Refusal> {
        let Meldungsinhalt::Kdle(k) = &kdle.inhalt else {
            return refuse("a KUE-LE process starts with a KDLE (Anl. 7 EPK V.13 p.4)");
        };
        if kdle.auftragstyp.geschaeftsfall != GeschaeftsfallCode::KueLe {
            return refuse("a KDLE belongs to business case KUE-LE (Anl. 7 EPK V.13 p.4)");
        }
        let expected = match direction {
            Direction::Sent => Role::Leistungserbringer,
            Direction::Received => Role::Auftraggeber,
        };
        if role != expected {
            return refuse("a KDLE is sent by the Leistungserbringer to the Auftraggeber (S/PRI 4.4 §8.2.11)");
        }
        if k.abschaltungstermin <= day_of(at) {
            return refuse("a KDLE names a shutdown date in the future (S/PRI 4.4 §8.2.11)");
        }
        let mut core = Core::new(State::Confirmed { liefertermin: k.abschaltungstermin });
        core.liefertermin = Some(k.abschaltungstermin);
        let mut p = Self::start(role, GeschaeftsfallCode::KueLe, k.externe_auftragsnummer.clone(), core);
        p.awaiting.push(Awaiting {
            from: Role::Leistungserbringer,
            to: Role::Auftraggeber,
            at,
            attachment: false,
            item: Item::Meldung(Meldungstyp::Kdle),
            before: Box::new(p.core.clone()),
        });
        p.history.push(Event {
            at,
            direction,
            sender: Role::Leistungserbringer,
            message: Message::Meldung(kdle.clone()),
        });
        Ok(p)
    }

    /// Starts the losing provider's view of a provider switch from the AKM-PV it received at
    /// `at` (Anl. 7 EPK V.13 p.7). The process follows only the leg: RUEM-PV, ABM-PV, ERLM-PV,
    /// ENTM-PV, ABBM-PV and their receipts; its [`State`] mirrors the switch (confirmed by the
    /// ABM-PV, completed by the ERLM-PV, closed by the ENTM-PV, aborted by the ABBM-PV).
    ///
    /// # Errors
    ///
    /// The message is not an AKM-PV of business case PV.
    pub fn from_akm_pv(akm_pv: &Meldung, at: Timestamp) -> Result<Self, Refusal> {
        let Meldungsinhalt::AkmPv(a) = &akm_pv.inhalt else {
            return refuse("the losing provider's process starts with an AKM-PV (Anl. 7 EPK V.13 p.7)");
        };
        if akm_pv.auftragstyp.geschaeftsfall != GeschaeftsfallCode::Pv {
            return refuse("an AKM-PV belongs to business case PV (Anl. 7 EPK V.13 p.7)");
        }
        let mut core = Core::new(State::Accepted);
        core.pv = Some(PvLeg::AwaitRuem { antwortfrist: a.aufnehmender_provider.antwortfrist });
        core.pv_ext = Some(a.externe_auftragsnummer.clone());
        core.wunschtermin = Some(a.aufnehmender_provider.uebernahmedatum_geplant);
        let mut p =
            Self::start(Role::AbgebenderProvider, GeschaeftsfallCode::Pv, a.externe_auftragsnummer.clone(), core);
        p.awaiting.push(Awaiting {
            from: Role::Leistungserbringer,
            to: Role::AbgebenderProvider,
            at,
            attachment: a.anlage.is_some(),
            item: Item::Meldung(Meldungstyp::AkmPv),
            before: Box::new(p.core.clone()),
        });
        p.history.push(Event {
            at,
            direction: Direction::Received,
            sender: Role::Leistungserbringer,
            message: Message::Meldung(akm_pv.clone()),
        });
        Ok(p)
    }

    /// Marks a NEU as a Konnektivität (build-out) order (Anl. 7 EPK V.13 p.2; S/PRI 4.4
    /// §5.1.6): no Terminverschiebung before the first ABM, and completion only after the
    /// activation ABM with a real LineID. Has no effect on other business cases.
    #[must_use]
    pub fn with_konnektivitaet(mut self) -> Self {
        if self.fall == GeschaeftsfallCode::Neu {
            self.core.konnektivitaet = true;
            for w in &mut self.awaiting {
                w.before.konnektivitaet = true;
            }
        }
        self
    }

    /// Counts Werktage with `calendar` (default [`Calendar::werktage`]), e.g. with a closed day
    /// added.
    #[must_use]
    pub fn with_calendar(mut self, calendar: Calendar) -> Self {
        self.calendar = calendar;
        self
    }

    /// The owner's side.
    #[must_use]
    pub const fn role(&self) -> Role {
        self.role
    }

    /// The business case.
    #[must_use]
    pub const fn geschaeftsfall(&self) -> GeschaeftsfallCode {
        self.fall
    }

    /// The order number every message of the process repeats.
    #[must_use]
    pub const fn externe_auftragsnummer(&self) -> &ExterneAuftragsnummer {
        &self.ext
    }

    /// The state.
    #[must_use]
    pub const fn state(&self) -> &State {
        &self.core.state
    }

    /// The WBCI Vorabstimmungs-ID of the order — of the AKM-PV in the losing provider's view —
    /// if it carries one (S/PRI 4.4 §5.5.1.1).
    #[must_use]
    pub fn vorabstimmungs_id(&self) -> Option<&VorabstimmungsId> {
        self.history.first().and_then(|e| match &e.message {
            Message::Auftrag(a) => a.geschaeftsfall.vorabstimmungs_id(),
            Message::Meldung(Meldung { inhalt: Meldungsinhalt::AkmPv(a), .. }) => a.vorabstimmungs_id.as_ref(),
            Message::Meldung(_) | Message::Teq(_) => None,
        })
    }

    /// The binding date of the latest ABM (KUE-LE: the shutdown date).
    #[must_use]
    pub const fn liefertermin(&self) -> Option<Date> {
        self.core.liefertermin
    }

    /// Whether the order is a Konnektivität order (declared or detected from the first ABM).
    #[must_use]
    pub const fn is_konnektivitaet(&self) -> bool {
        self.core.konnektivitaet
    }

    /// The open Storno and Terminverschiebung.
    #[must_use]
    pub fn pending(&self) -> &[Pending] {
        &self.core.pending
    }

    /// The provider-switch leg with the losing provider, once the AKM-PV is sent (not visible
    /// to the service provider).
    #[must_use]
    pub const fn pv(&self) -> Option<&PvLeg> {
        self.core.pv.as_ref()
    }

    /// Every accepted event, in order.
    #[must_use]
    pub fn history(&self) -> &[Event] {
        &self.history
    }

    /// Whether a KDLE for this order's line aborts this order: an open order of any business
    /// case but EST, DIAG and KUE-LE (S/PRI 4.4 §5.2.2.2, §5.5.1.4). The caller applies it.
    #[must_use]
    pub const fn is_aborted_by_kdle(&self) -> bool {
        !matches!(
            self.fall,
            GeschaeftsfallCode::Est | GeschaeftsfallCode::Get | GeschaeftsfallCode::Set | GeschaeftsfallCode::KueLe
        ) && (self.core.state.is_open() || matches!(self.core.state, State::Sent))
    }

    /// Applies `message`, sent or received by the owner at `at`.
    ///
    /// # Errors
    ///
    /// [`Refusal`] naming the message type and the rule; the process is unchanged.
    pub fn apply(&mut self, direction: Direction, at: Timestamp, message: impl Into<Message>) -> Result<(), Refusal> {
        let message = message.into();
        let before = self.core.clone();
        let before_awaiting = self.awaiting.clone();
        let result = match &message {
            Message::Auftrag(a) => self.auftrag(a, direction, at),
            Message::Meldung(m) => self.meldung(m, direction, at),
            Message::Teq(t) => self.teq(t, direction),
        };
        match result {
            Ok(Accepted { sender, hop }) => {
                if let Some((to, item, attachment)) = hop {
                    // The receiver answering proves delivery of what was sent to it before
                    // (S/PRI 4.4 §11.2.3.3).
                    self.awaiting.retain(|w| w.to != sender);
                    if self.core.undelivered.as_ref().is_some_and(|u| u.from == sender) {
                        self.core.undelivered = None;
                    }
                    self.awaiting.push(Awaiting { from: sender, to, at, attachment, item, before: Box::new(before) });
                }
                self.history.push(Event { at, direction, sender, message });
                Ok(())
            }
            Err(e) => {
                self.core = before;
                self.awaiting = before_awaiting;
                Err(e)
            }
        }
    }

    fn check_direction(&self, direction: Direction, from: Role, to: Role, what: &str) -> Result<(), Refusal> {
        match direction {
            Direction::Sent if self.role != from => refuse(format!("{what} is sent by the {from}")),
            Direction::Received if self.role != to => refuse(format!("{what} is addressed to the {to}")),
            _ => Ok(()),
        }
    }

    fn auftrag(&mut self, a: &Auftrag, direction: Direction, at: Timestamp) -> Result<Accepted, Refusal> {
        self.check_direction(direction, Role::Auftraggeber, Role::Leistungserbringer, "an order")?;
        if a.externe_auftragsnummer != self.ext {
            return refuse("an order with another order number starts its own process (S/PRI 4.4 §11.4)");
        }
        if a.geschaeftsfall.code() != self.fall {
            return refuse("a Storno or Terminverschiebung keeps the business case of the order (S/PRI 4.4 §5.1.3)");
        }
        let attachment = order_has_attachment(&a.geschaeftsfall);
        let item = match a.aenderungskennzeichen {
            Aenderungskennzeichen::Standard => {
                self.resend_or_reklamation(a)?;
                Item::Order
            }
            Aenderungskennzeichen::Storno => {
                self.check_storno(at)?;
                self.core.pending.push(Pending::Storno { since: at });
                Item::Storno
            }
            Aenderungskennzeichen::Terminverschiebung => {
                let termin = a.termin_neu.or_else(|| wunschtermin_of(&a.geschaeftsfall));
                self.check_tv(at, termin)?;
                if self.core.state == State::Accepted {
                    // Before the ABM a TV just replaces the requested date (AH 2.9.5 §3.4).
                    self.core.wunschtermin = termin;
                }
                self.core.pending.push(Pending::Terminverschiebung { since: at, termin });
                Item::Tv
            }
        };
        Ok(Accepted { sender: Role::Auftraggeber, hop: Some((Role::Leistungserbringer, item, attachment)) })
    }

    fn resend_or_reklamation(&mut self, a: &Auftrag) -> Result<(), Refusal> {
        match (&self.core.state, &a.geschaeftsfall) {
            (State::Rejected { .. }, _) => {
                // Not delivered: the same order number is sent again (Anl. 7 EPK V.13 p.10).
                self.core.state = State::Sent;
                self.core.wunschtermin = wunschtermin_of(&a.geschaeftsfall);
                Ok(())
            }
            (State::Completed { .. } | State::Closed, Geschaeftsfall::Est(e)) if e.reklamation.is_some() => {
                if let (Some(known), Some(r)) = (&self.core.stoerungs_nr, &e.reklamation)
                    && *known != r.stoerungs_nr_leistungserbringer
                {
                    return refuse(
                        "an EST Reklamation names the network owner's ticket of the repair (S/PRI 4.4 §6.1.5)",
                    );
                }
                let stoerungs_nr = self.core.stoerungs_nr.take();
                self.core = Core::new(State::Sent);
                self.core.stoerungs_nr = stoerungs_nr;
                self.core.wunschtermin = Some(e.wunschtermin.datum);
                Ok(())
            }
            (State::Completed { .. } | State::Closed, Geschaeftsfall::Est(_)) => refuse(
                "after a repair the same order number returns only as a Reklamation (Anl. 7 EPK V.13 p.8; S/PRI 4.4 §6.1.5)",
            ),
            _ => refuse("the order was already sent; a repeated order is refused (S/PRI 4.4 §11.4)"),
        }
    }

    fn has_pending(&self, storno: bool) -> bool {
        self.core.pending.iter().any(|p| p.is_storno() == storno)
    }

    fn take_pending(&mut self, storno: bool) -> bool {
        let before = self.core.pending.len();
        self.core.pending.retain(|p| p.is_storno() != storno);
        self.core.pending.len() != before
    }

    /// The Storno rules (Anl. 7 EPK V.13 p.11; AH 2.9.5 §3.5).
    fn check_storno(&self, at: Timestamp) -> Result<(), Refusal> {
        if !has_storno(self.fall) {
            return refuse_with(Meldungscode::C1007, "no Storno in DIAG and KUE-LE (Anl. 7 EPK V.13 p.4, p.9)");
        }
        if !self.core.state.is_after_qeb() {
            return refuse_with(
                Meldungscode::C1087,
                "a Storno is sent after the QEB and before the ERLM (AH 2.9.5 §3.5)",
            );
        }
        if self.has_pending(true) {
            return refuse_with(Meldungscode::C1086, "a Storno is already open (Anl. 7 EPK V.13 p.11)");
        }
        if let State::Confirmed { liefertermin } = self.core.state {
            let latest = deadline::storno_latest(liefertermin);
            if latest.is_overdue(at) {
                return refuse_with(Meldungscode::C1087, format!("Storno too late: {}", latest.rule.text));
            }
        }
        Ok(())
    }

    /// The Terminverschiebung rules (Anl. 7 EPK V.13 p.12; AH 2.9.5 §3.4, §3.4.1).
    fn check_tv(&self, at: Timestamp, termin: Option<Date>) -> Result<(), Refusal> {
        if !has_tv(self.fall) {
            return refuse_with(
                Meldungscode::C1007,
                "no Terminverschiebung in AEN-LMAE, EST, DIAG and KUE-LE (S/PRI 4.4 §5.3.1.3, §6.1.3; Anl. 7 EPK V.13 p.9)",
            );
        }
        if !self.core.state.is_after_qeb() {
            return refuse_with(
                Meldungscode::C1085,
                "a Terminverschiebung is sent after the QEB and before the ERLM (AH 2.9.5 §3.4)",
            );
        }
        if self.has_pending(false) {
            return refuse_with(Meldungscode::C1085, "a Terminverschiebung is already open (Anl. 7 EPK V.13 p.12)");
        }
        if self.core.konnektivitaet && !self.core.abm_seen {
            return refuse_with(
                Meldungscode::C1085,
                "no Terminverschiebung before the first ABM of a Konnektivität order (AH 2.9.5 §3.4.1; S/PRI 4.4 §5.1.4)",
            );
        }
        let current = self.core.liefertermin.or(self.core.wunschtermin);
        if let (Some(new), Some(old)) = (termin, current) {
            if new == old {
                return refuse_with(Meldungscode::C1085, "a Terminverschiebung names a new date (S/PRI 4.4 §5.1.4)");
            }
            if self.fall == GeschaeftsfallCode::Pv && new < old {
                return refuse_with(
                    Meldungscode::C1085,
                    "in PV the date cannot move earlier after the positive pre-agreement (AH 2.9.5 §3.4)",
                );
            }
        }
        if let State::Confirmed { liefertermin } = self.core.state {
            let latest = deadline::terminverschiebung_latest(liefertermin);
            if latest.is_overdue(at) {
                return refuse_with(Meldungscode::C1085, format!("Terminverschiebung too late: {}", latest.rule.text));
            }
        }
        Ok(())
    }

    /// Which open request a message of the network owner answers: by its Änderungskennzeichen,
    /// else by its code. `Some(true)` is the Storno, `Some(false)` the TV.
    fn answers(&self, m: &Meldung) -> Option<bool> {
        match m.auftragstyp.aenderungskennzeichen {
            Aenderungskennzeichen::Storno => return Some(true),
            Aenderungskennzeichen::Terminverschiebung => return Some(false),
            Aenderungskennzeichen::Standard => {}
        }
        match &m.inhalt {
            Meldungsinhalt::Abbm(_) => {
                if has_code(&m.inhalt, &["1086", "1087"]) {
                    Some(true)
                } else if has_code(&m.inhalt, &["1085", "7523"])
                    || (self.has_pending(false) && has_code(&m.inhalt, &["7522", "7524"]))
                {
                    Some(false)
                } else if has_code(&m.inhalt, &["1007"]) && self.core.pending.len() == 1 {
                    self.core.pending.first().map(Pending::is_storno)
                } else {
                    None
                }
            }
            Meldungsinhalt::Erlm(_) if has_code(&m.inhalt, &["0012"]) => Some(true),
            _ => None,
        }
    }

    fn meldung(&mut self, m: &Meldung, direction: Direction, at: Timestamp) -> Result<Accepted, Refusal> {
        let typ = m.typ();
        let (from, to) = (sender_of(typ), receiver_of(typ));
        self.check_direction(direction, from, to, &format!("a {typ}"))?;
        if m.auftragstyp.geschaeftsfall != self.fall {
            return refuse(format!("a {typ} of another business case"));
        }
        let ext = m.externe_auftragsnummer();
        if is_leg(typ) {
            if self.core.pv_ext.as_ref().is_some_and(|e| e != ext) {
                return refuse(format!("a {typ} refers to another provider switch"));
            }
            self.leg(m, typ, at)?;
        } else {
            if *ext != self.ext {
                return refuse(format!("a {typ} refers to another order number"));
            }
            if self.core.state.is_terminal() {
                return refuse(format!("no {typ} after the order has ended (S/PRI 4.4 §8.2.4, §8.2.5)"));
            }
            match self.core.state {
                State::Rejected { .. } => {
                    return refuse(format!("no {typ}: the order was not delivered (Anl. 7 EPK V.13 p.10)"));
                }
                State::Sent if from == Role::Leistungserbringer => self.core.state = State::Received,
                _ => {}
            }
            self.order_meldung(m, typ, at)?;
        }
        Ok(Accepted { sender: from, hop: Some((to, Item::Meldung(typ), meldung_has_attachment(&m.inhalt))) })
    }

    #[allow(clippy::too_many_lines)] // one transition table
    fn order_meldung(&mut self, m: &Meldung, typ: Meldungstyp, at: Timestamp) -> Result<(), Refusal> {
        use GeschaeftsfallCode as G;
        let fall = self.fall;
        let state = self.core.state.clone();
        let diag = matches!(fall, G::Get | G::Set);
        match &m.inhalt {
            Meldungsinhalt::Qeb(q) => {
                if diag || fall == G::KueLe {
                    return refuse("no QEB in DIAG and KUE-LE (Anl. 7 EPK V.13 p.4, p.9)");
                }
                if state != State::Received {
                    return refuse("a QEB is the first message, once, before the ABM (S/PRI 4.4 §8.2.1, §8.2.2)");
                }
                self.core.state = State::Accepted;
                self.core.stoerungs_nr.clone_from(&q.stoerungs_nr_leistungserbringer);
            }
            Meldungsinhalt::Vzm(_) => {
                if diag || fall == G::KueLe {
                    return refuse("no VZM in DIAG and KUE-LE (Anl. 7 EPK V.13 p.4, p.9)");
                }
                match self.answers(m) {
                    Some(true) if fall == G::Est => {
                        return refuse("no VZM to a Storno in EST (Anl. 7 EPK V.13 p.11)");
                    }
                    Some(storno) if !self.has_pending(storno) => {
                        return refuse(
                            "a VZM to a Storno or Terminverschiebung needs one open (Anl. 7 EPK V.13 p.11, p.12)",
                        );
                    }
                    None if !state.is_open() => {
                        return refuse("a VZM comes between the receipt and the ERLM (S/PRI 4.4 §8.2.6)");
                    }
                    _ => {}
                }
            }
            Meldungsinhalt::Abm(a) => self.abm(m, a, &state)?,
            Meldungsinhalt::Abbm(_) => self.abbm(m, &state, at)?,
            Meldungsinhalt::Erlm(e) => {
                if self.answers(m) == Some(false) {
                    return refuse("a Terminverschiebung is not answered with an ERLM (Anl. 7 EPK V.13 p.12)");
                }
                if self.answers(m) == Some(true) {
                    if !self.take_pending(true) {
                        return refuse("an ERLM to a Storno needs an open Storno (Anl. 7 EPK V.13 p.11)");
                    }
                    self.core.pending.clear();
                    self.core.state = State::Cancelled;
                    return Ok(());
                }
                let allowed = match fall {
                    G::Get | G::Set => state == State::Received,
                    G::KueLe => matches!(state, State::Confirmed { .. }),
                    G::Est => state == State::Accepted,
                    _ => matches!(state, State::Confirmed { .. } | State::ErlmKCheck),
                };
                if !allowed {
                    return refuse(match fall {
                        G::Get | G::Set => "a DIAG ends with ERLM or ABBM after the receipt (Anl. 7 EPK V.13 p.9)",
                        G::Est => "an EST is completed after the QEB, outside a TAM (Anl. 7 EPK V.13 p.8)",
                        _ => {
                            "an ERLM follows the ABM, or the check of an ERLM-K (Anl. 7 EPK V.13 p.1, p.3, p.5–7; S/PRI 4.4 §8.2.3)"
                        }
                    });
                }
                if self.core.konnektivitaet && !self.core.activated {
                    return refuse(
                        "a Konnektivität order completes after the activation ABM with a real LineID (Anl. 7 EPK V.13 p.2; S/PRI 4.4 §5.1.6)",
                    );
                }
                self.core.pending.clear();
                self.core.state =
                    if diag { State::Closed } else { State::Completed { erledigungstermin: e.erledigungstermin } };
            }
            Meldungsinhalt::Entm(_) => {
                if diag {
                    return refuse("no ENTM in DIAG (Anl. 7 EPK V.13 p.9)");
                }
                if !matches!(state, State::Completed { .. } | State::Cancelled) {
                    return refuse("an ENTM follows the ERLM (S/PRI 4.4 §8.2.3, §8.2.4)");
                }
                self.core.state = State::Closed;
            }
            Meldungsinhalt::Tam(_) => {
                let ok = match fall {
                    G::Est => state == State::Accepted,
                    f if has_tam_loop(f) => matches!(state, State::Confirmed { .. } | State::ErlmKCheck),
                    _ => false,
                };
                if !ok {
                    return refuse(
                        "a TAM comes after the ABM (EST: after the QEB) in NEU, LAE, PV and EST (S/PRI 4.4 §8.2.7; AH 2.9.5 §3.6)",
                    );
                }
                self.core.state = State::TamOpen { since: at };
            }
            Meldungsinhalt::Mtam(_) => {
                if !has_tam_loop(fall) || !matches!(state, State::TamOpen { .. }) {
                    return refuse("an MTAM follows an unanswered TAM in NEU, LAE and PV (AH 2.9.5 §3.6)");
                }
                self.core.state = State::MtamOpen { since: at };
            }
            Meldungsinhalt::ErlmK(_) => {
                let ok = has_tam_loop(fall)
                    && (matches!(state, State::TamOpen { .. } | State::MtamOpen { .. })
                        || (fall == G::Pv && matches!(state, State::Confirmed { .. })));
                if !ok {
                    return refuse(
                        "an ERLM-K answers a TAM or MTAM in NEU, LAE and PV, in PV also after the ABM (AH 2.9.5 §3.6; Anl. 7 EPK V.13 p.7)",
                    );
                }
                self.core.state = State::ErlmKCheck;
            }
            Meldungsinhalt::TbkLe(_) => {
                if fall != G::Est || state != State::Accepted {
                    return refuse("a TBK-LE comes in an EST after the QEB (Anl. 7 EPK V.13 p.8)");
                }
            }
            Meldungsinhalt::TbkAg(_) => {
                if fall != G::Est || !matches!(state, State::TamOpen { .. }) {
                    return refuse("a TBK-AG answers the TAM of an EST (Anl. 7 EPK V.13 p.8)");
                }
                self.core.state = State::Accepted;
            }
            Meldungsinhalt::ZwmLe(_) | Meldungsinhalt::ZwmAg(_) => {
                if !matches!(fall, G::Neu | G::Est) || !state.is_after_qeb() {
                    return refuse(format!(
                        "a {typ} comes between the QEB and the ERLM in NEU and EST (S/PRI 4.4 §8.2.9, §8.3.1; Anl. 7 EPK V.13 p.1, p.8)"
                    ));
                }
            }
            Meldungsinhalt::Kdle(_) => {
                return refuse("a KDLE starts its own process (Anl. 7 EPK V.13 p.4)");
            }
            Meldungsinhalt::AkmPv(_)
            | Meldungsinhalt::RuemPv(_)
            | Meldungsinhalt::AbmPv(_)
            | Meldungsinhalt::ErlmPv(_)
            | Meldungsinhalt::EntmPv(_)
            | Meldungsinhalt::AbbmPv(_) => {}
        }
        Ok(())
    }

    fn abm(&mut self, m: &Meldung, a: &crate::Abm, state: &State) -> Result<(), Refusal> {
        use GeschaeftsfallCode as G;
        let fall = self.fall;
        if matches!(fall, G::Est | G::Get | G::Set | G::KueLe) {
            return refuse("no ABM in EST, DIAG and KUE-LE (Anl. 7 EPK V.13 p.4, p.8, p.9)");
        }
        if m.auftragstyp.aenderungskennzeichen != Aenderungskennzeichen::Standard {
            return refuse("an ABM carries the Änderungskennzeichen Standard, also after a TV (AH 2.9.5 §3.4)");
        }
        let tv_open = self.has_pending(false);
        match state {
            State::Accepted => {
                if self.role == Role::Leistungserbringer
                    && fall == G::Pv
                    && !matches!(self.core.pv, Some(PvLeg::Agreed | PvLeg::Confirmed { .. }))
                {
                    return refuse("in PV the ABM follows a positive RUEM-PV (Anl. 7 EPK V.13 p.7)");
                }
            }
            State::Confirmed { .. } | State::TamOpen { .. } | State::MtamOpen { .. } | State::ErlmKCheck => {
                let repeat = m.codes().iter().any(|c| {
                    ["6012", "8500", "8501", "8502"].contains(&c.as_str()) && c.applies(Meldungstyp::Abm, fall)
                });
                if !repeat && !tv_open {
                    return refuse(
                        "a repeated ABM carries 6012 or 8500–8502 or confirms a Terminverschiebung (Anl. 7 EPK V.13 p.1, p.3, p.6, p.7, p.12)",
                    );
                }
            }
            State::Received => return refuse("the QEB comes before the ABM (S/PRI 4.4 §8.2.2)"),
            _ => return refuse("an ABM comes after the QEB and before the ERLM (S/PRI 4.4 §8.2.2, §8.2.3)"),
        }
        let dummy = is_dummy(&a.anschluss.line_id);
        if fall == G::Neu && !self.core.abm_seen && dummy {
            self.core.konnektivitaet = true;
        }
        if self.core.konnektivitaet && !dummy {
            self.core.activated = true;
        }
        self.take_pending(false);
        self.core.abm_seen = true;
        let date = a.verbindlicher_liefertermin.datum;
        self.core.liefertermin = Some(date);
        self.core.state = State::Confirmed { liefertermin: date };
        if self.role == Role::Leistungserbringer && self.core.pv.is_some() {
            self.core.abm_pv_owed = true;
        }
        Ok(())
    }

    fn abbm(&mut self, m: &Meldung, state: &State, at: Timestamp) -> Result<(), Refusal> {
        use GeschaeftsfallCode as G;
        let fall = self.fall;
        if fall == G::KueLe {
            return refuse("no ABBM in KUE-LE (Anl. 7 EPK V.13 p.4)");
        }
        if let Some(storno) = self.answers(m) {
            if !self.take_pending(storno) {
                return refuse("an ABBM to a Storno or Terminverschiebung needs one open (Anl. 7 EPK V.13 p.11, p.12)");
            }
            return Ok(());
        }
        if !state.is_open() {
            return refuse("an ABBM comes between the receipt and the ERLM (S/PRI 4.4 §8.2.5)");
        }
        let le = self.role == Role::Leistungserbringer;
        if has_code(&m.inhalt, &["1198"]) && !matches!(state, State::MtamOpen { .. }) {
            return refuse("ABBM 1198 follows an unanswered MTAM (AH 2.9.5 §3.6)");
        }
        if has_code(&m.inhalt, &["7015"]) && !(fall == G::Est && matches!(state, State::TamOpen { .. })) {
            return refuse("ABBM 7015 follows an unanswered TAM of an EST (Anl. 7 EPK V.13 p.8)");
        }
        if has_code(&m.inhalt, &["0041"]) {
            let ok = if le {
                matches!(self.core.pv, Some(PvLeg::Refused { .. } | PvLeg::Aborted { answer: PvAnswer::Negative }))
            } else {
                fall == G::Pv && *state == State::Accepted
            };
            if !ok {
                return refuse("ABBM 0041 follows a negative RUEM-PV (Anl. 7 EPK V.13 p.7)");
            }
        }
        if has_code(&m.inhalt, &["0042"]) {
            let ok = if le {
                match &self.core.pv {
                    Some(PvLeg::AwaitRuem { antwortfrist }) => {
                        let missed = deadline::ruem_pv(*antwortfrist).is_overdue(at);
                        if missed {
                            self.core.pv = Some(PvLeg::Missed);
                        }
                        missed
                    }
                    Some(PvLeg::Missed | PvLeg::Aborted { answer: PvAnswer::Missing }) => true,
                    _ => false,
                }
            } else {
                fall == G::Pv && *state == State::Accepted
            };
            if !ok {
                return refuse("ABBM 0042 follows the antwortfrist of the AKM-PV without RUEM-PV (S/PRI 4.4 §5.5.1.1)");
            }
        }
        let code = m.codes().first().map_or_else(|| Meldungscode::parse(""), |c| (*c).clone());
        self.core.pending.clear();
        self.core.state = State::Aborted { code };
        Ok(())
    }

    #[allow(clippy::too_many_lines)] // one transition table
    fn leg(&mut self, m: &Meldung, typ: Meldungstyp, at: Timestamp) -> Result<(), Refusal> {
        if self.fall != GeschaeftsfallCode::Pv {
            return refuse(format!("a {typ} belongs to a provider switch (Anl. 7 EPK V.13 p.7)"));
        }
        let le = self.role == Role::Leistungserbringer;
        let ekp_ab = self.role == Role::AbgebenderProvider;
        let leg = self.core.pv.clone();
        match &m.inhalt {
            Meldungsinhalt::AkmPv(a) => {
                if leg.is_some() {
                    return refuse("one AKM-PV per order; no new AKM-PV on a TV (S/PRI 4.4 §5.5.1.3)");
                }
                if self.core.state != State::Accepted {
                    return refuse("the AKM-PV follows the QEB, before the ABM (Anl. 7 EPK V.13 p.7)");
                }
                self.core.pv = Some(PvLeg::AwaitRuem { antwortfrist: a.aufnehmender_provider.antwortfrist });
                self.core.pv_ext = Some(a.externe_auftragsnummer.clone());
            }
            Meldungsinhalt::RuemPv(r) => {
                let Some(PvLeg::AwaitRuem { antwortfrist }) = leg else {
                    return refuse("a RUEM-PV answers the AKM-PV, once (Anl. 7 EPK V.13 p.7)");
                };
                if deadline::ruem_pv(antwortfrist).is_overdue(at) {
                    return refuse(
                        "a RUEM-PV after the antwortfrist comes too late: the network owner aborts (S/PRI 4.4 §5.5.1.1)",
                    );
                }
                if le && !self.core.state.is_open() {
                    return refuse(
                        "after the order ended the losing provider's messages are discarded (S/PRI 4.4 §5.5.1.2)",
                    );
                }
                self.core.pv = Some(if ruem_negative(r) {
                    PvLeg::Refused { antwortcode: r.abgebender_provider.antwortcode.clone() }
                } else {
                    PvLeg::Agreed
                });
            }
            Meldungsinhalt::AbmPv(a) => {
                if !matches!(leg, Some(PvLeg::Agreed | PvLeg::Confirmed { .. })) {
                    return refuse("an ABM-PV follows a positive RUEM-PV (Anl. 7 EPK V.13 p.7)");
                }
                if le && !self.core.abm_pv_owed {
                    return refuse("an ABM-PV is sent together with an ABM (S/PRI 4.4 §8.2.13)");
                }
                self.core.abm_pv_owed = false;
                let uebernahmedatum = a.aufnehmender_provider.uebernahmedatum_verbindlich;
                self.core.pv = Some(PvLeg::Confirmed { uebernahmedatum });
                if ekp_ab {
                    self.core.liefertermin = Some(uebernahmedatum);
                    self.core.state = State::Confirmed { liefertermin: uebernahmedatum };
                }
            }
            Meldungsinhalt::ErlmPv(e) => {
                if !matches!(leg, Some(PvLeg::Confirmed { .. })) {
                    return refuse("an ERLM-PV follows the ABM-PV (Anl. 7 EPK V.13 p.7)");
                }
                if le && !matches!(self.core.state, State::Completed { .. } | State::Closed) {
                    return refuse("an ERLM-PV is sent together with the ERLM (Anl. 7 EPK V.13 p.7)");
                }
                self.core.pv = Some(PvLeg::Completed);
                if ekp_ab {
                    self.core.state = State::Completed { erledigungstermin: e.erledigungstermin };
                }
            }
            Meldungsinhalt::EntmPv(_) => {
                if leg != Some(PvLeg::Completed) {
                    return refuse("an ENTM-PV follows the ERLM-PV (Anl. 7 EPK V.13 p.7)");
                }
                self.core.pv = Some(PvLeg::Closed);
                if ekp_ab {
                    self.core.state = State::Closed;
                }
            }
            Meldungsinhalt::AbbmPv(_) => {
                let Some(open) = leg.filter(PvLeg::is_open) else {
                    return refuse("an ABBM-PV ends an open provider-switch leg after the AKM-PV (S/PRI 4.4 §5.5.1.2)");
                };
                let answer = match &open {
                    PvLeg::Refused { .. } => PvAnswer::Negative,
                    PvLeg::AwaitRuem { .. } | PvLeg::Missed => PvAnswer::Missing,
                    _ => PvAnswer::Positive,
                };
                if le {
                    let ok = match &open {
                        PvLeg::Refused { .. } | PvLeg::Missed => true,
                        PvLeg::AwaitRuem { antwortfrist } => {
                            deadline::ruem_pv(*antwortfrist).is_overdue(at) || self.ended_without_switch()
                        }
                        PvLeg::Agreed | PvLeg::Confirmed { .. } => self.ended_without_switch(),
                        _ => false,
                    };
                    if !ok {
                        return refuse(
                            "an ABBM-PV follows the end of the order without switch: ABBM, Storno, negative or missing RUEM-PV (Anl. 7 EPK V.13 p.7)",
                        );
                    }
                }
                self.core.pv = Some(PvLeg::Aborted { answer });
                if ekp_ab {
                    let code = m.codes().first().map_or_else(|| Meldungscode::parse(""), |c| (*c).clone());
                    self.core.state = State::Aborted { code };
                }
            }
            _ => {}
        }
        Ok(())
    }

    const fn ended_without_switch(&self) -> bool {
        matches!(self.core.state, State::Aborted { .. } | State::Cancelled)
    }

    fn teq(&mut self, t: &Teq, direction: Direction) -> Result<Accepted, Refusal> {
        let role = self.role;
        let pos = match direction {
            Direction::Sent => self.awaiting.iter().position(|w| w.to == role),
            Direction::Received => self.awaiting.iter().position(|w| w.from == role),
        };
        let Some(pos) = pos else {
            return refuse("a TEQ acknowledges an order or message awaiting one (Anl. 7 EPK V.13 p.10)");
        };
        let Some(item) = self.awaiting.get(pos).cloned() else {
            return refuse("a TEQ acknowledges an order or message awaiting one (Anl. 7 EPK V.13 p.10)");
        };
        let leg = matches!(item.item, Item::Meldung(typ) if is_leg(typ));
        let expected = if leg { self.core.pv_ext.as_ref().unwrap_or(&self.ext) } else { &self.ext };
        if t.externe_auftragsnummer.as_ref().is_some_and(|e| e != expected) {
            return refuse("a TEQ refers to another order number");
        }
        if t.is_ok() && item.item == Item::Order && self.core.state == State::Sent {
            self.core.state = State::Received;
        }
        if t.is_ok() || item.from == Role::Leistungserbringer {
            // A negative receipt to a message of the network owner is only a hint: it counts as
            // delivered (S/PRI 4.4 §11.2.3.2; AH 2.9.5 §2.6).
            self.awaiting.remove(pos);
        } else {
            // Not delivered: undo it and everything after it (Anl. 7 EPK V.13 p.10).
            let code = t
                .positionen
                .iter()
                .find(|p| !p.code.is_ok())
                .map_or(Meldungscode::TECHNISCHER_FEHLER, |p| p.code.clone());
            self.core = *item.before;
            self.awaiting.truncate(pos);
            if item.item == Item::Order && matches!(self.core.state, State::Sent | State::Rejected { .. }) {
                self.core.state = State::Rejected { code: code.clone() };
            }
            self.core.undelivered = Some(Undelivered { from: item.from, code });
        }
        Ok(Accepted { sender: item.to, hop: None })
    }

    /// What `role` may do next.
    #[must_use]
    #[allow(clippy::too_many_lines)] // one table per role
    pub fn next_actions(&self, role: Role) -> Vec<Action> {
        use GeschaeftsfallCode as G;
        use Meldungstyp as T;
        let mut out = Vec::new();
        if self.awaiting.iter().any(|w| w.to == role) {
            out.push(Action::Acknowledge);
        }
        if let Some(u) = &self.core.undelivered
            && u.from == role
        {
            out.push(if u.code == Meldungscode::UEBERLASTET { Action::Resend } else { Action::Correct });
        }
        let fall = self.fall;
        let state = &self.core.state;
        let pv = self.core.pv.as_ref();
        match role {
            Role::Auftraggeber => {
                if state.is_after_qeb() {
                    if has_storno(fall) && !self.has_pending(true) {
                        out.push(Action::Storno);
                    }
                    if has_tv(fall) && !self.has_pending(false) && (!self.core.konnektivitaet || self.core.abm_seen) {
                        out.push(Action::Terminverschiebung);
                    }
                    if matches!(fall, G::Neu | G::Est) {
                        out.push(Action::Send(T::ZwmAg));
                    }
                }
                match state {
                    State::TamOpen { .. } | State::MtamOpen { .. } if has_tam_loop(fall) => {
                        out.push(Action::Send(T::ErlmK));
                    }
                    State::Confirmed { .. } if fall == G::Pv => out.push(Action::Send(T::ErlmK)),
                    State::TamOpen { .. } if fall == G::Est => out.push(Action::Send(T::TbkAg)),
                    State::Completed { .. } | State::Closed if fall == G::Est => out.push(Action::Reklamation),
                    _ => {}
                }
            }
            Role::Leistungserbringer => {
                for p in &self.core.pending {
                    out.push(if p.is_storno() { Action::AnswerStorno } else { Action::AnswerTerminverschiebung });
                }
                match state {
                    State::Received if matches!(fall, G::Get | G::Set) => {
                        out.extend([Action::Send(T::Erlm), Action::Send(T::Abbm)]);
                    }
                    State::Received => out.extend([Action::Send(T::Qeb), Action::Send(T::Vzm), Action::Send(T::Abbm)]),
                    State::Accepted if fall == G::Est => out.extend([
                        Action::Send(T::Erlm),
                        Action::Send(T::Tam),
                        Action::Send(T::TbkLe),
                        Action::Send(T::ZwmLe),
                        Action::Send(T::Vzm),
                        Action::Send(T::Abbm),
                    ]),
                    State::Accepted if fall == G::Pv => match pv {
                        None => out.push(Action::Send(T::AkmPv)),
                        Some(PvLeg::Agreed) => out.extend([Action::Send(T::Abm), Action::Send(T::Abbm)]),
                        Some(PvLeg::Refused { .. }) => out.push(Action::Abort(Meldungscode::C0041)),
                        Some(PvLeg::Missed) => out.push(Action::Abort(Meldungscode::C0042)),
                        _ => {}
                    },
                    State::Accepted => out.extend([Action::Send(T::Abm), Action::Send(T::Vzm), Action::Send(T::Abbm)]),
                    State::Confirmed { .. } if fall == G::KueLe => out.push(Action::Send(T::Erlm)),
                    State::Confirmed { .. } | State::ErlmKCheck => {
                        if !self.core.konnektivitaet || self.core.activated {
                            out.push(Action::Send(T::Erlm));
                        }
                        if has_tam_loop(fall) {
                            out.push(Action::Send(T::Tam));
                        }
                        out.extend([Action::Send(T::Vzm), Action::Send(T::Abbm)]);
                    }
                    State::TamOpen { .. } if fall == G::Est => out.push(Action::Abort(Meldungscode::C7015)),
                    State::TamOpen { .. } => out.push(Action::Send(T::Mtam)),
                    State::MtamOpen { .. } => out.push(Action::Abort(Meldungscode::C1198)),
                    State::Completed { .. } | State::Cancelled => out.push(Action::Send(T::Entm)),
                    _ => {}
                }
                if self.core.abm_pv_owed {
                    out.push(Action::Send(T::AbmPv));
                }
                match pv {
                    Some(PvLeg::Confirmed { .. }) if matches!(state, State::Completed { .. } | State::Closed) => {
                        out.push(Action::Send(T::ErlmPv));
                    }
                    Some(PvLeg::Completed) => out.push(Action::Send(T::EntmPv)),
                    Some(PvLeg::Refused { .. } | PvLeg::Missed) => out.push(Action::Send(T::AbbmPv)),
                    Some(l) if l.is_open() && self.ended_without_switch() => out.push(Action::Send(T::AbbmPv)),
                    _ => {}
                }
            }
            Role::AbgebenderProvider => {
                if matches!(pv, Some(PvLeg::AwaitRuem { .. })) {
                    out.push(Action::Send(T::RuemPv));
                }
            }
        }
        out
    }

    /// The deadlines running now: receipts, the Storno and TV windows, the TAM and MTAM
    /// answers and the losing provider's answer.
    #[must_use]
    pub fn deadlines(&self) -> Vec<Deadline> {
        let mut out: Vec<Deadline> =
            self.awaiting.iter().map(|w| deadline::teq_response(w.at, w.to, w.attachment)).collect();
        match &self.core.state {
            State::Confirmed { liefertermin } if self.role != Role::AbgebenderProvider => {
                if has_storno(self.fall) && !self.has_pending(true) {
                    out.push(deadline::storno_latest(*liefertermin));
                }
                if has_tv(self.fall) && !self.has_pending(false) {
                    out.push(deadline::terminverschiebung_latest(*liefertermin));
                }
            }
            State::TamOpen { since } if has_tam_loop(self.fall) => out.push(deadline::mtam_due(*since, &self.calendar)),
            State::MtamOpen { since } => out.push(deadline::abbm_1198_due(*since, &self.calendar)),
            _ => {}
        }
        if let Some(PvLeg::AwaitRuem { antwortfrist }) = &self.core.pv {
            out.push(deadline::ruem_pv(*antwortfrist));
        }
        out.sort_by_key(|d| d.due);
        out
    }

    /// The obligations overdue at `now`; the Storno and TV windows are not obligations. An
    /// overdue [`deadline::DeadlineKind::RuemPv`] means the network owner owes ABBM `0042` and
    /// ABBM-PV `0040`.
    #[must_use]
    pub fn overdue(&self, now: Timestamp) -> Vec<Deadline> {
        self.deadlines().into_iter().filter(|d| !d.is_window() && d.is_overdue(now)).collect()
    }
}
