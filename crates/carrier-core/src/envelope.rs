//! Envelope values every payload carries: the send timestamp and the signing certificate.
//!
//! Sources: XSD `wbci-envelope.xsd` (V2.00) and S/PRI `oss-envelope.xsd` (4.00.03): both
//! define `zeitstempel` as `xsd:dateTime` and `signaturId` as `issuer` + `serial`.

use core::fmt;

use jiff::Timestamp;
use jiff::tz::Offset;

use crate::text::{Charset, TextSpec, Ws};
use crate::text_type;

/// `zeitstempel`: when the message was sent, with the sender's UTC offset.
///
/// The schemas forbid a timestamp without a time zone. The offset is kept so a message reads
/// back to the same bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Zeitstempel {
    /// The instant.
    pub timestamp: Timestamp,
    /// The offset it was written with.
    pub offset: Offset,
}

impl Zeitstempel {
    /// A timestamp written with the German offset in force at that instant (+01:00 or +02:00).
    #[must_use]
    pub fn berlin(timestamp: Timestamp) -> Self {
        Self { timestamp, offset: crate::calendar::berlin().to_offset(timestamp) }
    }

    /// A timestamp written in UTC (`+00:00`; `Z` is read as the same offset).
    #[must_use]
    pub const fn utc(timestamp: Timestamp) -> Self {
        Self { timestamp, offset: Offset::UTC }
    }
}

impl From<Timestamp> for Zeitstempel {
    fn from(timestamp: Timestamp) -> Self {
        Self::berlin(timestamp)
    }
}

impl fmt::Display for Zeitstempel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.timestamp.display_with_offset(self.offset))
    }
}

text_type!(
    /// Issuer or serial of the signing certificate. XSD `SignatureIssuerType`/`SignatureSerialIdType`.
    ///
    /// WBCI restricts both to its character set; S/PRI admits any string. One type serves both,
    /// so the WBCI set applies to S/PRI too (an interpretation: certificate issuers and serials
    /// stay within it).
    SignaturText,
    TextSpec { name: "SignaturText", min: 0, max: usize::MAX, charset: Charset::Wbci, ws: Ws::Replace, pattern: None },
    plain
);

/// `signaturId`: identifies the sender's signing certificate (WBCI 2.0 §10; S/PRI
/// `SignaturIdType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SignaturId {
    /// `issuer`: the certificate issuer.
    pub issuer: SignaturText,
    /// `serial`: the certificate serial number, unique per issuer.
    pub serial: SignaturText,
}
