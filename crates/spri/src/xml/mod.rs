//! XML: reading and writing the four S/PRI payloads.
//!
//! Source: XSD 4.00.03 (`oss-envelope.xsd`, `oss-order.xsd`, `oss-message.xsd`,
//! `oss-product-FTTx.xsd`, `oss-type-complex.xsd`). No schema sets `elementFormDefault`, so
//! only the four payload elements are namespace-qualified; every element inside them is
//! unqualified. Abstract types carry `xsi:type`: the business case (`geschaeftsfall`), the
//! product (`produkt`), the FTTx product data (`geschaeftsfallProdukt`), the message
//! (`meldungstyp`) and the position attributes (`positionsattribute`).
//!
//! Attachments (`inhalt`, `xsd:base64Binary`) are bounded by [`Limits::max_attachment`]; the
//! S/PRI documents name no size, so the bound is this library's (1 MiB by default, see
//! [`default_limits`]).

mod base64;
mod read;
mod write;

pub use carrier_core::xml::{Limits, WriteError, WriteErrorKind};
pub use read::{ReadError, ReadErrorKind};

pub(crate) const NS_ENVELOPE: &str = "http://spri.telekom.de/oss/v4/envelope";
pub(crate) const NS_ORDER: &str = "http://spri.telekom.de/oss/v4/order";
pub(crate) const NS_MESSAGE: &str = "http://spri.telekom.de/oss/v4/message";
pub(crate) const NS_FTTX: &str = "http://spri.telekom.de/oss/v4/fttx";

/// The largest attachment [`default_limits`] admits: 1 MiB after decoding.
pub const MAX_ANLAGE_BYTES: usize = 1 << 20;

/// The limits [`crate::Payload::from_xml`] applies: [`Limits::default`] plus room for one
/// attachment of [`MAX_ANLAGE_BYTES`] in base64.
#[must_use]
pub const fn default_limits() -> Limits {
    Limits::for_attachments(MAX_ANLAGE_BYTES)
}

/// Repetition bounds. The schema leaves most lists unbounded; the reader's element limit
/// bounds them then.
pub(crate) mod max {
    /// `maxOccurs="unbounded"`.
    pub(crate) const UNBOUNDED: usize = usize::MAX;
    /// `zuPortierenderRufnummernblock`.
    pub(crate) const BLOCKS: usize = 5;
    /// `zuPortierendeOnkzRnr`.
    pub(crate) const NUMBERS: usize = 10;
}

/// `xsi:type` names of the concrete types.
pub(crate) mod types {
    pub(crate) const NEU: &str = "BereitstellungType";
    pub(crate) const KUE_AG: &str = "KuendigungType";
    pub(crate) const LAE: &str = "LeistungsaenderungType";
    pub(crate) const AEN_LMAE: &str = "LeistungsmerkmalAenderungType";
    pub(crate) const EST: &str = "EntstoerungType";
    pub(crate) const PV: &str = "ProviderwechselType";
    pub(crate) const GET: &str = "DiagnoseGetType";
    pub(crate) const SET: &str = "DiagnoseSetType";
    pub(crate) const PRODUKT: &str = "ProduktFTTXType";
    pub(crate) const FTTX_NEU: &str = "FTTXBereitstellungType";
    pub(crate) const FTTX_KUE: &str = "FTTXKuendigungType";
    pub(crate) const FTTX_PV: &str = "FTTXProviderwechselType";
    pub(crate) const POS_QEB: &str = "MeldungspositionsattributeQEBType";
    pub(crate) const POS_ABM: &str = "MeldungspositionsattributeABMType";
}

/// The `xsi:type` of a message type, e.g. `MeldungstypZWM-LEType`.
pub(crate) fn meldungstyp_type(typ: crate::Meldungstyp) -> String {
    format!("Meldungstyp{}Type", typ.as_str())
}
