//! Messages (`Meldung`) and the technical receipt (`TEQ`).
//!
//! Source: XSD `wbci-message.xsd` (V2.00); message kinds AH4WBCI 2.10 §5.2.

use jiff::civil::Date;

use super::{Anlagenkopf, Ekp, OnkzRufnummer, Rufnummernblock};
use crate::{
    AenderungsId, CarrierCode, Freitext, Geschaeftsfall, Hausnummer, Identifizierer, LineId, Meldungscode,
    Meldungstext, Meldungstyp, Ortsname, Portierungskennung, Postleitzahl, Rufnummer, StornoId, Strassenname,
    Technologie, Vertragsnummer, VorabstimmungsId,
};

/// A position of a message: a code and its text (`AbstractMeldungsPositionType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    /// `Meldungscode`.
    pub code: Meldungscode,
    /// `Meldungstext`.
    pub text: Meldungstext,
}

impl Position {
    /// A position with the code's official text.
    ///
    /// For an unknown code (no official text) the text is the code itself.
    #[must_use]
    pub fn new(code: Meldungscode) -> Self {
        let text = Meldungstext::new(code.text().unwrap_or(code.as_str())).unwrap_or_else(|_| Meldungstext::dash());
        Self { code, text }
    }

    /// A position with an own text, e.g. the reason for `8116 Sonstiges`.
    #[must_use]
    pub const fn with_text(code: Meldungscode, text: Meldungstext) -> Self {
        Self { code, text }
    }
}

/// A RUEM-VA position, which may carry a corrected address part (`MeldungsPositionRUEM_VAType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionRuemVa {
    /// Code and text.
    pub position: Position,
    /// `AdresseAbweichend`: the address part as the losing provider has it (codes 8104–8107).
    pub adresse_abweichend: Option<AdresseAbweichend>,
}

/// An ABBM position, which may list numbers (`MeldungsPositionABBMType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionAbbm {
    /// Code and text.
    pub position: Position,
    /// `Rufnummer`, at most 10: with code 8103 (RNG), the numbers the losing provider has.
    pub rufnummern: Vec<Rufnummer>,
}

impl From<Position> for PositionRuemVa {
    fn from(position: Position) -> Self {
        Self { position, adresse_abweichend: None }
    }
}

impl From<Position> for PositionAbbm {
    fn from(position: Position) -> Self {
        Self { position, rufnummern: Vec::new() }
    }
}

/// A corrected address part (`AdresseAbweichendType`, ADA codes 8104–8107).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdresseAbweichend {
    /// `strassenname` (ADASTR).
    Strassenname(Strassenname),
    /// `postleitzahl` (ADAPLZ).
    Postleitzahl(Postleitzahl),
    /// `ort` (ADAORT).
    Ort(Ortsname),
    /// `hausnummer` (ADAHSNR).
    Hausnummer(Hausnummer),
}

/// The technical resource the losing provider names (`TechnischeRessourceType`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TechnischeRessource {
    /// `Vertragsnummer`: the WITA contract number.
    pub vertragsnummer: Option<Vertragsnummer>,
    /// `LineID`: the S/PRI line.
    pub line_id: Option<LineId>,
    /// `Identifizierer`: the losing provider's own reference.
    pub identifizierer: Option<Identifizierer>,
    /// `KennungTNBabg`: porting id of the losing network operator.
    pub kennung_tnb_abg: Option<Portierungskennung>,
}

/// A line the gaining provider takes over (`UebernahmeLeitungType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UebernahmeLeitung {
    /// `Vertragsnummer` (WITA).
    Vertragsnummer(Vertragsnummer),
    /// `LineID` (S/PRI).
    LineId(LineId),
}

/// The numbers the losing provider confirms for porting (`RufnummernportierungMeldungType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortierteRufnummern {
    /// `portierungRufnummern`: numbers with the losing porting holder, 1–10.
    Rufnummern(Vec<(OnkzRufnummer, Portierungskennung)>),
    /// `portierungRufnummernbloecke`: PBX blocks with the losing porting holder, 1–5.
    Bloecke {
        /// `onkzDurchwahlAbfragestelle`.
        kopf: Anlagenkopf,
        /// `zuPortierenderRufnummernblock`.
        bloecke: Vec<(Rufnummernblock, Portierungskennung)>,
    },
}

/// The elements every message except TEQ starts with (`AbstractMeldungType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeldungKopf {
    /// `Absender`: who sends this message.
    pub absender: CarrierCode,
    /// `VorabstimmungsIdRef`.
    pub vorabstimmungs_id: VorabstimmungsId,
    /// `Endkundenvertragspartner`.
    pub ekp: Ekp,
    /// `Geschaeftsfall`: the case of the request answered.
    pub geschaeftsfall: Geschaeftsfall,
}

/// A business message, sent with `annehmenMeldung`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Meldung {
    /// RUEM-VA: the losing provider agrees, with a binding switch date.
    RuemVa(RuemVa),
    /// ABBM: a request is refused.
    Abbm(Abbm),
    /// ERLM: a cancellation or postponement is done.
    Erlm(Erlm),
    /// AKM-TR: the gaining provider announces whether it takes over the line.
    AkmTr(AkmTr),
    /// ABBM-TR: the losing provider refuses the AKM-TR.
    AbbmTr(AbbmTr),
}

/// RUEM-VA (`MeldungRUEM_VAType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuemVa {
    /// Absender, ids, parties, case.
    pub kopf: MeldungKopf,
    /// `Position`, 1–99: 8001 or 8002, plus ADA hints 8104–8107.
    pub positionen: Vec<PositionRuemVa>,
    /// `Ressource`, at most 99.
    pub ressourcen: Vec<TechnischeRessource>,
    /// `Technologie` (Anlage 5).
    pub technologie: Option<Technologie>,
    /// `Wechseltermin`: the binding switch date.
    pub wechseltermin: Date,
    /// `RufnummernPortierung`: the numbers to be ported.
    pub rufnummernportierung: Option<PortierteRufnummern>,
}

/// ABBM (`MeldungABBMType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abbm {
    /// Absender, ids, parties, case.
    pub kopf: MeldungKopf,
    /// `Position`, 1–99.
    pub positionen: Vec<PositionAbbm>,
    /// `Wechseltermin`: for a refused postponement, the date that stays valid (A1 §2.11).
    pub wechseltermin: Option<Date>,
    /// `StornoIdRef`.
    pub storno_id: Option<StornoId>,
    /// `AenderungsIdRef`.
    pub aenderungs_id: Option<AenderungsId>,
    /// `Begruendung`.
    pub begruendung: Option<Freitext>,
}

/// ERLM (`MeldungERLMType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Erlm {
    /// Absender, ids, parties, case.
    pub kopf: MeldungKopf,
    /// `Position`, 1–99: 8003 or 8004.
    pub positionen: Vec<Position>,
    /// `Wechseltermin`: for an accepted postponement, the new date.
    pub wechseltermin: Option<Date>,
    /// `StornoIdRef`.
    pub storno_id: Option<StornoId>,
    /// `AenderungsIdRef`.
    pub aenderungs_id: Option<AenderungsId>,
}

/// AKM-TR (`MeldungAKM_TRType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AkmTr {
    /// Absender, ids, parties, case.
    pub kopf: MeldungKopf,
    /// `Position`, 1–99: 8010.
    pub positionen: Vec<Position>,
    /// `Ressourcenuebernahme`: whether the gaining provider takes over the line.
    pub ressourcenuebernahme: bool,
    /// `UebernahmeLeitung`, at most 99.
    pub uebernahme_leitung: Vec<UebernahmeLeitung>,
    /// `SichererHafen`: the „sicherer Hafen" procedure is requested.
    pub sicherer_hafen: bool,
    /// `PortierungskennungPKIauf`.
    pub pki_auf: Option<Portierungskennung>,
}

/// ABBM-TR (`MeldungABBM_TRType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbbmTr {
    /// Absender, ids, parties, case.
    pub kopf: MeldungKopf,
    /// `Position`, 1–99: 8140, 8143–8148.
    pub positionen: Vec<Position>,
}

/// The technical receipt (`SystemmeldungTEQType`), the synchronous answer to every call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Teq {
    /// `Absender`: the receiver of the call.
    pub absender: CarrierCode,
    /// `VorabstimmungsIdRef`.
    pub vorabstimmungs_id: VorabstimmungsId,
    /// `Position`, 1–99: `OK`, `0993`, `0995` or `0999`.
    pub positionen: Vec<Position>,
    /// `StornoIdRef`, when acknowledging a cancellation or its answers.
    pub storno_id: Option<StornoId>,
    /// `AenderungsIdRef`, when acknowledging a postponement or its answers.
    pub aenderungs_id: Option<AenderungsId>,
}

impl Teq {
    /// Whether the receipt is positive: every position is `OK`.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        !self.positionen.is_empty() && self.positionen.iter().all(|p| p.code.is_ok())
    }
}

impl Meldung {
    /// The common head.
    #[must_use]
    pub const fn kopf(&self) -> &MeldungKopf {
        match self {
            Self::RuemVa(m) => &m.kopf,
            Self::Abbm(m) => &m.kopf,
            Self::Erlm(m) => &m.kopf,
            Self::AkmTr(m) => &m.kopf,
            Self::AbbmTr(m) => &m.kopf,
        }
    }

    /// The kind.
    #[must_use]
    pub const fn meldungstyp(&self) -> Meldungstyp {
        match self {
            Self::RuemVa(_) => Meldungstyp::RuemVa,
            Self::Abbm(_) => Meldungstyp::Abbm,
            Self::Erlm(_) => Meldungstyp::Erlm,
            Self::AkmTr(_) => Meldungstyp::AkmTr,
            Self::AbbmTr(_) => Meldungstyp::AbbmTr,
        }
    }

    /// The codes of all positions.
    pub fn codes(&self) -> impl Iterator<Item = &Meldungscode> {
        let positions: Vec<&Position> = match self {
            Self::RuemVa(m) => m.positionen.iter().map(|p| &p.position).collect(),
            Self::Abbm(m) => m.positionen.iter().map(|p| &p.position).collect(),
            Self::Erlm(m) => m.positionen.iter().collect(),
            Self::AkmTr(m) => m.positionen.iter().collect(),
            Self::AbbmTr(m) => m.positionen.iter().collect(),
        };
        positions.into_iter().map(|p| &p.code)
    }

    /// The `StornoId` referred to, for ABBM and ERLM.
    #[must_use]
    pub const fn storno_id(&self) -> Option<&StornoId> {
        match self {
            Self::Abbm(m) => m.storno_id.as_ref(),
            Self::Erlm(m) => m.storno_id.as_ref(),
            _ => None,
        }
    }

    /// The `AenderungsId` referred to, for ABBM and ERLM.
    #[must_use]
    pub const fn aenderungs_id(&self) -> Option<&AenderungsId> {
        match self {
            Self::Abbm(m) => m.aenderungs_id.as_ref(),
            Self::Erlm(m) => m.aenderungs_id.as_ref(),
            _ => None,
        }
    }

    /// The switch date the message names, if any.
    #[must_use]
    pub const fn wechseltermin(&self) -> Option<Date> {
        match self {
            Self::RuemVa(m) => Some(m.wechseltermin),
            Self::Abbm(m) => m.wechseltermin,
            Self::Erlm(m) => m.wechseltermin,
            _ => None,
        }
    }
}

macro_rules! from_kind {
    ($($kind:ident),+) => {$(
        impl From<$kind> for Meldung {
            fn from(m: $kind) -> Self {
                Self::$kind(m)
            }
        }
    )+};
}
from_kind!(RuemVa, Abbm, Erlm, AkmTr, AbbmTr);
