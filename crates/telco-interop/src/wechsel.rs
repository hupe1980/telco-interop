//! One provider switch across WBCI and S/PRI.
//!
//! The gaining provider agrees the switch with the losing provider over WBCI (VA, RUEM-VA,
//! AKM-TR), then orders the line from the network owner over S/PRI with the WBCI
//! Vorabstimmungs-ID, which „wird im gesamten Prozessablauf als eindeutiges
//! Identifikationsmerkmal verwendet" (S/PRI 4.4 §5.5.1.1; the sequence of p. 27). A [`Wechsel`]
//! holds the one [`wbci::Process`] and every [`spri::process::Process`] carrying its id, and recomputes
//! from them, on every call:
//!
//! - [`Wechsel::timeline`]: every event of both interfaces in time order ([`Eintrag`]);
//! - [`Wechsel::befunde`]: where the two interfaces disagree ([`Befund`], rules below);
//! - [`Wechsel::deadlines`]: the deadlines of both interfaces, sorted ([`Frist`]);
//! - [`Wechsel::next_actions`]: what a party may do next on either interface ([`Aktion`]).
//!
//! The switch has no state of its own: the processes stay the single source of truth, and
//! the caller keeps feeding them through [`Wechsel::wbci_mut`] and [`Wechsel::spri_mut`].
//!
//! ## Parties
//!
//! [`Partei`] unifies the roles: `EkpAuf` is the WBCI EKPauf and the S/PRI Auftraggeber,
//! `EkpAbg` the WBCI EKPabg and the S/PRI abgebender Provider, `Netzbetreiber` the S/PRI
//! Leistungserbringer. In a `KUE-AG` with the Vorabstimmungs-ID the Auftraggeber is the losing
//! provider cancelling its line for the switch (S/PRI 4.4 p. 26, „KUE/VA-ID"), so there the
//! Auftraggeber is `EkpAbg` (an interpretation of the figure on p. 27).
//!
//! ## Rules
//!
//! | Rule | Finding | Source | Party |
//! |---|---|---|---|
//! | `wechsel.nicht-abgestimmt` | an S/PRI order was placed while the WBCI pre-agreement was not agreed (requested, rejected, or not yet answered with a RUEM-VA) | S/PRI 4.4 §5.5.1.1 | the order's Auftraggeber |
//! | `wechsel.leitung` | the PV's `lineId` is not among the lines (LineID) the AKM-TR takes over (when it names any) | Anl. 1 V5.4.1 §2.2; S/PRI 4.4 §5.5.1.1 | EKPauf |
//! | `wechsel.termin` | the order's requested date (before any Terminverschiebung) or the binding date of its ABM differs from the WBCI switch date | Anl. 1 V5.4.1 §2.5; S/PRI 4.4 §5.5.1.1, §5.5.1.3 | the Auftraggeber, or the network owner for the ABM |
//! | `wechsel.tv-zuerst-wbci` | an S/PRI Terminverschiebung names a date other than the WBCI switch date in force when it was sent: the WBCI postponement (TVS-VA, ERLM `8003`) comes first | Anl. 1 V5.4.1 §2.11 | EKPauf |
//! | `wechsel.storno` | the WBCI pre-agreement was agreed and is now cancelled or changed, but an S/PRI order is still open and not being cancelled | Anl. 1 V5.4.1 §2.9, §2.10; S/PRI 4.4 §5.5.1.2 | the order's Auftraggeber |
//! | `wechsel.kontinuitaet` | a PV or NEU is completed (ERLM, ERLM-PV) more than one working day of [`Calendar::wbci`] after the WBCI switch date | § 59 Abs. 1 TKG (interpretation) | network owner |
//!
//! The continuity rule is an **interpretation**: § 59 Abs. 1 TKG requires that the service is
//! „nicht länger als einen Arbeitstag unterbrochen"; the real outage is not visible on the
//! interfaces, so the S/PRI completion date stands in for the end of the outage.
//!
//! Findings, timeline entries and their `Display` and `Debug` carry ids, dates, message types
//! and codes only — never a name, address or phone number.
//!
//! ```
//! use telco_interop::{Partei, Wechsel, wbci};
//! use wbci::{Anfrage, Direction, Meldung, Payload, Process, Role, RuemVa, Timestamp};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let xml = include_str!("../../wbci/tests/fixtures/va-kue-mrn.xml");
//! let Payload::AnfrageRequest { anfrage: Anfrage::Vorabstimmung(va), .. } = Payload::from_xml(xml.as_bytes())? else {
//!     return Err("not a pre-agreement".into());
//! };
//! let sent: Timestamp = "2026-11-23T08:00:00Z".parse()?;
//! let mut wechsel = Wechsel::new(Process::new(Role::EkpAuf, va.clone(), Direction::Sent, sent)?);
//!
//! // The losing provider agrees …
//! let ruem = RuemVa::zustimmung(&va, va.kundenwunschtermin);
//! wechsel.wbci_mut().apply(Direction::Received, "2026-11-24T09:00:00Z".parse()?, Meldung::from(ruem))?;
//!
//! let lines: Vec<String> = wechsel.timeline().iter().map(ToString::to_string).collect();
//! assert_eq!(lines, ["Mo 23.11.2026 09:00 WBCI EKPauf → VA-KUE-MRN", "Di 24.11.2026 10:00 WBCI EKPabg → RUEM-VA 8001"]);
//! // … and nothing disagrees yet: no S/PRI order has been placed.
//! assert!(wechsel.befunde().is_empty());
//! assert!(wechsel.next_actions(Partei::EkpAuf).iter().any(|a| matches!(a, telco_interop::Aktion::Wbci(_))));
//! # Ok(())
//! # }
//! ```

use core::fmt;

use carrier_core::calendar::berlin;
use carrier_core::count::{Berlin, Day};

use crate::{Calendar, LineId, VorabstimmungsId};

use spri::process::{Message as SMessage, Pending, Role as SRole, State as SState};
use spri::{Aenderungskennzeichen, ExterneAuftragsnummer, Geschaeftsfall as SFall, GeschaeftsfallCode, Meldungsinhalt};
use wbci::process::{Message as WMessage, State as WState, Takeover};
use wbci::{Anfrage, Date, Geschaeftsfall as WFall, Meldung as WMeldung, Role as WRole, Timestamp, UebernahmeLeitung};

/// The interface an event or deadline belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Schnittstelle {
    /// WBCI: provider ↔ provider.
    Wbci,
    /// S/PRI: service provider ↔ network owner (and the losing provider in a PV).
    Spri,
}

impl fmt::Display for Schnittstelle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Wbci => "WBCI",
            Self::Spri => "S/PRI",
        })
    }
}

/// A party of the switch, across both interfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Partei {
    /// The gaining provider: WBCI EKPauf, S/PRI Auftraggeber.
    EkpAuf,
    /// The losing provider: WBCI EKPabg, S/PRI abgebender Provider (and the Auftraggeber of a
    /// `KUE-AG` for the switch).
    EkpAbg,
    /// The network owner: S/PRI Leistungserbringer.
    Netzbetreiber,
}

impl fmt::Display for Partei {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EkpAuf => "EKPauf",
            Self::EkpAbg => "EKPabg",
            Self::Netzbetreiber => "Netzbetreiber",
        })
    }
}

impl From<WRole> for Partei {
    fn from(r: WRole) -> Self {
        match r {
            WRole::EkpAuf => Self::EkpAuf,
            WRole::EkpAbg => Self::EkpAbg,
        }
    }
}

/// The party an S/PRI role plays in the switch, for an order of business case `fall`.
const fn partei_spri(role: SRole, fall: GeschaeftsfallCode) -> Partei {
    match role {
        SRole::Auftraggeber if matches!(fall, GeschaeftsfallCode::KueAg) => Partei::EkpAbg,
        SRole::Auftraggeber => Partei::EkpAuf,
        SRole::Leistungserbringer => Partei::Netzbetreiber,
        SRole::AbgebenderProvider => Partei::EkpAbg,
    }
}

/// Whether an event was sent or received by the owner of the process it was recorded in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Richtung {
    /// Sent by the owner.
    Gesendet,
    /// Received by the owner.
    Empfangen,
}

/// One event of the switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Eintrag {
    /// When it was sent or received.
    pub at: Timestamp,
    /// The interface.
    pub schnittstelle: Schnittstelle,
    /// The party that sent it.
    pub partei: Partei,
    /// Sent or received, as the process it comes from recorded it.
    pub richtung: Richtung,
    /// The request, order or message type, e.g. `VA-KUE-MRN`, `RUEM-VA`, `PV`, `ABM-PV`,
    /// `Storno`, `Terminverschiebung`, `TEQ`.
    pub nachricht: &'static str,
    /// The codes of its positions, e.g. `8001`.
    pub codes: Vec<String>,
}

impl fmt::Display for Eintrag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let z = self.at.to_zoned_datetime(berlin());
        write!(
            f,
            "{} {:02}:{:02} {} {} → {}",
            Day(z.date()),
            z.hour(),
            z.minute(),
            self.schnittstelle,
            self.partei,
            self.nachricht
        )?;
        for c in &self.codes {
            write!(f, " {c}")?;
        }
        Ok(())
    }
}

/// Where the two interfaces disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Befund {
    /// The rule, e.g. `wechsel.termin`.
    pub regel: &'static str,
    /// Where the rule is written.
    pub quelle: &'static str,
    /// Who has to act.
    pub partei: Partei,
    /// What disagrees: ids, dates and codes only.
    pub text: String,
}

impl fmt::Display for Befund {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}) — {}: {}", self.regel, self.quelle, self.partei, self.text)
    }
}

/// The cross-interface rules.
pub mod regeln {
    /// An S/PRI order placed while the WBCI pre-agreement was not agreed.
    pub const NICHT_ABGESTIMMT: (&str, &str) = ("wechsel.nicht-abgestimmt", "S/PRI 4.4 §5.5.1.1");
    /// A PV for a line the AKM-TR does not take over.
    pub const LEITUNG: (&str, &str) = ("wechsel.leitung", "Anl. 1 V5.4.1 §2.2; S/PRI 4.4 §5.5.1.1");
    /// An S/PRI date other than the WBCI switch date.
    pub const TERMIN: (&str, &str) = ("wechsel.termin", "Anl. 1 V5.4.1 §2.5; S/PRI 4.4 §5.5.1.1, §5.5.1.3");
    /// An S/PRI Terminverschiebung before the WBCI postponement.
    pub const TV_ZUERST_WBCI: (&str, &str) = ("wechsel.tv-zuerst-wbci", "Anl. 1 V5.4.1 §2.11");
    /// A WBCI cancellation or change while an S/PRI order is open.
    pub const STORNO: (&str, &str) = ("wechsel.storno", "Anl. 1 V5.4.1 §2.9, §2.10; S/PRI 4.4 §5.5.1.2");
    /// A completion more than one working day after the switch date (interpretation).
    pub const KONTINUITAET: (&str, &str) = ("wechsel.kontinuitaet", "§ 59 Abs. 1 TKG (interpretation)");
}

/// Why an S/PRI process was not attached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mismatch {
    /// The Vorabstimmungs-ID of the switch.
    pub erwartet: VorabstimmungsId,
    /// The Vorabstimmungs-ID of the order, if it carries one.
    pub gefunden: Option<VorabstimmungsId>,
    /// The refused process, handed back so nothing is lost.
    pub prozess: Box<spri::process::Process>,
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.gefunden {
            Some(id) => write!(f, "the S/PRI order carries Vorabstimmungs-ID {id}, the switch is {}", self.erwartet),
            None => write!(f, "the S/PRI order carries no Vorabstimmungs-ID, the switch is {}", self.erwartet),
        }?;
        f.write_str(" (S/PRI 4.4 §5.5.1.1)")
    }
}

impl std::error::Error for Mismatch {}

/// A deadline of either interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FristDetail {
    /// A WBCI deadline.
    Wbci(wbci::deadline::Deadline),
    /// An S/PRI deadline.
    Spri(spri::deadline::Deadline),
}

/// A deadline of the switch, tagged with its interface and party.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frist {
    /// The interface.
    pub schnittstelle: Schnittstelle,
    /// Who must act, or whose window it is.
    pub partei: Partei,
    /// The S/PRI order it belongs to.
    pub auftrag: Option<ExterneAuftragsnummer>,
    /// The act must happen before this instant.
    pub due: Timestamp,
    /// The deadline with its rule and arithmetic.
    pub detail: FristDetail,
}

impl Frist {
    /// The rule's id, source and text.
    #[must_use]
    pub const fn rule(&self) -> (&'static str, &'static str, &'static str) {
        match &self.detail {
            FristDetail::Wbci(d) => (d.rule.id, d.rule.source, d.rule.text),
            FristDetail::Spri(d) => (d.rule.id, d.rule.source, d.rule.text),
        }
    }
}

impl fmt::Display for Frist {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (_, source, text) = self.rule();
        write!(f, "{} {}: by {} — {text} ({source})", self.schnittstelle, self.partei, Berlin(self.due))
    }
}

/// Something a party may do next.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Aktion {
    /// A WBCI action.
    Wbci(wbci::process::Action),
    /// An S/PRI action on an order.
    Spri {
        /// The order.
        auftrag: ExterneAuftragsnummer,
        /// The action.
        action: spri::process::Action,
    },
    /// Place the S/PRI order with the Vorabstimmungs-ID: the switch is agreed and the line
    /// takeover announced, but no S/PRI order is attached (S/PRI 4.4 §5.5.1.1, p. 27).
    SpriAuftragStellen,
}

/// One provider switch: a WBCI pre-agreement and the S/PRI orders carrying its id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wechsel {
    wbci: wbci::Process,
    spri: Vec<spri::process::Process>,
}

impl Wechsel {
    /// A switch of the pre-agreement `wbci`, with no S/PRI order yet.
    #[must_use]
    pub const fn new(wbci: wbci::Process) -> Self {
        Self { wbci, spri: Vec::new() }
    }

    /// Attaches an S/PRI process whose order carries this switch's Vorabstimmungs-ID.
    ///
    /// # Errors
    ///
    /// [`Mismatch`] when the order carries another or no Vorabstimmungs-ID (S/PRI 4.4
    /// §5.5.1.1); the switch is unchanged.
    pub fn attach(&mut self, process: spri::process::Process) -> Result<(), Mismatch> {
        let erwartet = self.vorabstimmungs_id();
        match process.vorabstimmungs_id() {
            Some(id) if id == erwartet => {
                self.spri.push(process);
                Ok(())
            }
            gefunden => {
                let gefunden = gefunden.cloned();
                Err(Mismatch { erwartet: erwartet.clone(), gefunden, prozess: Box::new(process) })
            }
        }
    }

    /// The Vorabstimmungs-ID of the switch.
    #[must_use]
    pub const fn vorabstimmungs_id(&self) -> &VorabstimmungsId {
        &self.wbci.vorabstimmung().id
    }

    /// The WBCI process.
    #[must_use]
    pub const fn wbci(&self) -> &wbci::Process {
        &self.wbci
    }

    /// The WBCI process, to apply further events.
    pub const fn wbci_mut(&mut self) -> &mut wbci::Process {
        &mut self.wbci
    }

    /// The attached S/PRI processes, in the order they were attached.
    #[must_use]
    pub fn spri(&self) -> &[spri::process::Process] {
        &self.spri
    }

    /// The attached S/PRI processes, to apply further events.
    pub fn spri_mut(&mut self) -> &mut [spri::process::Process] {
        &mut self.spri
    }

    /// The agreed switch date: the RUEM-VA's, moved by every accepted postponement; after a
    /// cancellation, the last date that was agreed.
    #[must_use]
    pub fn wechseltermin(&self) -> Option<Date> {
        match self.wbci.state() {
            WState::Agreed { wechseltermin } => Some(*wechseltermin),
            WState::Changed { wechseltermin } => *wechseltermin,
            WState::Cancelled => self.agreed_dates().last().map(|(_, d)| *d),
            WState::Requested | WState::Rejected { .. } => None,
        }
    }

    /// Every event of both interfaces, in time order (stable: WBCI first, then the S/PRI
    /// processes in the order attached). An event recorded by two attached views of the same
    /// order (e.g. the Auftraggeber's and the network owner's) appears once.
    #[must_use]
    pub fn timeline(&self) -> Vec<Eintrag> {
        let mut out: Vec<Eintrag> = self
            .wbci
            .history()
            .iter()
            .map(|e| Eintrag {
                at: e.at,
                schnittstelle: Schnittstelle::Wbci,
                partei: e.sender.into(),
                richtung: match e.direction {
                    wbci::Direction::Sent => Richtung::Gesendet,
                    wbci::Direction::Received => Richtung::Empfangen,
                },
                nachricht: wbci_label(&e.message),
                codes: wbci_codes(&e.message),
            })
            .collect();
        let mut seen: Vec<&spri::process::Event> = Vec::new();
        for p in &self.spri {
            let fall = p.geschaeftsfall();
            for e in p.history() {
                if seen.iter().any(|s| s.at == e.at && s.sender == e.sender && s.message == e.message) {
                    continue;
                }
                seen.push(e);
                out.push(Eintrag {
                    at: e.at,
                    schnittstelle: Schnittstelle::Spri,
                    partei: partei_spri(e.sender, fall),
                    richtung: match e.direction {
                        spri::process::Direction::Sent => Richtung::Gesendet,
                        spri::process::Direction::Received => Richtung::Empfangen,
                    },
                    nachricht: spri_label(&e.message),
                    codes: spri_codes(&e.message),
                });
            }
        }
        out.sort_by_key(|e| e.at);
        out
    }

    /// Where the two interfaces disagree, by the rules of the [module](self) in table order;
    /// the same finding from two views of one order appears once.
    #[must_use]
    pub fn befunde(&self) -> Vec<Befund> {
        let mut out = Vec::new();
        for p in &self.spri {
            self.nicht_abgestimmt(p, &mut out);
        }
        for p in &self.spri {
            self.leitung(p, &mut out);
        }
        for p in &self.spri {
            self.termin(p, &mut out);
        }
        for p in &self.spri {
            self.tv_zuerst_wbci(p, &mut out);
        }
        for p in &self.spri {
            self.storno(p, &mut out);
        }
        for p in &self.spri {
            self.kontinuitaet(p, &mut out);
        }
        let mut unique: Vec<Befund> = Vec::with_capacity(out.len());
        for b in out {
            if !unique.contains(&b) {
                unique.push(b);
            }
        }
        unique
    }

    /// The deadlines of both interfaces, sorted by due time; the same deadline from two views
    /// of one order appears once.
    #[must_use]
    pub fn deadlines(&self) -> Vec<Frist> {
        let mut out: Vec<Frist> = self
            .wbci
            .deadlines()
            .into_iter()
            .map(|d| Frist {
                schnittstelle: Schnittstelle::Wbci,
                partei: d.party.into(),
                auftrag: None,
                due: d.due,
                detail: FristDetail::Wbci(d),
            })
            .collect();
        for p in &self.spri {
            let fall = p.geschaeftsfall();
            for d in p.deadlines() {
                let f = Frist {
                    schnittstelle: Schnittstelle::Spri,
                    partei: partei_spri(d.party, fall),
                    auftrag: Some(p.externe_auftragsnummer().clone()),
                    due: d.due,
                    detail: FristDetail::Spri(d),
                };
                let dup = out.iter().any(|o| {
                    o.auftrag == f.auftrag && o.due == f.due && o.partei == f.partei && o.rule().0 == f.rule().0
                });
                if !dup {
                    out.push(f);
                }
            }
        }
        out.sort_by_key(|f| f.due);
        out
    }

    /// What `partei` may do next on either interface. WBCI answers for both providers; an
    /// S/PRI process answers for its owner and — the network owner's view, which sees every
    /// message — for every party.
    #[must_use]
    pub fn next_actions(&self, partei: Partei) -> Vec<Aktion> {
        let mut out: Vec<Aktion> = match partei {
            Partei::EkpAuf => self.wbci.next_actions(WRole::EkpAuf),
            Partei::EkpAbg => self.wbci.next_actions(WRole::EkpAbg),
            Partei::Netzbetreiber => Vec::new(),
        }
        .into_iter()
        .map(Aktion::Wbci)
        .collect();
        if partei == Partei::EkpAuf
            && self.spri.is_empty()
            && matches!(self.wbci.state(), WState::Agreed { .. })
            && matches!(self.wbci.takeover(), Some(Takeover::Announced { uebernahme: true, .. }))
        {
            out.push(Aktion::SpriAuftragStellen);
        }
        for p in &self.spri {
            let fall = p.geschaeftsfall();
            for role in [SRole::Auftraggeber, SRole::Leistungserbringer, SRole::AbgebenderProvider] {
                let sees = p.role() == role || p.role() == SRole::Leistungserbringer;
                if !sees || partei_spri(role, fall) != partei {
                    continue;
                }
                for action in p.next_actions(role) {
                    let a = Aktion::Spri { auftrag: p.externe_auftragsnummer().clone(), action };
                    if !out.contains(&a) {
                        out.push(a);
                    }
                }
            }
        }
        out
    }

    // -- the rules ------------------------------------------------------------------------

    /// The (time, date) of every agreement in the WBCI history: the RUEM-VA and each ERLM to a
    /// postponement.
    fn agreed_dates(&self) -> Vec<(Timestamp, Date)> {
        let mut tvs: Vec<(&wbci::AenderungsId, Date)> = Vec::new();
        let mut out = Vec::new();
        for e in self.wbci.history() {
            match &e.message {
                WMessage::Anfrage(Anfrage::Terminverschiebung(t)) => {
                    tvs.push((&t.aenderungs_id, t.neuer_kundenwunschtermin));
                }
                WMessage::Meldung(WMeldung::RuemVa(r)) => out.push((e.at, r.wechseltermin)),
                WMessage::Meldung(WMeldung::Erlm(m)) if m.kopf.geschaeftsfall == WFall::TvsVa => {
                    let requested = tvs.iter().find(|(id, _)| Some(*id) == m.aenderungs_id.as_ref()).map(|(_, d)| *d);
                    if let Some(d) = m.wechseltermin.or(requested) {
                        out.push((e.at, d));
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// The WBCI switch date in force at `at`.
    fn termin_at(&self, at: Timestamp) -> Option<Date> {
        self.agreed_dates().into_iter().filter(|(t, _)| *t <= at).map(|(_, d)| d).next_back()
    }

    fn nicht_abgestimmt(&self, p: &spri::process::Process, out: &mut Vec<Befund>) {
        let Some(placed) = p.history().first().map(|e| e.at) else { return };
        let agreed_at = match self.wbci.state() {
            WState::Requested | WState::Rejected { .. } => None,
            _ => self.agreed_dates().first().map(|(t, _)| *t),
        };
        if agreed_at.is_some_and(|a| a <= placed) {
            return;
        }
        let (regel, quelle) = regeln::NICHT_ABGESTIMMT;
        out.push(Befund {
            regel,
            quelle,
            partei: partei_spri(SRole::Auftraggeber, p.geschaeftsfall()),
            text: format!(
                "order {} ({}) was placed before the WBCI pre-agreement {} was agreed (WBCI: {})",
                p.externe_auftragsnummer(),
                p.geschaeftsfall(),
                self.vorabstimmungs_id(),
                self.wbci.state()
            ),
        });
    }

    fn leitung(&self, p: &spri::process::Process, out: &mut Vec<Befund>) {
        if p.geschaeftsfall() != GeschaeftsfallCode::Pv
            || !matches!(self.wbci.takeover(), Some(Takeover::Announced { .. }))
        {
            return;
        }
        let Some(line) = order_line(p) else { return };
        let Some(akm) = self.wbci.history().iter().rev().find_map(|e| match &e.message {
            WMessage::Meldung(WMeldung::AkmTr(t)) => Some(t),
            _ => None,
        }) else {
            return;
        };
        let lines: Vec<&LineId> = akm
            .uebernahme_leitung
            .iter()
            .filter_map(|l| match l {
                UebernahmeLeitung::LineId(l) => Some(l),
                UebernahmeLeitung::Vertragsnummer(_) => None,
            })
            .collect();
        if lines.is_empty() || lines.contains(&line) {
            return;
        }
        let (regel, quelle) = regeln::LEITUNG;
        out.push(Befund {
            regel,
            quelle,
            partei: Partei::EkpAuf,
            text: format!(
                "order {} takes over line {line}, which the AKM-TR does not name ({})",
                p.externe_auftragsnummer(),
                lines.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
            ),
        });
    }

    fn termin(&self, p: &spri::process::Process, out: &mut Vec<Befund>) {
        let Some(x) = self.wechseltermin() else { return };
        let tv_sent = p.history().iter().any(|e| is_tv(&e.message));
        let tv_open = p.pending().iter().any(|q| matches!(q, Pending::Terminverschiebung { .. }));
        // After a Terminverschiebung the requested date is the rule of the TV; while one is
        // open the ABM is still the old one.
        let wish = if tv_sent { None } else { order_wish(p).filter(|w| *w != x) };
        let abm = if tv_open { None } else { p.liefertermin().filter(|l| *l != x) };
        let partei = match (wish, abm) {
            (None, None) => return,
            (Some(_), _) => partei_spri(SRole::Auftraggeber, p.geschaeftsfall()),
            (None, Some(_)) => Partei::Netzbetreiber,
        };
        let mut parts = Vec::new();
        if let Some(w) = wish {
            parts.push(format!("requested {}", Day(w)));
        }
        if let Some(l) = abm {
            parts.push(format!("confirmed {}", Day(l)));
        }
        let (regel, quelle) = regeln::TERMIN;
        out.push(Befund {
            regel,
            quelle,
            partei,
            text: format!(
                "order {}: {} — the WBCI switch date is {}",
                p.externe_auftragsnummer(),
                parts.join(", "),
                Day(x)
            ),
        });
    }

    fn tv_zuerst_wbci(&self, p: &spri::process::Process, out: &mut Vec<Befund>) {
        for e in p.history() {
            let SMessage::Auftrag(a) = &e.message else { continue };
            if a.aenderungskennzeichen != Aenderungskennzeichen::Terminverschiebung {
                continue;
            }
            let Some(neu) = a.termin_neu.or_else(|| wunschtermin(&a.geschaeftsfall)) else { continue };
            let in_force = self.termin_at(e.at);
            if in_force == Some(neu) {
                continue;
            }
            let (regel, quelle) = regeln::TV_ZUERST_WBCI;
            let wbci = in_force.map_or_else(|| "none".to_owned(), |d| Day(d).to_string());
            out.push(Befund {
                regel,
                quelle,
                partei: Partei::EkpAuf,
                text: format!(
                    "order {}: Terminverschiebung to {} while the WBCI switch date is {wbci}; postpone in WBCI first (TVS-VA, ERLM 8003)",
                    p.externe_auftragsnummer(),
                    Day(neu)
                ),
            });
        }
    }

    fn storno(&self, p: &spri::process::Process, out: &mut Vec<Befund>) {
        let what = match self.wbci.state() {
            WState::Cancelled => "cancelled",
            WState::Changed { .. } => "changed",
            _ => return,
        };
        if self.agreed_dates().is_empty() {
            return; // never agreed: the order is wechsel.nicht-abgestimmt
        }
        let open = matches!(
            p.state(),
            SState::Sent
                | SState::Received
                | SState::Accepted
                | SState::Confirmed { .. }
                | SState::TamOpen { .. }
                | SState::MtamOpen { .. }
                | SState::ErlmKCheck
        );
        if !open || p.pending().iter().any(|q| matches!(q, Pending::Storno { .. })) {
            return;
        }
        let (regel, quelle) = regeln::STORNO;
        out.push(Befund {
            regel,
            quelle,
            partei: partei_spri(SRole::Auftraggeber, p.geschaeftsfall()),
            text: format!(
                "the WBCI pre-agreement {} is {what} but order {} ({}) is still open: cancel it (Storno)",
                self.vorabstimmungs_id(),
                p.externe_auftragsnummer(),
                p.geschaeftsfall()
            ),
        });
    }

    fn kontinuitaet(&self, p: &spri::process::Process, out: &mut Vec<Befund>) {
        if !matches!(p.geschaeftsfall(), GeschaeftsfallCode::Pv | GeschaeftsfallCode::Neu) {
            return;
        }
        let Some(x) = self.wechseltermin() else { return };
        let Some(done) = completion(p) else { return };
        let latest = Calendar::wbci().add_working_days(x, 1);
        if done <= latest {
            return;
        }
        let (regel, quelle) = regeln::KONTINUITAET;
        out.push(Befund {
            regel,
            quelle,
            partei: Partei::Netzbetreiber,
            text: format!(
                "order {} completed on {}, later than one working day after the WBCI switch date {} (latest {})",
                p.externe_auftragsnummer(),
                Day(done),
                Day(x),
                Day(latest)
            ),
        });
    }
}

const fn is_tv(m: &SMessage) -> bool {
    matches!(m, SMessage::Auftrag(a) if matches!(a.aenderungskennzeichen, Aenderungskennzeichen::Terminverschiebung))
}

const fn wunschtermin(g: &SFall) -> Option<Date> {
    match g {
        SFall::Neu(x) => Some(x.wunschtermin.datum),
        SFall::KueAg(x) => Some(x.wunschtermin.datum),
        SFall::Lae(x) => Some(x.wunschtermin.datum),
        SFall::AenLmae(x) => Some(x.wunschtermin.datum),
        SFall::Est(x) => Some(x.wunschtermin.datum),
        SFall::Pv(x) => Some(x.wunschtermin.datum),
        SFall::Get(_) | SFall::Set(_) => None,
    }
}

/// The order's own requested date — the AKM-PV's planned date in the losing provider's view.
fn order_wish(p: &spri::process::Process) -> Option<Date> {
    p.history().first().and_then(|e| match &e.message {
        SMessage::Auftrag(a) => wunschtermin(&a.geschaeftsfall),
        SMessage::Meldung(m) => match &m.inhalt {
            Meldungsinhalt::AkmPv(a) => Some(a.aufnehmender_provider.uebernahmedatum_geplant),
            _ => None,
        },
        SMessage::Teq(_) => None,
    })
}

/// The line the order takes over — the AKM-PV's in the losing provider's view.
fn order_line(p: &spri::process::Process) -> Option<&LineId> {
    p.history().first().and_then(|e| match &e.message {
        SMessage::Auftrag(a) => a.geschaeftsfall.line_id(),
        SMessage::Meldung(m) => match &m.inhalt {
            Meldungsinhalt::AkmPv(a) => Some(&a.line_id),
            _ => None,
        },
        SMessage::Teq(_) => None,
    })
}

/// The completion date: of the last ERLM that is not a Storno's, or of the ERLM-PV.
fn completion(p: &spri::process::Process) -> Option<Date> {
    p.history().iter().rev().find_map(|e| match &e.message {
        SMessage::Meldung(m) => match &m.inhalt {
            Meldungsinhalt::Erlm(x)
                if m.auftragstyp.aenderungskennzeichen != Aenderungskennzeichen::Storno
                    && !m.codes().iter().any(|c| c.as_str() == "0012") =>
            {
                Some(x.erledigungstermin)
            }
            Meldungsinhalt::ErlmPv(x) => Some(x.erledigungstermin),
            _ => None,
        },
        _ => None,
    })
}

fn wbci_label(m: &WMessage) -> &'static str {
    match m {
        WMessage::Anfrage(a) => a.geschaeftsfall().as_str(),
        WMessage::Meldung(m) => m.meldungstyp().as_str(),
        WMessage::Teq(_) => "TEQ",
    }
}

fn wbci_codes(m: &WMessage) -> Vec<String> {
    match m {
        WMessage::Anfrage(_) => Vec::new(),
        WMessage::Meldung(m) => m.codes().map(|c| c.as_str().to_owned()).collect(),
        WMessage::Teq(t) => t.positionen.iter().map(|p| p.code.as_str().to_owned()).collect(),
    }
}

const fn spri_label(m: &SMessage) -> &'static str {
    match m {
        SMessage::Auftrag(a) => match a.aenderungskennzeichen {
            Aenderungskennzeichen::Standard => a.geschaeftsfall.code().as_str(),
            Aenderungskennzeichen::Storno => "Storno",
            Aenderungskennzeichen::Terminverschiebung => "Terminverschiebung",
        },
        SMessage::Meldung(m) => m.typ().as_str(),
        SMessage::Teq(_) => "TEQ",
    }
}

fn spri_codes(m: &SMessage) -> Vec<String> {
    match m {
        SMessage::Auftrag(_) => Vec::new(),
        SMessage::Meldung(m) => m.codes().iter().map(|c| c.as_str().to_owned()).collect(),
        SMessage::Teq(t) => t.positionen.iter().map(|p| p.code.as_str().to_owned()).collect(),
    }
}
