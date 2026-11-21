//! Requests, messages and the values they are made of.
//!
//! Field names follow the XSD element names in `snake_case`; the XSD name is given where it
//! differs. Mandatory elements are plain fields, optional ones `Option`, repeated ones `Vec`
//! (the schema's maxima are checked when writing and reading).

mod control;
mod message;
mod request;

pub use control::{Control, Payload, Release, SignaturId, Zeitstempel};
pub use message::{
    Abbm, AbbmTr, AdresseAbweichend, AkmTr, Erlm, Meldung, MeldungKopf, PortierteRufnummern, Position, PositionAbbm,
    PositionRuemVa, RuemVa, TechnischeRessource, Teq, UebernahmeLeitung,
};
pub use request::{Anfrage, Bezug, Storno, StornoArt, Terminverschiebung, Vorabstimmung, VorabstimmungsFall};

use crate::{
    Abfragestelle, Anrede, AnredeFirma, CarrierCode, Durchwahlnummer, Firmenname, Hausnummer, HausnummernZusatz,
    Kopplungskenner, Nachname, Onkz, Ortsname, Portierungszeitfenster, Postleitzahl, Projektkenner, Rufnummer,
    RufnummernBlock, Strassenname, Vorname,
};

/// The two parties of a switch (`Endkundenvertragspartner`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ekp {
    /// `EKPauf` — the gaining provider, who asks for the switch.
    pub auf: CarrierCode,
    /// `EKPabg` — the losing provider, whose customer leaves.
    pub abg: CarrierCode,
}

/// Which side of a switch a party is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// The gaining provider (aufnehmender Endkundenvertragspartner).
    EkpAuf,
    /// The losing provider (abgebender Endkundenvertragspartner).
    EkpAbg,
}

impl Role {
    /// The other side.
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::EkpAuf => Self::EkpAbg,
            Self::EkpAbg => Self::EkpAuf,
        }
    }

    /// The name the specifications use.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EkpAuf => "EKPauf",
            Self::EkpAbg => "EKPabg",
        }
    }
}

impl core::fmt::Display for Role {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Ekp {
    /// The carrier code of `role`.
    #[must_use]
    pub const fn of(&self, role: Role) -> &CarrierCode {
        match role {
            Role::EkpAuf => &self.auf,
            Role::EkpAbg => &self.abg,
        }
    }

    /// The role `carrier` plays in this pair, if any.
    #[must_use]
    pub fn role_of(&self, carrier: &CarrierCode) -> Option<Role> {
        if *carrier == self.auf {
            Some(Role::EkpAuf)
        } else if *carrier == self.abg {
            Some(Role::EkpAbg)
        } else {
            None
        }
    }
}

/// A natural person (`PersonType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    /// `anrede`.
    pub anrede: Anrede,
    /// `vorname`.
    pub vorname: Option<Vorname>,
    /// `nachname`.
    pub nachname: Nachname,
}

/// A company (`FirmaType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Firma {
    /// `anrede`.
    pub anrede: AnredeFirma,
    /// `firmenname`.
    pub name: Firmenname,
    /// `firmennameZweiterTeil`.
    pub name_zweiter_teil: Option<Nachname>,
}

/// The customer or a further line holder (`PersonOderFirmaType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kunde {
    /// `person`.
    Person(Person),
    /// `firma`.
    Firma(Firma),
}

impl Kunde {
    /// A person with salutation, optional first name and surname.
    #[must_use]
    pub fn person(anrede: Anrede, vorname: Option<Vorname>, nachname: Nachname) -> Self {
        Self::Person(Person { anrede, vorname, nachname })
    }

    /// A company with salutation `Firma` and no second name part.
    #[must_use]
    pub fn firma(name: Firmenname) -> Self {
        Self::Firma(Firma { anrede: AnredeFirma::Firma, name, name_zweiter_teil: None })
    }
}

/// The location of the line (`StandortType`, the TAE of the resource to take over).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standort {
    /// `strasse/strassenname`.
    pub strassenname: Strassenname,
    /// `strasse/hausnummer`.
    pub hausnummer: Hausnummer,
    /// `strasse/hausnummernZusatz`.
    pub hausnummern_zusatz: Option<HausnummernZusatz>,
    /// `postleitzahl`.
    pub postleitzahl: Postleitzahl,
    /// `ort`.
    pub ort: Ortsname,
}

/// Area code and number (`OnkzRufNrType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OnkzRufnummer {
    /// `ONKZ`.
    pub onkz: Onkz,
    /// `rufnummer`.
    pub rufnummer: Rufnummer,
}

/// The head of a PBX line (`OnkzDurchwahlAbfragestelleType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anlagenkopf {
    /// `ONKZ`.
    pub onkz: Onkz,
    /// `durchwahlnummer`.
    pub durchwahl: Durchwahlnummer,
    /// `abfragestelle`.
    pub abfragestelle: Abfragestelle,
}

/// A block of PBX numbers (`RufnummernblockType`); always given in full (A1 §2.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rufnummernblock {
    /// `rnrBlockVon`.
    pub von: RufnummernBlock,
    /// `rnrBlockBis`.
    pub bis: RufnummernBlock,
}

/// What is ported (`RufnummernportierungType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rufnummernportierung {
    /// `einzelanschluss` or `anlagenanschluss`.
    pub anschluss: Anschluss,
    /// `Portierungszeitfenster`.
    pub zeitfenster: Portierungszeitfenster,
}

/// A single line or a PBX line to port from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Anschluss {
    /// `einzelanschluss` (`PortierungEinzelanschlussType`).
    Einzel {
        /// `rufnummernliste/zuPortierendeOnkzRnr`, 1–10.
        rufnummern: Vec<OnkzRufnummer>,
        /// `alleRufnummern`: port all numbers of the line, not only the ones listed; the
        /// losing provider returns the others with its answer (A1 §2.3).
        alle_rufnummern: bool,
    },
    /// `anlagenanschluss` (`PortierungDurchwahlanlageType`).
    Anlage {
        /// `onkzDurchwahlAbfragestelle`.
        kopf: Anlagenkopf,
        /// `zuPortierenderRufnummernblock`, 1–5.
        bloecke: Vec<Rufnummernblock>,
    },
}

/// A project and optional coupling of orders (`ProjektIDType`, A1 §2.1.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjektId {
    /// `projektkenner`.
    pub projektkenner: Projektkenner,
    /// `kopplungskenner`.
    pub kopplungskenner: Option<Kopplungskenner>,
}
