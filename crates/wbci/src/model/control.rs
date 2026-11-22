//! The envelope: control data and the four operation payloads.
//!
//! Source: XSD `wbci-envelope.xsd` (V2.00); versioning WBCI 2.0 §11.4.

pub use carrier_core::envelope::{SignaturId, Zeitstempel};

use super::{Anfrage, Meldung, Teq};

/// `majorRelease` and `minorRelease`, two digits each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Release {
    /// Major release: changes it are incompatible and change the namespace.
    pub major: u8,
    /// Minor version: compatible additions.
    pub minor: u8,
}

impl Release {
    /// WBCI 2.0: major `02`, minor `00`.
    pub const WBCI_2_0: Self = Self { major: 2, minor: 0 };
}

impl Default for Release {
    fn default() -> Self {
        Self::WBCI_2_0
    }
}

/// `Control`: sent with every request, message and receipt.
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
    /// Control data for WBCI 2.0.
    #[must_use]
    pub fn new(zeitstempel: impl Into<Zeitstempel>, signatur: SignaturId) -> Self {
        Self { zeitstempel: zeitstempel.into(), release: Release::WBCI_2_0, signatur }
    }
}

/// One of the four payloads exchanged over WBCI: the body of a SOAP call.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum Payload {
    /// `annehmenAnfrageRequest`: a request.
    AnfrageRequest {
        /// `Control`.
        control: Control,
        /// `Anfrage`.
        anfrage: Anfrage,
    },
    /// `annehmenAnfrageResponse`: the receipt for a request.
    AnfrageResponse {
        /// `Control`.
        control: Control,
        /// `Quittung`.
        quittung: Teq,
    },
    /// `annehmenMeldungRequest`: a message.
    MeldungRequest {
        /// `Control`.
        control: Control,
        /// `Meldung`.
        meldung: Meldung,
    },
    /// `annehmenMeldungResponse`: the receipt for a message.
    MeldungResponse {
        /// `Control`.
        control: Control,
        /// `Quittung`.
        quittung: Teq,
    },
}

impl Payload {
    /// What the payload is, as the specifications name it: the business case of a request
    /// (`VA-KUE-MRN`, `STR-AUF`, `TVS-VA`), the message type (`RUEM-VA`), or `TEQ`.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::AnfrageRequest { anfrage, .. } => anfrage.geschaeftsfall().as_str(),
            Self::MeldungRequest { meldung, .. } => meldung.meldungstyp().as_str(),
            Self::AnfrageResponse { .. } | Self::MeldungResponse { .. } => "TEQ",
        }
    }

    /// The control data.
    #[must_use]
    pub const fn control(&self) -> &Control {
        match self {
            Self::AnfrageRequest { control, .. }
            | Self::AnfrageResponse { control, .. }
            | Self::MeldungRequest { control, .. }
            | Self::MeldungResponse { control, .. } => control,
        }
    }
}
