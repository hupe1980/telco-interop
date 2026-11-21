//! Requests (`Anfrage`): pre-agreement, cancellation, postponement.
//!
//! Source: XSD `wbci-request.xsd` (V2.00); directions A1 V5.4.1 §2.9–§2.11.

use jiff::civil::Date;

use super::{Ekp, Kunde, OnkzRufnummer, ProjektId, Role, Rufnummernportierung, Standort};
use crate::{AenderungsId, CarrierCode, Freitext, Geschaeftsfall, Portierungskennung, StornoId, VorabstimmungsId};

/// A request, sent with `annehmenAnfrage`.
///
/// A request does not store its `Absender`: the kind fixes who sends it (A1 V5.4.1 §2.11,
/// AH §2), so [`Anfrage::absender`] derives it and the reader refuses a contradiction.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum Anfrage {
    /// `VA-KUE-MRN`, `VA-KUE-ORN` or `VA-RRNP`.
    Vorabstimmung(Vorabstimmung),
    /// `STR-AUF` or `STR-AEN`, by either side.
    Storno(Storno),
    /// `TVS-VA`, by the gaining provider only.
    Terminverschiebung(Terminverschiebung),
}

impl Anfrage {
    /// The business case.
    #[must_use]
    pub fn geschaeftsfall(&self) -> Geschaeftsfall {
        match self {
            Self::Vorabstimmung(va) => va.geschaeftsfall(),
            Self::Storno(s) => s.geschaeftsfall(),
            Self::Terminverschiebung(_) => Geschaeftsfall::TvsVa,
        }
    }

    /// The parties of the switch.
    #[must_use]
    pub fn ekp(&self) -> &Ekp {
        match self {
            Self::Vorabstimmung(va) => &va.ekp,
            Self::Storno(s) => &s.ekp,
            Self::Terminverschiebung(t) => &t.ekp,
        }
    }

    /// Who sends this request.
    #[must_use]
    pub fn sender(&self) -> Role {
        match self {
            Self::Vorabstimmung(_) | Self::Terminverschiebung(_) => Role::EkpAuf,
            Self::Storno(s) => s.art.sender(),
        }
    }

    /// The `Absender`: the carrier code of [`Anfrage::sender`].
    #[must_use]
    pub fn absender(&self) -> &CarrierCode {
        self.ekp().of(self.sender())
    }

    /// The Vorabstimmungs-ID this request has or refers to.
    #[must_use]
    pub fn vorabstimmungs_id(&self) -> &VorabstimmungsId {
        match self {
            Self::Vorabstimmung(va) => &va.id,
            Self::Storno(s) => s.bezug.id(),
            Self::Terminverschiebung(t) => t.bezug.id(),
        }
    }
}

impl From<Vorabstimmung> for Anfrage {
    fn from(va: Vorabstimmung) -> Self {
        Self::Vorabstimmung(va)
    }
}
impl From<Storno> for Anfrage {
    fn from(s: Storno) -> Self {
        Self::Storno(s)
    }
}
impl From<Terminverschiebung> for Anfrage {
    fn from(t: Terminverschiebung) -> Self {
        Self::Terminverschiebung(t)
    }
}

/// A pre-agreement request (`VorabstimmungType` and its three concrete types).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vorabstimmung {
    /// `Endkundenvertragspartner`.
    pub ekp: Ekp,
    /// `VorabstimmungsId`.
    pub id: VorabstimmungsId,
    /// `Kundenwunschtermin`: the date the customer wants to switch.
    pub kundenwunschtermin: Date,
    /// `Endkunde`.
    pub endkunde: Kunde,
    /// `WeitereAnschlussinhaber`, at most 99.
    pub weitere_anschlussinhaber: Vec<Kunde>,
    /// `ProjektID`.
    pub projekt: Option<ProjektId>,
    /// The business case and its own elements.
    pub fall: VorabstimmungsFall,
}

/// The elements particular to each pre-agreement case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VorabstimmungsFall {
    /// `VA-KUE-MRN` (`KuendigungMitRNPGeschaeftsfallType`).
    KueMrn {
        /// `Standort`.
        standort: Standort,
        /// `RufnummernPortierung`.
        rufnummernportierung: Rufnummernportierung,
    },
    /// `VA-KUE-ORN` (`KuendigungOhneRNPGeschaeftsfallType`).
    KueOrn {
        /// `Standort`.
        standort: Standort,
        /// `Anschlussidentifikation`: a number to identify the line by.
        anschlussidentifikation: Option<OnkzRufnummer>,
    },
    /// `VA-RRNP` (`ReineRufnummernportierungGeschaeftsfallType`).
    Rrnp {
        /// `RufnummernPortierung`.
        rufnummernportierung: Rufnummernportierung,
        /// `RufnummernPortierung/PortierungskennungPKIauf`: the gaining porting holder.
        pki_auf: Portierungskennung,
    },
}

impl Vorabstimmung {
    /// The business case.
    #[must_use]
    pub const fn geschaeftsfall(&self) -> Geschaeftsfall {
        match self.fall {
            VorabstimmungsFall::KueMrn { .. } => Geschaeftsfall::VaKueMrn,
            VorabstimmungsFall::KueOrn { .. } => Geschaeftsfall::VaKueOrn,
            VorabstimmungsFall::Rrnp { .. } => Geschaeftsfall::VaRrnp,
        }
    }

    /// The `Standort`, for the two `VA-KUE-*` cases.
    #[must_use]
    pub const fn standort(&self) -> Option<&Standort> {
        match &self.fall {
            VorabstimmungsFall::KueMrn { standort, .. } | VorabstimmungsFall::KueOrn { standort, .. } => Some(standort),
            VorabstimmungsFall::Rrnp { .. } => None,
        }
    }

    /// The porting data, for `VA-KUE-MRN` and `VA-RRNP`.
    #[must_use]
    pub const fn rufnummernportierung(&self) -> Option<&Rufnummernportierung> {
        match &self.fall {
            VorabstimmungsFall::KueMrn { rufnummernportierung, .. }
            | VorabstimmungsFall::Rrnp { rufnummernportierung, .. } => Some(rufnummernportierung),
            VorabstimmungsFall::KueOrn { .. } => None,
        }
    }
}

/// What a cancellation or postponement refers to (`VorabstimmungsdatenType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bezug {
    /// `VorabstimmungsIdRef`: the id alone, sufficient by A1 §2.9.1.
    Id(VorabstimmungsId),
    /// `Originalanfrage`: the original request repeated.
    Originalanfrage(Box<Vorabstimmung>),
}

impl Bezug {
    /// The Vorabstimmungs-ID referred to.
    #[must_use]
    pub fn id(&self) -> &VorabstimmungsId {
        match self {
            Self::Id(id) => id,
            Self::Originalanfrage(va) => &va.id,
        }
    }
}

/// A cancellation (`StornoType` and its four concrete types).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Storno {
    /// `Endkundenvertragspartner`.
    pub ekp: Ekp,
    /// `StornoId`, issued by the sender.
    pub storno_id: StornoId,
    /// `VorabstimmungsDaten`.
    pub bezug: Bezug,
    /// Which cancellation, by whom, with its own elements.
    pub art: StornoArt,
}

/// The four cancellation types: Aufhebung (cancel for good) or Änderung (cancel to resubmit),
/// by the gaining or the losing provider (A1 §2.9, §2.10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StornoArt {
    /// `StornoAufhebungEKPaufType`.
    AufhebungAuf,
    /// `StornoAufhebungEKPabgType`: only for the reasons of A1 §2.9.2.
    AufhebungAbg {
        /// `Name`.
        name: Option<Kunde>,
        /// `Standort`.
        standort: Option<Standort>,
        /// `Stornogrund`.
        stornogrund: Freitext,
    },
    /// `StornoAenderungEKPaufType`.
    AenderungAuf {
        /// `Name`.
        name: Option<Kunde>,
        /// `Standort`.
        standort: Option<Standort>,
    },
    /// `StornoAenderungEKPabgType`: for the reasons of A1 §2.10.2.
    AenderungAbg {
        /// `Name`.
        name: Option<Kunde>,
        /// `Standort`.
        standort: Option<Standort>,
        /// `Stornogrund`.
        stornogrund: Freitext,
    },
}

impl StornoArt {
    /// Who sends this cancellation.
    #[must_use]
    pub const fn sender(&self) -> Role {
        match self {
            Self::AufhebungAuf | Self::AenderungAuf { .. } => Role::EkpAuf,
            Self::AufhebungAbg { .. } | Self::AenderungAbg { .. } => Role::EkpAbg,
        }
    }

    /// Whether this is an Aufhebung (`STR-AUF`) rather than an Änderung (`STR-AEN`).
    #[must_use]
    pub const fn is_aufhebung(&self) -> bool {
        matches!(self, Self::AufhebungAuf | Self::AufhebungAbg { .. })
    }
}

impl Storno {
    /// `STR-AUF` or `STR-AEN`.
    #[must_use]
    pub const fn geschaeftsfall(&self) -> Geschaeftsfall {
        if self.art.is_aufhebung() { Geschaeftsfall::StrAuf } else { Geschaeftsfall::StrAen }
    }
}

/// A postponement by the gaining provider (`TerminverschiebungType`, A1 §2.11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terminverschiebung {
    /// `Endkundenvertragspartner`.
    pub ekp: Ekp,
    /// `AenderungsId`.
    pub aenderungs_id: AenderungsId,
    /// `VorabstimmungsDaten`.
    pub bezug: Bezug,
    /// `Name`.
    pub name: Option<Kunde>,
    /// `NeuerKundenwunschtermin`: only later than the agreed date.
    pub neuer_kundenwunschtermin: Date,
}
