//! A sandbox: named scenarios that play a whole switch or order from both sides.
//!
//! Each [`Szenario`] plays the side you test — the gaining provider (WBCI EKPauf, S/PRI
//! Auftraggeber), scripted: it sends its requests and orders at set moments and acknowledges
//! every message it receives two seconds later — against the simulated peers of the other side:
//! [`wbci::sim::Ekpabg`], [`spri::sim::Leistungserbringer`] and [`spri::sim::AbgebenderProvider`].
//! The result is an [`Ablauf`]: every request, order, message and receipt of both sides, each a
//! [`Nachricht`] with its time, interface, sender and receiver, ready to compare with your own
//! system or to write as files (`telco-interop simulate`).
//!
//! The peers are driven by the process state machines, so every message is one the protocol
//! allows, inside its deadline. A scenario reads no clock: it starts at the instant you give
//! ([`START`] by default) and derives every date from it with the calendars, so the same start
//! gives the same bytes. The data is synthetic.
//!
//! ```
//! use telco_interop::sim;
//!
//! let szenario = sim::szenario("spri-neu").ok_or("no such scenario")?;
//! let ablauf = szenario.play(sim::START);
//! assert!(ablauf.abgelehnt.is_empty());
//! let kinds: Vec<&str> = ablauf.nachrichten.iter().filter(|n| n.art() != "TEQ").map(|n| n.art()).collect();
//! assert_eq!(kinds, ["NEU", "QEB", "ABM", "ERLM", "ENTM"]);
//! println!("{}", ablauf.nachrichten[0]); // Mo 01.03.2027 09:00 S/PRI EKPauf → Netzbetreiber NEU
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## The catalogue
//!
//! | Name | What it shows | Source |
//! |---|---|---|
//! | `wbci-agreed` | VA-KUE-MRN checked against the losing provider's record: TEQ, RUEM-VA 8001, AKM-TR | Anl. 1 V5.4.1 §2.2–§2.6; WBCI 2.0 §12.3 |
//! | `wbci-refused-rng` | VA-KUE-MRN, the record has another number: ABBM 8103 (RNG) | Anl. 1 V5.4.1 §2.3, §2.4 |
//! | `wbci-storno` | VA-KUE-ORN agreed, cancelled by the gaining provider (STR-AUF): ERLM 8004 | Anl. 1 V5.4.1 §2.9.1 |
//! | `wbci-tvs` | VA-KUE-ORN agreed, postponed by the gaining provider (TVS-VA): ERLM 8003 | Anl. 1 V5.4.1 §2.11 |
//! | `spri-neu` | NEU: TEQ, QEB, ABM, ERLM on the binding date, ENTM | Anl. 7 EPK V.13 p.1, p.10 |
//! | `spri-neu-tam` | NEU, the appointment fails: TAM, MTAM, ABBM 1198 | Anl. 7 EPK V.13 p.1; AH 2.9.5 §3.6 |
//! | `spri-storno` | NEU, Storno after the ABM: ERLM 0012, ENTM | Anl. 7 EPK V.13 p.11; AH 2.9.5 §3.5 |
//! | `spri-storno-refused` | NEU, Storno refused with ABBM 1087; the order completes | Anl. 7 EPK V.13 p.11 |
//! | `spri-tv` | NEU, Terminverschiebung after the ABM: a new ABM, ERLM on the new date, ENTM | Anl. 7 EPK V.13 p.12; AH 2.9.5 §3.4 |
//! | `spri-pv` | PV: AKM-PV, RUEM-PV, ABM with ABM-PV, ERLM with ERLM-PV, ENTM with ENTM-PV | Anl. 7 EPK V.13 p.7; S/PRI 4.4 §5.5.1, §8.2.13 |
//! | `spri-pv-refused` | PV, the losing provider refuses (KVE): ABBM 0041, ABBM-PV 0040 | Anl. 7 EPK V.13 p.7; S/PRI 4.4 §5.5.1.1, §5.5.1.2 |
//! | `spri-pv-timeout` | PV, no RUEM-PV by the antwortfrist: ABBM 0042, ABBM-PV 0040 | S/PRI 4.4 §5.5.1.1; Anl. 7 EPK V.13 p.7 |
//! | `wechsel` | the switch across both: VA, RUEM-VA with the LineID, AKM-TR, PV for the switch date, the PV leg, completion on the switch date | S/PRI 4.4 §5.5.1.1 (p. 27); Anl. 1 V5.4.1 §2.6.2; § 59 TKG |
//!
//! The S/PRI NEU scenarios carry a Vorabstimmungs-ID: they are the new line of a switching
//! customer (S/PRI 4.4 §5.5: a switch with NEU, KUE-AG or PV carries it), so a replay finds them
//! as one switch. [`Nachricht`]'s `Display` carries times, kinds and codes only.

use core::fmt;
use std::error::Error;

use carrier_core::calendar::berlin;
use carrier_core::count::{Day, day_of};
use carrier_core::xml::WriteError;
use wbci::Timestamp;
use wbci::sim::SignedDuration;

use crate::{Partei, Schnittstelle};

use spri::process::Role as SRole;
use spri::sim::{AbgebenderProvider, Leistungserbringer, Rueckmeldung};
use wbci::sim::{Antwort, Ekpabg};

/// The default start of every scenario: Monday 01.03.2027, 09:00 German time.
pub const START: Timestamp = Timestamp::constant(1_803_888_000, 0);

/// The most steps a scenario takes; a bound, never reached.
const MAX_STEPS: usize = 10_000;

/// The scripted side acknowledges a message after this long.
const ACK: SignedDuration = SignedDuration::from_secs(2);

/// The scripted side acts this long after the message it waited for.
const PAUSE: SignedDuration = SignedDuration::from_hours(1);

/// A named scenario of the catalogue.
#[derive(Clone, Copy)]
pub struct Szenario {
    /// Its name, e.g. `spri-pv`.
    pub name: &'static str,
    /// What it shows, in one line.
    pub beschreibung: &'static str,
    /// The chart page or clause it follows.
    pub quelle: &'static str,
    play: fn(Timestamp) -> Result<Lauf, Box<dyn Error>>,
}

impl fmt::Debug for Szenario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Szenario").field("name", &self.name).finish_non_exhaustive()
    }
}

impl Szenario {
    /// Plays the scenario from `start`.
    #[must_use]
    pub fn play(&self, start: Timestamp) -> Ablauf {
        match (self.play)(start) {
            Ok(lauf) => lauf.ablauf(),
            Err(e) => Ablauf { nachrichten: Vec::new(), abgelehnt: vec![format!("{}: {e}", self.name)] },
        }
    }
}

/// The catalogue, in the order of the table above.
#[must_use]
pub fn katalog() -> &'static [Szenario] {
    KATALOG
}

/// The scenario called `name`.
#[must_use]
pub fn szenario(name: &str) -> Option<&'static Szenario> {
    KATALOG.iter().find(|s| s.name == name)
}

/// The payload of a [`Nachricht`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inhalt {
    /// A WBCI payload.
    Wbci(Box<wbci::Payload>),
    /// An S/PRI payload.
    Spri(Box<spri::Payload>),
}

/// One message of a scenario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nachricht {
    /// When it was sent (also its control timestamp).
    pub at: Timestamp,
    /// The interface.
    pub schnittstelle: Schnittstelle,
    /// Who sent it.
    pub von: Partei,
    /// Who received it.
    pub an: Partei,
    /// The payload.
    pub inhalt: Inhalt,
}

impl Nachricht {
    /// What it is: `VA-KUE-MRN`, `RUEM-VA`, `PV`, `Storno`, `ABM-PV`, `TEQ`, ….
    #[must_use]
    pub fn art(&self) -> &'static str {
        match &self.inhalt {
            Inhalt::Wbci(p) => p.kind(),
            Inhalt::Spri(p) => p.kind(),
        }
    }

    /// The codes of its positions, e.g. `8001`.
    #[must_use]
    pub fn codes(&self) -> Vec<String> {
        match &self.inhalt {
            Inhalt::Wbci(p) => match &**p {
                wbci::Payload::MeldungRequest { meldung, .. } => {
                    meldung.codes().map(|c| c.as_str().to_owned()).collect()
                }
                wbci::Payload::AnfrageResponse { quittung, .. } | wbci::Payload::MeldungResponse { quittung, .. } => {
                    quittung.positionen.iter().map(|p| p.code.as_str().to_owned()).collect()
                }
                wbci::Payload::AnfrageRequest { .. } => Vec::new(),
            },
            Inhalt::Spri(p) => match &**p {
                spri::Payload::MeldungRequest { meldung, .. } => {
                    meldung.codes().iter().map(|c| c.as_str().to_owned()).collect()
                }
                spri::Payload::AuftragResponse { quittung, .. } | spri::Payload::MeldungResponse { quittung, .. } => {
                    quittung.positionen.iter().map(|p| p.code.as_str().to_owned()).collect()
                }
                spri::Payload::AuftragRequest { .. } => Vec::new(),
            },
        }
    }

    /// The payload as XML.
    ///
    /// # Errors
    ///
    /// The payload exceeds a schema maximum (never for a scenario's payload).
    pub fn to_xml(&self) -> Result<String, WriteError> {
        match &self.inhalt {
            Inhalt::Wbci(p) => p.to_xml(),
            Inhalt::Spri(p) => p.to_xml(),
        }
    }

    /// Its file name at `position` (1-based, in time order):
    /// `NNN-<interface>-<sender>-<kind>.xml`, e.g. `001-wbci-ekpauf-va-kue-mrn.xml`.
    #[must_use]
    pub fn file_name(&self, position: usize) -> String {
        let schnittstelle = match self.schnittstelle {
            Schnittstelle::Wbci => "wbci",
            Schnittstelle::Spri => "spri",
        };
        let art: String =
            self.art().chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
        format!("{position:03}-{schnittstelle}-{}-{art}.xml", self.von.to_string().to_ascii_lowercase())
    }
}

impl fmt::Display for Nachricht {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let z = self.at.to_zoned_datetime(berlin());
        write!(
            f,
            "{} {:02}:{:02} {} {} → {} {}",
            Day(z.date()),
            z.hour(),
            z.minute(),
            self.schnittstelle,
            self.von,
            self.an,
            self.art()
        )?;
        for c in self.codes() {
            write!(f, " {c}")?;
        }
        Ok(())
    }
}

/// A played scenario.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ablauf {
    /// Every message of both sides, in time order.
    pub nachrichten: Vec<Nachricht>,
    /// What a peer refused while playing, value-free; empty for every scenario of the catalogue.
    pub abgelehnt: Vec<String>,
}

impl Ablauf {
    /// Every message as `(file name, XML)`, in time order.
    ///
    /// # Errors
    ///
    /// A payload exceeds a schema maximum (never for a scenario's payload).
    pub fn files(&self) -> Result<Vec<(String, String)>, WriteError> {
        self.nachrichten.iter().enumerate().map(|(i, n)| Ok((n.file_name(i.saturating_add(1)), n.to_xml()?))).collect()
    }
}

// -- the runner -------------------------------------------------------------------------------

/// A message on its way.
struct Item {
    at: Timestamp,
    seq: u64,
    von: Partei,
    an: Partei,
    inhalt: Inhalt,
}

/// The peers, the scripted side's mailbox and what was sent.
struct Lauf {
    ekpabg: Option<Ekpabg>,
    le: Option<Leistungserbringer>,
    ab: Option<AbgebenderProvider>,
    signatur: wbci::SignaturId,
    ag: spri::Auftraggebernummer,
    queue: Vec<Item>,
    seq: u64,
    sent: Vec<Item>,
    abgelehnt: Vec<String>,
}

const fn partei(role: SRole) -> Partei {
    match role {
        SRole::Auftraggeber => Partei::EkpAuf,
        SRole::Leistungserbringer => Partei::Netzbetreiber,
        SRole::AbgebenderProvider => Partei::EkpAbg,
    }
}

impl Lauf {
    fn new(d: &Daten) -> Self {
        Self {
            ekpabg: None,
            le: None,
            ab: None,
            signatur: d.signatur.clone(),
            ag: d.ag.clone(),
            queue: Vec::new(),
            seq: 0,
            sent: Vec::new(),
            abgelehnt: Vec::new(),
        }
    }

    fn push(&mut self, at: Timestamp, von: Partei, an: Partei, inhalt: Inhalt) {
        self.seq = self.seq.saturating_add(1);
        self.queue.push(Item { at, seq: self.seq, von, an, inhalt });
    }

    /// The scripted side sends a WBCI request.
    fn wbci(&mut self, at: Timestamp, anfrage: impl Into<wbci::Anfrage>) {
        let control = self.wbci_control(at);
        let p = wbci::Payload::AnfrageRequest { control, anfrage: anfrage.into() };
        self.push(at, Partei::EkpAuf, Partei::EkpAbg, Inhalt::Wbci(Box::new(p)));
    }

    /// The scripted side sends a WBCI message.
    fn wbci_meldung(&mut self, at: Timestamp, meldung: impl Into<wbci::Meldung>) {
        let control = self.wbci_control(at);
        let p = wbci::Payload::MeldungRequest { control, meldung: meldung.into() };
        self.push(at, Partei::EkpAuf, Partei::EkpAbg, Inhalt::Wbci(Box::new(p)));
    }

    /// The scripted side sends an S/PRI order.
    fn spri(&mut self, at: Timestamp, auftrag: spri::Auftrag) {
        let control = self.spri_control(at);
        let p = spri::Payload::AuftragRequest { control, auftrag };
        self.push(at, Partei::EkpAuf, Partei::Netzbetreiber, Inhalt::Spri(Box::new(p)));
    }

    fn wbci_control(&self, at: Timestamp) -> wbci::Control {
        wbci::Control::new(at, self.signatur.clone())
    }

    fn spri_control(&self, at: Timestamp) -> spri::Control {
        spri::Control::new(at, self.signatur.clone())
    }

    fn reject(&mut self, what: &str, why: impl fmt::Display) {
        self.abgelehnt.push(format!("{what}: {why}"));
    }

    /// The scripted side receives `item`: it acknowledges every business message.
    fn scripted(&mut self, item: &Item) {
        let at = item.at.saturating_add(ACK).unwrap_or(item.at);
        match &item.inhalt {
            Inhalt::Wbci(p) => {
                if let wbci::Payload::MeldungRequest { meldung, .. } = &**p {
                    let quittung = wbci::Teq::for_meldung(meldung, wbci::Meldungscode::Ok);
                    let control = self.wbci_control(at);
                    let teq = wbci::Payload::MeldungResponse { control, quittung };
                    self.push(at, Partei::EkpAuf, item.von, Inhalt::Wbci(Box::new(teq)));
                }
            }
            Inhalt::Spri(p) => {
                if let spri::Payload::MeldungRequest { meldung, .. } = &**p {
                    let quittung = spri::Teq::new(
                        spri::Meldungscode::Ok,
                        Some(meldung.externe_auftragsnummer().clone()),
                        Some(self.ag.clone()),
                    );
                    let control = self.spri_control(at);
                    let teq = spri::Payload::MeldungResponse { control, quittung };
                    self.push(at, Partei::EkpAuf, item.von, Inhalt::Spri(Box::new(teq)));
                }
            }
        }
    }

    /// Delivers `item` to its receiver.
    fn deliver(&mut self, item: &Item) {
        let kind = match &item.inhalt {
            Inhalt::Wbci(p) => p.kind(),
            Inhalt::Spri(p) => p.kind(),
        };
        match (&item.inhalt, item.an) {
            (_, Partei::EkpAuf) => self.scripted(item),
            (Inhalt::Wbci(p), Partei::EkpAbg) => {
                let Some(peer) = self.ekpabg.as_mut() else { return self.reject(kind, "no WBCI losing provider") };
                let r = peer.receive(p, item.at);
                if let Some(rej) = r.rejection {
                    self.reject(kind, rej);
                }
                for o in r.outbound {
                    self.push(o.at, Partei::EkpAbg, Partei::EkpAuf, Inhalt::Wbci(Box::new(o.payload)));
                }
            }
            (Inhalt::Spri(p), Partei::EkpAbg) => {
                let Some(peer) = self.ab.as_mut() else { return self.reject(kind, "no S/PRI losing provider") };
                let r = peer.receive(p, item.at);
                if let Some(rej) = r.rejection {
                    self.reject(kind, rej);
                }
                for o in r.outbound {
                    self.push(o.at, Partei::EkpAbg, partei(o.to), Inhalt::Spri(Box::new(o.payload)));
                }
            }
            (Inhalt::Spri(p), Partei::Netzbetreiber) => {
                let Some(peer) = self.le.as_mut() else { return self.reject(kind, "no network owner") };
                let r = peer.receive(p, item.at);
                if let Some(rej) = r.rejection {
                    self.reject(kind, rej);
                }
                for o in r.outbound {
                    self.push(o.at, Partei::Netzbetreiber, partei(o.to), Inhalt::Spri(Box::new(o.payload)));
                }
            }
            (Inhalt::Wbci(_), Partei::Netzbetreiber) => self.reject(kind, "the network owner is no WBCI party"),
        }
    }

    /// One step: the earliest delivery or peer act by `until`. `false` when nothing is left.
    fn step(&mut self, until: Timestamp) -> bool {
        let queued = self.queue.iter().map(|i| (i.at, i.seq)).min();
        let w = self.ekpabg.as_ref().and_then(Ekpabg::next_due);
        let le = self.le.as_ref().and_then(Leistungserbringer::next_due);
        let ab = self.ab.as_ref().and_then(AbgebenderProvider::next_due);
        let Some(t) = [queued.map(|q| q.0), w, le, ab].into_iter().flatten().min().filter(|t| *t <= until) else {
            return false;
        };
        if let Some((at, seq)) = queued
            && at == t
        {
            let Some(pos) = self.queue.iter().position(|i| i.seq == seq) else { return false };
            let item = self.queue.remove(pos);
            self.deliver(&item);
            self.sent.push(item);
        } else if w == Some(t) {
            for o in self.ekpabg.as_mut().map(|p| p.advance(t)).unwrap_or_default() {
                self.push(o.at, Partei::EkpAbg, Partei::EkpAuf, Inhalt::Wbci(Box::new(o.payload)));
            }
        } else if le == Some(t) {
            for o in self.le.as_mut().map(|p| p.advance(t)).unwrap_or_default() {
                self.push(o.at, Partei::Netzbetreiber, partei(o.to), Inhalt::Spri(Box::new(o.payload)));
            }
        } else {
            for o in self.ab.as_mut().map(|p| p.advance(t)).unwrap_or_default() {
                self.push(o.at, Partei::EkpAbg, partei(o.to), Inhalt::Spri(Box::new(o.payload)));
            }
        }
        true
    }

    /// Plays everything due by `until`.
    fn run(&mut self, until: Timestamp) {
        for _ in 0..MAX_STEPS {
            if !self.step(until) {
                return;
            }
        }
        self.abgelehnt.push("the scenario does not settle".to_owned());
    }

    /// Plays until a peer has sent a message of one of `kinds`, then its receipt; returns when
    /// the scripted side acts next (one [`PAUSE`] after that message).
    fn after(&mut self, kinds: &[&str]) -> Result<Timestamp, Box<dyn Error>> {
        let mark = self.sent.len();
        let horizon = Timestamp::MAX;
        for _ in 0..MAX_STEPS {
            let found = self.sent.get(mark..).and_then(|s| {
                s.iter().find(|i| i.von != Partei::EkpAuf && kinds.contains(&kind_of(&i.inhalt))).map(|i| i.at)
            });
            if let Some(at) = found {
                self.run(at.saturating_add(SignedDuration::from_mins(1)).unwrap_or(at));
                return Ok(at.saturating_add(PAUSE).unwrap_or(at));
            }
            if !self.step(horizon) {
                break;
            }
        }
        Err(format!("no {} was sent", kinds.join(" or ")).into())
    }

    /// The last WBCI message of type `typ` the losing provider sent.
    fn wbci_last(&self, typ: wbci::Meldungstyp) -> Option<wbci::Meldung> {
        self.sent.iter().rev().find_map(|i| match &i.inhalt {
            Inhalt::Wbci(p) => match &**p {
                wbci::Payload::MeldungRequest { meldung, .. } if meldung.meldungstyp() == typ => Some(meldung.clone()),
                _ => None,
            },
            Inhalt::Spri(_) => None,
        })
    }

    fn ablauf(mut self) -> Ablauf {
        self.run(Timestamp::MAX);
        for (name, stopped) in [
            ("WBCI losing provider", self.ekpabg.as_ref().and_then(|p| p.stopped().map(ToString::to_string))),
            ("network owner", self.le.as_ref().and_then(|p| p.stopped().map(ToString::to_string))),
            ("S/PRI losing provider", self.ab.as_ref().and_then(|p| p.stopped().map(ToString::to_string))),
        ] {
            if let Some(s) = stopped {
                self.abgelehnt.push(format!("{name} stopped: {s}"));
            }
        }
        let mut sent = self.sent;
        sent.sort_by_key(|i| (i.at, i.seq));
        let nachrichten = sent
            .into_iter()
            .map(|i| Nachricht {
                at: i.at,
                schnittstelle: match i.inhalt {
                    Inhalt::Wbci(_) => Schnittstelle::Wbci,
                    Inhalt::Spri(_) => Schnittstelle::Spri,
                },
                von: i.von,
                an: i.an,
                inhalt: i.inhalt,
            })
            .collect();
        Ablauf { nachrichten, abgelehnt: self.abgelehnt }
    }
}

fn kind_of(i: &Inhalt) -> &'static str {
    match i {
        Inhalt::Wbci(p) => p.kind(),
        Inhalt::Spri(p) => p.kind(),
    }
}

// -- synthetic data ---------------------------------------------------------------------------

/// The scenarios' synthetic data. Every name, address and number is invented.
struct Daten {
    ekp: wbci::Ekp,
    signatur: wbci::SignaturId,
    ag: spri::Auftraggebernummer,
}

impl Daten {
    fn new() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            ekp: wbci::Ekp { auf: "DEU.EKP1".parse()?, abg: "DEU.EKP2".parse()? },
            signatur: wbci::SignaturId {
                issuer: carrier_core::SignaturText::new("CN=Sandbox CA, O=telco-interop")?,
                serial: carrier_core::SignaturText::new("1")?,
            },
            ag: spri::Auftraggebernummer::new("4711")?,
        })
    }

    fn va_id(&self) -> Result<wbci::VorabstimmungsId, Box<dyn Error>> {
        Ok(wbci::VorabstimmungsId::allocate(&self.ekp.auf, 1)?)
    }

    fn standort() -> Result<wbci::Standort, Box<dyn Error>> {
        Ok(wbci::Standort {
            strassenname: wbci::Strassenname::new("Musterweg")?,
            hausnummer: wbci::Hausnummer::new("1")?,
            hausnummern_zusatz: None,
            postleitzahl: wbci::Postleitzahl::new("12345")?,
            ort: wbci::Ortsname::new("Musterstadt")?,
        })
    }

    fn kunde() -> Result<wbci::Kunde, Box<dyn Error>> {
        Ok(wbci::Kunde::person(
            wbci::Anrede::Frau,
            Some(wbci::Vorname::new("Erika")?),
            wbci::Nachname::new("Musterfrau")?,
        ))
    }

    fn nummer(nr: &str) -> Result<wbci::OnkzRufnummer, Box<dyn Error>> {
        Ok(wbci::OnkzRufnummer { onkz: wbci::Onkz::new("30")?, rufnummer: wbci::Rufnummer::new(nr)? })
    }

    /// A pre-agreement sent at `start`, for 5 working days after the earliest permitted date.
    fn va(&self, start: Timestamp, mrn: bool) -> Result<wbci::Vorabstimmung, Box<dyn Error>> {
        let cal = wbci::Calendar::wbci();
        let fall = if mrn { wbci::Geschaeftsfall::VaKueMrn } else { wbci::Geschaeftsfall::VaKueOrn };
        let earliest = wbci::deadline::earliest_kundenwunschtermin(fall, start, &cal).ok_or("no lead time")?;
        let fall = if mrn {
            wbci::VorabstimmungsFall::KueMrn {
                standort: Self::standort()?,
                rufnummernportierung: wbci::Rufnummernportierung {
                    anschluss: wbci::Anschluss::Einzel {
                        rufnummern: vec![Self::nummer("5550100")?],
                        alle_rufnummern: false,
                    },
                    zeitfenster: wbci::Portierungszeitfenster::Zf2,
                },
            }
        } else {
            wbci::VorabstimmungsFall::KueOrn { standort: Self::standort()?, anschlussidentifikation: None }
        };
        Ok(wbci::Vorabstimmung {
            ekp: self.ekp.clone(),
            id: self.va_id()?,
            kundenwunschtermin: cal.add_working_days(earliest.date, 5),
            endkunde: Self::kunde()?,
            weitere_anschlussinhaber: Vec::new(),
            projekt: None,
            fall,
        })
    }

    /// The losing provider's record of the line: matching `va`, with `nummer` and the line.
    fn bestand(nummer: &str, line: Option<&spri::LineId>) -> Result<wbci::pruefung::Bestand, Box<dyn Error>> {
        let mut b = wbci::pruefung::Bestand::new(Self::kunde()?, wbci::Portierungskennung::new("D234")?);
        b.standort = Some(Self::standort()?);
        b.rufnummern = vec![Self::nummer(nummer)?];
        if let Some(line) = line {
            b.ressourcen = vec![wbci::TechnischeRessource { line_id: Some(line.clone()), ..Default::default() }];
        }
        Ok(b)
    }

    fn ansprechpartner() -> Result<spri::AnsprechpartnerAuftrag, Box<dyn Error>> {
        Ok(spri::AnsprechpartnerAuftrag {
            auftragsmanagement: spri::Ansprechpartner {
                person: spri::Person {
                    anrede: spri::Anrede::Herr,
                    vorname: None,
                    nachname: spri::Nachname::new("Auftragsmanagement")?,
                },
                telefonnummer: spri::Telefonnummer::new("030 5550200")?,
                mobilfunknummer: None,
                faxnummer: None,
                emailadresse: None,
            },
            weitere: Vec::new(),
        })
    }

    fn standort_a() -> Result<spri::StandortA, Box<dyn Error>> {
        Ok(spri::StandortA {
            inhaber: spri::Inhaber::Person(spri::Person {
                anrede: spri::Anrede::Frau,
                vorname: Some(spri::Vorname::new("Erika")?),
                nachname: spri::Nachname::new("Musterfrau")?,
            }),
            telefonnummer: None,
            mobilfunknummer: None,
            strasse: spri::Strasse {
                strassenname: spri::Strassenname::new("Musterweg")?,
                hausnummer: spri::Hausnummer::new("1")?,
                hausnummern_zusatz: None,
            },
            gebaeudeteil: None,
            land: None,
            postleitzahl: spri::Postleitzahl::new("12345")?,
            ort: spri::Ort { ortsname: spri::Ortsname::new("Musterstadt")?, ortsteil: None },
            lage_tae_ont: None,
        })
    }

    fn auftrag(
        &self,
        geschaeftsfall: spri::Geschaeftsfall,
        art: spri::GeschaeftsfallArt,
    ) -> Result<spri::Auftrag, Box<dyn Error>> {
        Ok(spri::Auftrag {
            externe_auftragsnummer: spri::ExterneAuftragsnummer::new("SB-2027-0001")?,
            auftraggeber: spri::Auftraggeber {
                auftraggebernummer: self.ag.clone(),
                leistungsnummer: spri::Leistungsnummer::new("RV-1")?,
            },
            besteller: None,
            geschaeftsfall,
            geschaeftsfall_art: art,
            aenderungskennzeichen: spri::Aenderungskennzeichen::Standard,
            termin_neu: None,
            kenner: None,
        })
    }

    /// A NEU for the line of a switching customer, 15 working days after `start`.
    fn neu(&self, start: Timestamp) -> Result<spri::Auftrag, Box<dyn Error>> {
        let datum = wbci::Calendar::wbci().add_working_days(day_of(start), 15);
        let neu = spri::Bereitstellung {
            vorabstimmungs_id: Some(self.va_id()?),
            anlage: None,
            ansprechpartner: Self::ansprechpartner()?,
            wunschtermin: datum.into(),
            produkt: spri::Produkt { bezeichner: spri::Produktbezeichner::new("FTTH 1000")? },
            fttx: spri::FttxBereitstellung {
                standort_a: Self::standort_a()?,
                standort_versand: None,
                vormieter: None,
                montageleistung: None,
                wita: spri::WitaExtendedInfos {
                    wita: spri::WitaInfos { wita_vertragsnummer: None, technologie: None },
                    bestandssuche: None,
                    rufnummern_portierung: None,
                    abgebender_ekp: None,
                },
            },
            positionen: Vec::new(),
        };
        self.auftrag(spri::Geschaeftsfall::Neu(neu), spri::GeschaeftsfallArt::Bereitstellung)
    }

    /// A PV of `line` for `datum`.
    fn pv(&self, datum: wbci::Date, line: spri::LineId) -> Result<spri::Auftrag, Box<dyn Error>> {
        let pv = spri::Providerwechsel {
            line_id: line,
            vorabstimmungs_id: self.va_id()?,
            anlage: None,
            ansprechpartner: Self::ansprechpartner()?,
            wunschtermin: datum.into(),
            produkt: spri::Produkt { bezeichner: spri::Produktbezeichner::new("FTTH 1000")? },
            fttx: spri::FttxProviderwechsel { standort_a: Self::standort_a()?, montageleistung: None },
            positionen: Vec::new(),
        };
        self.auftrag(spri::Geschaeftsfall::Pv(pv), spri::GeschaeftsfallArt::Endkundenanbieterwechsel)
    }

    fn line() -> Result<spri::LineId, Box<dyn Error>> {
        Ok("DEU.NB01.4200000042".parse()?)
    }
}

fn hours(n: i64) -> SignedDuration {
    SignedDuration::from_hours(n)
}

// -- the scenarios ----------------------------------------------------------------------------

/// A WBCI switch answered by the losing provider with `antwort`; returns the runner after the
/// answer and the pre-agreement.
fn wbci_answered(
    start: Timestamp,
    mrn: bool,
    antwort: impl FnOnce(&wbci::Vorabstimmung) -> Result<Antwort, Box<dyn Error>>,
) -> Result<(Lauf, wbci::Vorabstimmung, Timestamp), Box<dyn Error>> {
    let d = Daten::new()?;
    let va = d.va(start, mrn)?;
    let mut l = Lauf::new(&d);
    l.ekpabg = Some(Ekpabg::new(antwort(&va)?, d.signatur.clone()));
    l.wbci(start, va.clone());
    let next = l.after(&["RUEM-VA", "ABBM"])?;
    Ok((l, va, next))
}

fn wbci_agreed(start: Timestamp) -> Result<Lauf, Box<dyn Error>> {
    let (mut l, _, next) =
        wbci_answered(start, true, |_| Ok(Antwort::Pruefung(Box::new(Daten::bestand("5550100", None)?))))?;
    let Some(wbci::Meldung::RuemVa(ruem)) = l.wbci_last(wbci::Meldungstyp::RuemVa) else {
        return Err("no RUEM-VA".into());
    };
    l.wbci_meldung(next, wbci::AkmTr::uebernahme(&ruem, Vec::new(), false));
    Ok(l)
}

fn wbci_refused_rng(start: Timestamp) -> Result<Lauf, Box<dyn Error>> {
    let (l, _, _) = wbci_answered(start, true, |_| Ok(Antwort::Pruefung(Box::new(Daten::bestand("5550199", None)?))))?;
    Ok(l)
}

fn wbci_storno(start: Timestamp) -> Result<Lauf, Box<dyn Error>> {
    let (mut l, va, next) = wbci_answered(start, false, |_| Ok(Antwort::Zustimmung))?;
    let storno = wbci::Storno {
        ekp: va.ekp.clone(),
        storno_id: wbci::StornoId::allocate(&va.ekp.auf, 1)?,
        bezug: wbci::Bezug::Id(va.id.clone()),
        art: wbci::StornoArt::AufhebungAuf,
    };
    l.wbci(next.saturating_add(hours(23))?, storno);
    Ok(l)
}

fn wbci_tvs(start: Timestamp) -> Result<Lauf, Box<dyn Error>> {
    let (mut l, va, next) = wbci_answered(start, false, |_| Ok(Antwort::Zustimmung))?;
    let Some(wbci::Meldung::RuemVa(ruem)) = l.wbci_last(wbci::Meldungstyp::RuemVa) else {
        return Err("no RUEM-VA".into());
    };
    let tvs = wbci::Terminverschiebung {
        ekp: va.ekp.clone(),
        aenderungs_id: wbci::AenderungsId::allocate(&va.ekp.auf, 1)?,
        bezug: wbci::Bezug::Id(va.id.clone()),
        name: None,
        neuer_kundenwunschtermin: wbci::Calendar::wbci().add_working_days(ruem.wechseltermin, 5),
    };
    l.wbci(next.saturating_add(hours(23))?, tvs);
    Ok(l)
}

/// A NEU sent at `start` to `le`; returns the runner after the ABM.
fn neu_confirmed(start: Timestamp, le: Leistungserbringer) -> Result<(Lauf, spri::Auftrag, Timestamp), Box<dyn Error>> {
    let d = Daten::new()?;
    let neu = d.neu(start)?;
    let mut l = Lauf::new(&d);
    l.le = Some(le);
    l.spri(start, neu.clone());
    let next = l.after(&["ABM"])?;
    Ok((l, neu, next))
}

fn le(d: &Daten) -> Leistungserbringer {
    Leistungserbringer::new(d.signatur.clone())
}

fn spri_neu(start: Timestamp) -> Result<Lauf, Box<dyn Error>> {
    let (l, _, _) = neu_confirmed(start, le(&Daten::new()?))?;
    Ok(l)
}

fn spri_neu_tam(start: Timestamp) -> Result<Lauf, Box<dyn Error>> {
    let (l, _, _) = neu_confirmed(start, le(&Daten::new()?).with_terminanforderung())?;
    Ok(l)
}

fn storno(start: Timestamp, le: Leistungserbringer) -> Result<Lauf, Box<dyn Error>> {
    let (mut l, mut neu, next) = neu_confirmed(start, le)?;
    neu.aenderungskennzeichen = spri::Aenderungskennzeichen::Storno;
    l.spri(next.saturating_add(hours(23))?, neu);
    Ok(l)
}

fn spri_storno(start: Timestamp) -> Result<Lauf, Box<dyn Error>> {
    storno(start, le(&Daten::new()?))
}

fn spri_storno_refused(start: Timestamp) -> Result<Lauf, Box<dyn Error>> {
    storno(start, le(&Daten::new()?).with_storno_abgelehnt())
}

fn spri_tv(start: Timestamp) -> Result<Lauf, Box<dyn Error>> {
    let (mut l, mut neu, next) = neu_confirmed(start, le(&Daten::new()?))?;
    let spri::Geschaeftsfall::Neu(b) = &mut neu.geschaeftsfall else { return Err("not a NEU".into()) };
    b.wunschtermin.datum = wbci::Calendar::wbci().add_working_days(b.wunschtermin.datum, 5);
    neu.aenderungskennzeichen = spri::Aenderungskennzeichen::Terminverschiebung;
    l.spri(next.saturating_add(hours(23))?, neu);
    Ok(l)
}

fn pv(start: Timestamp, rueckmeldung: Rueckmeldung) -> Result<Lauf, Box<dyn Error>> {
    let d = Daten::new()?;
    let datum = wbci::Calendar::wbci().add_working_days(day_of(start), 15);
    let mut l = Lauf::new(&d);
    l.le = Some(le(&d));
    l.ab = Some(AbgebenderProvider::new(rueckmeldung, d.signatur.clone()));
    l.spri(start, d.pv(datum, Daten::line()?)?);
    Ok(l)
}

fn spri_pv(start: Timestamp) -> Result<Lauf, Box<dyn Error>> {
    pv(start, Rueckmeldung::Zustimmung)
}

fn spri_pv_refused(start: Timestamp) -> Result<Lauf, Box<dyn Error>> {
    pv(start, Rueckmeldung::Ablehnung(spri::Antwortcode::new("KVE")?))
}

fn spri_pv_timeout(start: Timestamp) -> Result<Lauf, Box<dyn Error>> {
    pv(start, Rueckmeldung::Keine)
}

fn wechsel(start: Timestamp) -> Result<Lauf, Box<dyn Error>> {
    let d = Daten::new()?;
    let line = Daten::line()?;
    let (mut l, _, next) =
        wbci_answered(start, true, |_| Ok(Antwort::Pruefung(Box::new(Daten::bestand("5550100", Some(&line))?))))?;
    let Some(wbci::Meldung::RuemVa(ruem)) = l.wbci_last(wbci::Meldungstyp::RuemVa) else {
        return Err("no RUEM-VA".into());
    };
    // The gaining provider takes over the line the losing provider named (Anl. 1 V5.4.1 §2.6.2) …
    let lines: Vec<wbci::UebernahmeLeitung> =
        ruem.ressourcen.iter().filter_map(|r| r.line_id.clone()).map(wbci::UebernahmeLeitung::LineId).collect();
    l.wbci_meldung(next, wbci::AkmTr::uebernahme(&ruem, lines, false));
    // … and orders it from the network owner for the agreed switch date (S/PRI 4.4 §5.5.1.1).
    l.le = Some(le(&d));
    l.ab = Some(AbgebenderProvider::new(Rueckmeldung::Zustimmung, d.signatur.clone()));
    l.spri(next.saturating_add(PAUSE)?, d.pv(ruem.wechseltermin, line)?);
    Ok(l)
}

const KATALOG: &[Szenario] = &[
    Szenario {
        name: "wbci-agreed",
        beschreibung: "VA-KUE-MRN checked against the losing provider's record: TEQ, RUEM-VA 8001, AKM-TR",
        quelle: "Anl. 1 V5.4.1 §2.2–§2.6; WBCI 2.0 §12.3",
        play: wbci_agreed,
    },
    Szenario {
        name: "wbci-refused-rng",
        beschreibung: "VA-KUE-MRN, the record has another number: ABBM 8103 (RNG)",
        quelle: "Anl. 1 V5.4.1 §2.3, §2.4",
        play: wbci_refused_rng,
    },
    Szenario {
        name: "wbci-storno",
        beschreibung: "VA-KUE-ORN agreed, cancelled by the gaining provider (STR-AUF): ERLM 8004",
        quelle: "Anl. 1 V5.4.1 §2.9.1",
        play: wbci_storno,
    },
    Szenario {
        name: "wbci-tvs",
        beschreibung: "VA-KUE-ORN agreed, postponed by the gaining provider (TVS-VA): ERLM 8003",
        quelle: "Anl. 1 V5.4.1 §2.11",
        play: wbci_tvs,
    },
    Szenario {
        name: "spri-neu",
        beschreibung: "NEU: TEQ, QEB, ABM, ERLM on the binding date, ENTM",
        quelle: "Anl. 7 EPK V.13 p.1, p.10",
        play: spri_neu,
    },
    Szenario {
        name: "spri-neu-tam",
        beschreibung: "NEU, the appointment fails: TAM, MTAM, ABBM 1198",
        quelle: "Anl. 7 EPK V.13 p.1; AH 2.9.5 §3.6",
        play: spri_neu_tam,
    },
    Szenario {
        name: "spri-storno",
        beschreibung: "NEU, Storno after the ABM: ERLM 0012, ENTM",
        quelle: "Anl. 7 EPK V.13 p.11; AH 2.9.5 §3.5",
        play: spri_storno,
    },
    Szenario {
        name: "spri-storno-refused",
        beschreibung: "NEU, Storno refused with ABBM 1087; the order completes",
        quelle: "Anl. 7 EPK V.13 p.11",
        play: spri_storno_refused,
    },
    Szenario {
        name: "spri-tv",
        beschreibung: "NEU, Terminverschiebung after the ABM: a new ABM, ERLM on the new date, ENTM",
        quelle: "Anl. 7 EPK V.13 p.12; AH 2.9.5 §3.4",
        play: spri_tv,
    },
    Szenario {
        name: "spri-pv",
        beschreibung: "PV: AKM-PV, RUEM-PV, ABM with ABM-PV, ERLM with ERLM-PV, ENTM with ENTM-PV",
        quelle: "Anl. 7 EPK V.13 p.7; S/PRI 4.4 §5.5.1, §8.2.13",
        play: spri_pv,
    },
    Szenario {
        name: "spri-pv-refused",
        beschreibung: "PV, the losing provider refuses (KVE): ABBM 0041, ABBM-PV 0040",
        quelle: "Anl. 7 EPK V.13 p.7; S/PRI 4.4 §5.5.1.1, §5.5.1.2",
        play: spri_pv_refused,
    },
    Szenario {
        name: "spri-pv-timeout",
        beschreibung: "PV, no RUEM-PV by the antwortfrist: ABBM 0042, ABBM-PV 0040",
        quelle: "S/PRI 4.4 §5.5.1.1; Anl. 7 EPK V.13 p.7",
        play: spri_pv_timeout,
    },
    Szenario {
        name: "wechsel",
        beschreibung: "The switch across both: VA, RUEM-VA with the LineID, AKM-TR, PV for the switch date, the PV leg, completion on the switch date",
        quelle: "S/PRI 4.4 §5.5.1.1 (p. 27); Anl. 1 V5.4.1 §2.6.2; § 59 TKG",
        play: wechsel,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_is_monday_nine_oclock() {
        assert_eq!(START.to_string(), "2027-03-01T08:00:00Z");
    }

    #[test]
    fn names_are_unique_and_found() {
        for s in katalog() {
            assert_eq!(szenario(s.name).map(|x| x.name), Some(s.name));
            assert_eq!(katalog().iter().filter(|x| x.name == s.name).count(), 1);
        }
        assert!(szenario("nope").is_none());
    }
}
