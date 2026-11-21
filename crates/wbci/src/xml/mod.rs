//! XML: reading and writing the four WBCI payloads.
//!
//! Source: XSD V2.00 (`wbci-envelope.xsd`, `wbci-request.xsd`, `wbci-message.xsd`,
//! `wbci-type-complex.xsd`). No schema sets `elementFormDefault`, so only the four payload
//! elements are namespace-qualified; every element inside them is unqualified. Abstract types
//! (`Anfrage`, `Originalanfrage`, `Meldung`, `Position`) carry `xsi:type`.

mod read;
mod write;

pub use carrier_core::xml::{Limits, WriteError, WriteErrorKind};
pub use read::{ReadError, ReadErrorKind};

pub(crate) use carrier_core::xml::{NS_SOAP, NS_XSI};

pub(crate) const NS_ENVELOPE: &str = "http://www.vatm.de/wbci/v1/envelope";
pub(crate) const NS_REQUEST: &str = "http://www.vatm.de/wbci/v1/request";
pub(crate) const NS_MESSAGE: &str = "http://www.vatm.de/wbci/v1/message";

/// Repetition maxima of the schema.
pub(crate) mod max {
    /// `Position`, `Ressource`, `UebernahmeLeitung`, `WeitereAnschlussinhaber`.
    pub(crate) const MANY: usize = 99;
    /// `zuPortierendeOnkzRnr`, `Rufnummer` in an ABBM position.
    pub(crate) const NUMBERS: usize = 10;
    /// `zuPortierenderRufnummernblock`.
    pub(crate) const BLOCKS: usize = 5;
}

/// `xsi:type` names of the concrete types.
pub(crate) mod types {
    pub(crate) const KUE_MRN: &str = "KuendigungMitRNPGeschaeftsfallType";
    pub(crate) const KUE_ORN: &str = "KuendigungOhneRNPGeschaeftsfallType";
    pub(crate) const RRNP: &str = "ReineRufnummernportierungGeschaeftsfallType";
    pub(crate) const STR_AUF_AUF: &str = "StornoAufhebungEKPaufType";
    pub(crate) const STR_AUF_ABG: &str = "StornoAufhebungEKPabgType";
    pub(crate) const STR_AEN_AUF: &str = "StornoAenderungEKPaufType";
    pub(crate) const STR_AEN_ABG: &str = "StornoAenderungEKPabgType";
    pub(crate) const TVS: &str = "TerminverschiebungType";
    pub(crate) const RUEM_VA: &str = "MeldungRUEM_VAType";
    pub(crate) const ABBM: &str = "MeldungABBMType";
    pub(crate) const ERLM: &str = "MeldungERLMType";
    pub(crate) const AKM_TR: &str = "MeldungAKM_TRType";
    pub(crate) const ABBM_TR: &str = "MeldungABBM_TRType";
    pub(crate) const POS_RUEM_VA: &str = "MeldungsPositionRUEM_VAType";
    pub(crate) const POS_ABBM: &str = "MeldungsPositionABBMType";
    pub(crate) const POS_ERLM: &str = "MeldungsPositionERLMType";
    pub(crate) const POS_AKM_TR: &str = "MeldungsPositionAKM_TRType";
    pub(crate) const POS_ABBM_TR: &str = "MeldungsPositionABBM_TRType";
    pub(crate) const POS_TEQ: &str = "MeldungsPositionTEQType";
}
