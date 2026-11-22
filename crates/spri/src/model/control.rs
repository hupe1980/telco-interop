//! The envelope: control data and the four operation payloads.
//!
//! Source: XSD 4.00.03 `oss-envelope.xsd` (`WholesaleMessageType`, the four operation
//! elements); versioning S/PRI 4.4 §10.4; operations `spri.wsdl`.

pub use carrier_core::envelope::{SignaturId, Zeitstempel};

use super::{Auftrag, Meldung, Teq};

/// `majorRelease` and `minorRelease` (S/PRI 4.4 §10.4: „AN 2", digits only).
///
/// The major release is the one the namespaces name (`…/oss/v4/…`); this library implements 4
/// and writes both numbers with two digits (`04`), an interpretation of „AN 2". It reads one or
/// two digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Release {
    /// Major release: changes it are incompatible and change the namespace.
    pub major: u8,
    /// Minor version: compatible additions, ignored by a receiver that does not use them.
    pub minor: u8,
}

impl Release {
    /// S/PRI 4.4: major `04`, minor `04`.
    pub const SPRI_4_4: Self = Self { major: 4, minor: 4 };
}

impl Default for Release {
    fn default() -> Self {
        Self::SPRI_4_4
    }
}

/// `control` (`WholesaleRequestType`, `WholesaleResponseType`): sent with every payload.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Control {
    /// `zeitstempel`.
    pub zeitstempel: Zeitstempel,
    /// `majorRelease`, `minorRelease`.
    pub release: Release,
    /// `signaturId`.
    pub signatur: SignaturId,
}

impl Control {
    /// Control data for S/PRI 4.4.
    #[must_use]
    pub fn new(zeitstempel: impl Into<Zeitstempel>, signatur: SignaturId) -> Self {
        Self { zeitstempel: zeitstempel.into(), release: Release::SPRI_4_4, signatur }
    }
}

/// One of the four payloads exchanged over S/PRI: the body of a SOAP call (`spri.wsdl`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum Payload {
    /// `annehmenAuftragRequest`: an order, from the service provider to the network owner.
    AuftragRequest {
        /// `control`.
        control: Control,
        /// `auftrag`.
        auftrag: Auftrag,
    },
    /// `annehmenAuftragResponse`: the receipt for an order.
    AuftragResponse {
        /// `control`.
        control: Control,
        /// `quittung`.
        quittung: Teq,
    },
    /// `annehmenMeldungRequest`: a message, in either direction.
    MeldungRequest {
        /// `control`.
        control: Control,
        /// `meldung`.
        meldung: Meldung,
    },
    /// `annehmenMeldungResponse`: the receipt for a message.
    MeldungResponse {
        /// `control`.
        control: Control,
        /// `quittung`.
        quittung: Teq,
    },
}

impl Payload {
    /// What the payload is, as the specifications name it: the business case of a standard
    /// order (`PV`), `Storno` or `Terminverschiebung`, the message type (`ABM`), or `TEQ`.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::AuftragRequest { auftrag, .. } => match auftrag.aenderungskennzeichen {
                crate::Aenderungskennzeichen::Standard => auftrag.geschaeftsfall.code().as_str(),
                crate::Aenderungskennzeichen::Storno => "Storno",
                crate::Aenderungskennzeichen::Terminverschiebung => "Terminverschiebung",
            },
            Self::MeldungRequest { meldung, .. } => meldung.typ().as_str(),
            Self::AuftragResponse { .. } | Self::MeldungResponse { .. } => "TEQ",
        }
    }

    /// The control data.
    #[must_use]
    pub const fn control(&self) -> &Control {
        match self {
            Self::AuftragRequest { control, .. }
            | Self::AuftragResponse { control, .. }
            | Self::MeldungRequest { control, .. }
            | Self::MeldungResponse { control, .. } => control,
        }
    }

    /// The SOAP action of the operation (`spri.wsdl`), e.g.
    /// `http://spri.telekom.de/oss/v4/spri/annehmenAuftrag`.
    #[must_use]
    pub const fn soap_action(&self) -> &'static str {
        match self {
            Self::AuftragRequest { .. } | Self::AuftragResponse { .. } => {
                "http://spri.telekom.de/oss/v4/spri/annehmenAuftrag"
            }
            Self::MeldungRequest { .. } | Self::MeldungResponse { .. } => {
                "http://spri.telekom.de/oss/v4/spri/annehmenMeldung"
            }
        }
    }
}
