//! Replaying the files of one switch from one party's view.

use std::path::PathBuf;

use telco_interop::spri::process::{Direction as SDir, Message as SMessage, Process as SProcess, Role as SRole};
use telco_interop::wbci::process::Message as WMessage;
use telco_interop::{
    Befund, Eintrag, Frist, FristDetail, Partei, Richtung, Schnittstelle, VorabstimmungsId, Wechsel, spri, wbci,
};

use crate::input::{self, Parsed};
use crate::summary::{meldung_ext, meldung_va};

/// One readable file.
pub(crate) struct File {
    pub(crate) path: PathBuf,
    pub(crate) at: wbci::Timestamp,
    pub(crate) payload: Parsed,
}

/// A file not replayed, and why.
pub(crate) struct Note {
    pub(crate) path: PathBuf,
    pub(crate) reason: String,
}

/// What a replay found.
pub(crate) struct Report {
    pub(crate) id: Option<VorabstimmungsId>,
    pub(crate) now: Option<wbci::Timestamp>,
    pub(crate) timeline: Vec<Eintrag>,
    pub(crate) befunde: Vec<Befund>,
    /// Open (`false`) and overdue (`true`) deadlines at `now`.
    pub(crate) deadlines: Vec<(bool, Frist)>,
    /// Why there are no cross-interface findings, if there cannot be any.
    pub(crate) remark: Option<&'static str>,
    pub(crate) skipped: Vec<Note>,
    pub(crate) refused: Vec<Note>,
    pub(crate) unreadable: Vec<Note>,
}

impl Report {
    /// Exit status 1: a finding, an overdue deadline, a refused or unreadable file, or no switch.
    pub(crate) fn failed(&self) -> bool {
        self.id.is_none()
            || !self.befunde.is_empty()
            || self.deadlines.iter().any(|(overdue, _)| *overdue)
            || !self.refused.is_empty()
            || !self.unreadable.is_empty()
    }
}

/// Lists, reads, orders and replays `inputs` as `party`.
///
/// # Errors
///
/// The listing exceeds [`input::MAX_FILES`] or a folder cannot be listed.
pub(crate) fn replay(inputs: &[PathBuf], party: Partei, now: Option<wbci::Timestamp>) -> Result<Report, String> {
    let mut listed = Vec::new();
    let paths = input::list(inputs, &mut listed)?;
    let skipped: Vec<Note> = listed.into_iter().map(|s| Note { path: s.path, reason: s.reason.to_owned() }).collect();
    let mut unreadable = Vec::new();
    let mut files = Vec::new();
    for path in paths {
        match input::read_file(&path) {
            Ok(payload) => files.push(File { at: payload.at(), path, payload }),
            Err(f) => unreadable.push(Note { path, reason: f.describe() }),
        }
    }
    // Stable: files with the same timestamp keep their listing order.
    files.sort_by_key(|f| f.at);
    let now = now.or_else(|| files.iter().map(|f| f.at).max());
    let id = switch_id(&files);
    let mut r = Replay { party, id: id.clone(), wbci: None, spri: Vec::new(), skipped, refused: Vec::new() };
    if id.is_some() {
        for f in files {
            match f.payload {
                Parsed::Wbci(p) => r.wbci_file(f.path, f.at, *p),
                Parsed::Spri(p) => r.spri_file(f.path, f.at, *p),
            }
        }
    } else {
        for f in files {
            r.skipped.push(Note { path: f.path, reason: "no Vorabstimmungs-ID found in the files".to_owned() });
        }
    }
    let (timeline, befunde, deadlines, remark) = r.result();
    let deadlines = match now {
        Some(now) => deadlines
            .into_iter()
            .filter_map(|f| {
                let overdue = f.due <= now;
                if overdue && is_window(&f) { None } else { Some((overdue, f)) }
            })
            .collect(),
        None => Vec::new(),
    };
    Ok(Report { id, now, timeline, befunde, deadlines, remark, skipped: r.skipped, refused: r.refused, unreadable })
}

/// The last moment for an optional act: when it passes, nobody is in default.
fn is_window(f: &Frist) -> bool {
    match &f.detail {
        FristDetail::Wbci(d) => matches!(
            d.kind,
            wbci::deadline::DeadlineKind::StornoAufhebungLatest
                | wbci::deadline::DeadlineKind::StornoAenderungLatest
                | wbci::deadline::DeadlineKind::TerminverschiebungLatest
        ),
        FristDetail::Spri(d) => d.is_window(),
    }
}

/// The switch's Vorabstimmungs-ID: the first WBCI pre-agreement's, else the first S/PRI
/// order's or AKM-PV's that carries one.
fn switch_id(files: &[File]) -> Option<VorabstimmungsId> {
    let va = files.iter().find_map(|f| match &f.payload {
        Parsed::Wbci(p) => match &**p {
            wbci::Payload::AnfrageRequest { anfrage: wbci::Anfrage::Vorabstimmung(va), .. } => Some(va.id.clone()),
            _ => None,
        },
        Parsed::Spri(_) => None,
    });
    va.or_else(|| {
        files.iter().find_map(|f| match &f.payload {
            Parsed::Spri(p) => match &**p {
                spri::Payload::AuftragRequest { auftrag, .. } => auftrag.geschaeftsfall.vorabstimmungs_id().cloned(),
                spri::Payload::MeldungRequest { meldung, .. } if meldung.typ() == spri::Meldungstyp::AkmPv => {
                    meldung_va(meldung).cloned()
                }
                _ => None,
            },
            Parsed::Wbci(_) => None,
        })
    })
}

const fn wbci_role(party: Partei) -> Option<wbci::Role> {
    match party {
        Partei::EkpAuf => Some(wbci::Role::EkpAuf),
        Partei::EkpAbg => Some(wbci::Role::EkpAbg),
        Partei::Netzbetreiber => None,
    }
}

/// The party an S/PRI role plays for an order of business case `fall`: in a `KUE-AG` for the
/// switch the Auftraggeber is the losing provider.
const fn partei_of(role: SRole, fall: spri::GeschaeftsfallCode) -> Partei {
    match role {
        SRole::Auftraggeber if matches!(fall, spri::GeschaeftsfallCode::KueAg) => Partei::EkpAbg,
        SRole::Auftraggeber => Partei::EkpAuf,
        SRole::Leistungserbringer => Partei::Netzbetreiber,
        SRole::AbgebenderProvider => Partei::EkpAbg,
    }
}

/// Who sends a message of type `typ`.
const fn meldung_sender(typ: spri::Meldungstyp) -> SRole {
    use spri::Meldungstyp as T;
    match typ {
        T::RuemPv => SRole::AbgebenderProvider,
        T::ErlmK | T::ZwmAg | T::TbkAg => SRole::Auftraggeber,
        _ => SRole::Leistungserbringer,
    }
}

/// The messages between the network owner and the losing provider of a provider switch.
const fn is_pv_leg(typ: spri::Meldungstyp) -> bool {
    use spri::Meldungstyp as T;
    matches!(typ, T::AkmPv | T::RuemPv | T::AbmPv | T::ErlmPv | T::EntmPv | T::AbbmPv)
}

struct Replay {
    party: Partei,
    id: Option<VorabstimmungsId>,
    wbci: Option<wbci::Process>,
    spri: Vec<SProcess>,
    skipped: Vec<Note>,
    refused: Vec<Note>,
}

/// Why a file was not replayed: it does not belong (skipped) or a process refused it.
enum Not {
    Skip(String),
    Refuse(String),
}

type Step = Result<(), Not>;

fn skip<T>(reason: impl Into<String>) -> Result<T, Not> {
    Err(Not::Skip(reason.into()))
}

fn refuse<T>(reason: impl Into<String>) -> Result<T, Not> {
    Err(Not::Refuse(reason.into()))
}

impl Replay {
    fn note(&mut self, path: PathBuf, step: Step) {
        match step {
            Ok(()) => {}
            Err(Not::Skip(reason)) => self.skipped.push(Note { path, reason }),
            Err(Not::Refuse(reason)) => self.refused.push(Note { path, reason }),
        }
    }

    fn other_switch(&self, id: &VorabstimmungsId) -> bool {
        self.id.as_ref() != Some(id)
    }

    fn wbci_file(&mut self, path: PathBuf, at: wbci::Timestamp, payload: wbci::Payload) {
        let step = self.wbci_step(at, payload);
        self.note(path, step);
    }

    fn spri_file(&mut self, path: PathBuf, at: wbci::Timestamp, payload: spri::Payload) {
        let step = self.spri_step(at, payload);
        self.note(path, step);
    }

    fn wbci_step(&mut self, at: wbci::Timestamp, payload: wbci::Payload) -> Step {
        let Some(role) = wbci_role(self.party) else {
            return skip("WBCI: the network owner is no WBCI party");
        };
        let (id, message) = match payload {
            wbci::Payload::AnfrageRequest { anfrage, .. } => {
                (anfrage.vorabstimmungs_id().clone(), WMessage::from(anfrage))
            }
            wbci::Payload::MeldungRequest { meldung, .. } => {
                (meldung.kopf().vorabstimmungs_id.clone(), WMessage::from(meldung))
            }
            wbci::Payload::AnfrageResponse { quittung, .. } | wbci::Payload::MeldungResponse { quittung, .. } => {
                (quittung.vorabstimmungs_id.clone(), WMessage::from(quittung))
            }
        };
        if self.other_switch(&id) {
            return skip(format!("WBCI: Vorabstimmungs-ID {id}, another switch"));
        }
        let Some(process) = self.wbci.as_mut() else {
            let WMessage::Anfrage(wbci::Anfrage::Vorabstimmung(va)) = message else {
                return refuse("WBCI: before the pre-agreement; the WBCI view starts with the VA");
            };
            let direction = if role == wbci::Role::EkpAuf { wbci::Direction::Sent } else { wbci::Direction::Received };
            let p = wbci::Process::new(role, va, direction, at).or_else(|e| refuse(format!("WBCI: {e}")))?;
            self.wbci = Some(p);
            return Ok(());
        };
        let sender = match &message {
            WMessage::Anfrage(a) => Some(a.sender()),
            WMessage::Meldung(m) => m.kopf().ekp.role_of(&m.kopf().absender),
            WMessage::Teq(t) => process.vorabstimmung().ekp.role_of(&t.absender),
        };
        let Some(sender) = sender else {
            return refuse("WBCI: the Absender is neither EKP of the switch");
        };
        let direction = if sender == role { wbci::Direction::Sent } else { wbci::Direction::Received };
        process.apply(direction, at, message).or_else(|e| refuse(format!("WBCI: {e}")))
    }

    fn spri_step(&mut self, at: wbci::Timestamp, payload: spri::Payload) -> Step {
        let is_order_receipt = matches!(payload, spri::Payload::AuftragResponse { .. });
        let (ext, id, message) = match payload {
            spri::Payload::AuftragRequest { auftrag, .. } => {
                let id = auftrag.geschaeftsfall.vorabstimmungs_id().cloned();
                (Some(auftrag.externe_auftragsnummer.clone()), id, SMessage::Auftrag(auftrag))
            }
            spri::Payload::MeldungRequest { meldung, .. } => {
                (Some(meldung_ext(&meldung).clone()), meldung_va(&meldung).cloned(), SMessage::Meldung(meldung))
            }
            spri::Payload::AuftragResponse { quittung, .. } | spri::Payload::MeldungResponse { quittung, .. } => {
                (quittung.externe_auftragsnummer.clone(), None, SMessage::Teq(quittung))
            }
        };
        if let Some(id) = &id
            && self.other_switch(id)
        {
            return skip(format!("S/PRI: Vorabstimmungs-ID {id}, another switch"));
        }
        let index = match &ext {
            Some(ext) => self.spri.iter().position(|p| p.externe_auftragsnummer() == ext),
            None if self.spri.len() == 1 => Some(0),
            None => return refuse("S/PRI: a receipt without order number, and more than one order"),
        };
        let party = self.party;
        let Some(process) = index.and_then(|i| self.spri.get_mut(i)) else {
            return self.spri_start(at, ext, &message);
        };
        let owner = process.role();
        let fall = process.geschaeftsfall();
        let visible = match (&message, owner) {
            (SMessage::Meldung(m), SRole::Auftraggeber) => !is_pv_leg(m.typ()),
            (SMessage::Meldung(m), SRole::AbgebenderProvider) => is_pv_leg(m.typ()),
            (SMessage::Auftrag(_), SRole::AbgebenderProvider) => false,
            _ => true,
        };
        if !visible {
            return skip(format!("S/PRI: not in the {party}'s view"));
        }
        let first = match &message {
            SMessage::Auftrag(_) => {
                if partei_of(SRole::Auftraggeber, fall) == party {
                    SDir::Sent
                } else {
                    SDir::Received
                }
            }
            SMessage::Meldung(m) => {
                if partei_of(meldung_sender(m.typ()), fall) == party {
                    SDir::Sent
                } else {
                    SDir::Received
                }
            }
            // A receipt is sent by the receiver of what it answers: the network owner for an
            // order, mostly the other side for a message. The other direction is tried next.
            SMessage::Teq(_) => match (owner == SRole::Leistungserbringer, is_order_receipt) {
                (true, true) | (false, false) => SDir::Sent,
                (true, false) | (false, true) => SDir::Received,
            },
        };
        let is_teq = matches!(message, SMessage::Teq(_));
        let result = match process.apply(first, at, message.clone()) {
            Err(e) if is_teq => {
                let other = if first == SDir::Sent { SDir::Received } else { SDir::Sent };
                process.apply(other, at, message).map_err(|_| e)
            }
            other => other,
        };
        match result {
            Ok(()) => Ok(()),
            // Only the network owner sees every exchange; another party's folder may hold
            // receipts of messages between the others.
            Err(_) if is_teq && party != Partei::Netzbetreiber => {
                skip(format!("S/PRI: a receipt nothing in the {party}'s view awaits"))
            }
            Err(e) => refuse(format!("S/PRI: {e}")),
        }
    }

    /// Starts an S/PRI process from an order, a KDLE or an AKM-PV, when the party sees it.
    fn spri_start(
        &mut self,
        at: wbci::Timestamp,
        ext: Option<spri::ExterneAuftragsnummer>,
        message: &SMessage,
    ) -> Step {
        let ext = ext.map_or_else(String::new, |e| e.to_string());
        let party = self.party;
        let started = match message {
            SMessage::Auftrag(a) => {
                let ordering = partei_of(SRole::Auftraggeber, a.geschaeftsfall.code());
                if ordering == party {
                    SProcess::new(SRole::Auftraggeber, a, SDir::Sent, at)
                } else if party == Partei::Netzbetreiber {
                    SProcess::new(SRole::Leistungserbringer, a, SDir::Received, at)
                } else {
                    return skip(format!("S/PRI: order {ext} is placed by the {ordering}"));
                }
            }
            SMessage::Meldung(m) if m.typ() == spri::Meldungstyp::AkmPv && party == Partei::EkpAbg => {
                SProcess::from_akm_pv(m, at)
            }
            SMessage::Meldung(m) if m.typ() == spri::Meldungstyp::Kdle => {
                if party == Partei::Netzbetreiber {
                    SProcess::from_kdle(SRole::Leistungserbringer, m, SDir::Sent, at)
                } else {
                    SProcess::from_kdle(SRole::Auftraggeber, m, SDir::Received, at)
                }
            }
            SMessage::Meldung(m)
                if (party == Partei::EkpAuf && is_pv_leg(m.typ()))
                    || (party == Partei::EkpAbg && !is_pv_leg(m.typ())) =>
            {
                return skip(format!("S/PRI: not in the {party}'s view"));
            }
            SMessage::Teq(_) if party != Partei::Netzbetreiber => {
                return skip(format!("S/PRI: a receipt for order {ext}, which is not in the {party}'s view"));
            }
            SMessage::Meldung(_) | SMessage::Teq(_) => {
                return refuse(format!("S/PRI: no order {ext} in the replay to apply it to"));
            }
        };
        let p = started.or_else(|e| refuse(format!("S/PRI: {e}")))?;
        match p.vorabstimmungs_id() {
            Some(id) if Some(id) == self.id.as_ref() => {
                self.spri.push(p);
                Ok(())
            }
            Some(id) => skip(format!("S/PRI: order {ext} carries Vorabstimmungs-ID {id}, another switch")),
            None => skip(format!("S/PRI: order {ext} carries no Vorabstimmungs-ID")),
        }
    }

    /// The timeline, findings and deadlines: from the [`Wechsel`] when the WBCI pre-agreement
    /// is part of the replay, else from the S/PRI processes alone.
    fn result(&mut self) -> (Vec<Eintrag>, Vec<Befund>, Vec<Frist>, Option<&'static str>) {
        let spri = std::mem::take(&mut self.spri);
        if let Some(p) = self.wbci.take() {
            let mut w = Wechsel::new(p);
            for s in spri {
                if let Err(m) = w.attach(s) {
                    let path = PathBuf::from(format!("order {}", m.prozess.externe_auftragsnummer()));
                    self.refused.push(Note { path, reason: m.to_string() });
                }
            }
            return (w.timeline(), w.befunde(), w.deadlines(), None);
        }
        let mut timeline = Vec::new();
        let mut deadlines = Vec::new();
        for p in &spri {
            let fall = p.geschaeftsfall();
            for e in p.history() {
                timeline.push(Eintrag {
                    at: e.at,
                    schnittstelle: Schnittstelle::Spri,
                    partei: partei_of(e.sender, fall),
                    richtung: match e.direction {
                        SDir::Sent => Richtung::Gesendet,
                        SDir::Received => Richtung::Empfangen,
                    },
                    nachricht: spri_label(&e.message),
                    codes: spri_codes(&e.message),
                });
            }
            for d in p.deadlines() {
                deadlines.push(Frist {
                    schnittstelle: Schnittstelle::Spri,
                    partei: partei_of(d.party, fall),
                    auftrag: Some(p.externe_auftragsnummer().clone()),
                    due: d.due,
                    detail: FristDetail::Spri(d),
                });
            }
        }
        timeline.sort_by_key(|e| e.at);
        deadlines.sort_by_key(|f| f.due);
        let remark = if self.party == Partei::Netzbetreiber {
            "the network owner sees no WBCI: the cross-interface rules are not checked"
        } else {
            "no WBCI pre-agreement in the replay: the cross-interface rules are not checked"
        };
        (timeline, Vec::new(), deadlines, Some(remark))
    }
}

const fn spri_label(m: &SMessage) -> &'static str {
    match m {
        SMessage::Auftrag(a) => match a.aenderungskennzeichen {
            spri::Aenderungskennzeichen::Standard => a.geschaeftsfall.code().as_str(),
            spri::Aenderungskennzeichen::Storno => "Storno",
            spri::Aenderungskennzeichen::Terminverschiebung => "Terminverschiebung",
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
