//! Identifiers both WBCI and S/PRI carry: carrier codes, the Vorabstimmungs-ID and the LineID.
//!
//! Source: XSD `wbci-type-simple.xsd` (`CarrierCode`, `VorabstimmungsIdType`, `LineIDType`);
//! id format WBCI Anl. 1 V5.4.1 §2.9.1. S/PRI XSD 4.00.03 `VorabstimmungIdType` (16–21
//! characters) and `LineIdType` (7–21) restrict by length only; they denote the same WBCI id
//! and AK S&P LineID, so the stricter types here serve both (an interpretation). A valid value
//! always satisfies the S/PRI facets.

use crate::text::{Charset, TextSpec, Ws, upper_alnum};
use crate::{ValueError, ValueErrorKind, text_type};

/// A [`TextSpec`] in the WBCI character set with a shape check.
#[must_use]
pub const fn id_spec(
    name: &'static str,
    min: usize,
    max: usize,
    check: fn(&str) -> bool,
    expected: &'static str,
) -> TextSpec {
    TextSpec { name, min, max, charset: Charset::Wbci, ws: Ws::Preserve, pattern: Some((check, expected)) }
}

/// `CC.ICC` — ISO country code and ITU M.1400 carrier code.
#[must_use]
pub fn is_carrier(s: &str) -> bool {
    s.split_once('.').is_some_and(|(cc, icc)| upper_alnum(cc, 3, 3) && upper_alnum(icc, 1, 6))
}

/// `CC.ICC.<tail>` with a tail check.
pub fn carrier_and(s: &str, tail: impl Fn(&str) -> bool) -> bool {
    s.rsplit_once('.').is_some_and(|(carrier, rest)| is_carrier(carrier) && tail(rest))
}

/// `CC.ICC.<letter>[0-9A-Z]{9}`.
#[must_use]
pub fn is_request_id(s: &str, letter: char) -> bool {
    carrier_and(s, |t| t.len() == 10 && t.starts_with(letter) && upper_alnum(t.get(1..).unwrap_or(""), 9, 9))
}

text_type!(
    /// A carrier: ISO 3166 alpha-3 country code and ITU M.1400 carrier code, e.g. `DEU.EKP1`.
    ///
    /// XSD `CarrierCode`: `[0-9A-Z]{3}\.[0-9A-Z]{1,6}`.
    CarrierCode,
    id_spec("CarrierCode", 5, 10, is_carrier, r"[0-9A-Z]{3}\.[0-9A-Z]{1,6}"),
    plain
);

/// The carrier prefix of a request id that has already been validated.
#[doc(hidden)]
#[must_use]
pub fn carrier_of_valid_id(id: &str) -> CarrierCode {
    let (carrier, _) = id.rsplit_once('.').unwrap_or((id, ""));
    CarrierCode(carrier.to_owned())
}

text_type!(
    /// The line identifier of a technical resource, e.g. `DEU.EKP1.1754639821`.
    ///
    /// XSD `LineIDType` (WBCI): `[0-9A-Z]{3}\.[0-9A-Z]{1,6}\.[0-9A-Z]{1,10}`, 7–21 characters;
    /// S/PRI `LineIdType` admits any 7–21 characters (see the module docs).
    LineId,
    id_spec(
        "LineId",
        7,
        21,
        |s| carrier_and(s, |t| upper_alnum(t, 1, 10)),
        r"[0-9A-Z]{3}\.[0-9A-Z]{1,6}\.[0-9A-Z]{1,10}"
    ),
    plain
);

/// The largest sequence number a request id's `allocate` accepts: nine decimal digits.
///
/// The schema admits `[0-9A-Z]{9}`; decimal keeps allocated ids readable and matches the
/// specification's example (`DEU.EKP1.V000000013` for number 13).
pub const MAX_SEQUENCE: u64 = 999_999_999;

/// `CC.ICC.<letter><seq:09>`.
///
/// # Errors
///
/// `seq` larger than [`MAX_SEQUENCE`].
pub fn encode_request_id(
    carrier: &CarrierCode,
    letter: char,
    seq: u64,
    name: &'static str,
) -> Result<String, ValueError> {
    if seq > MAX_SEQUENCE {
        return Err(ValueError { type_name: name, kind: ValueErrorKind::Sequence { max: MAX_SEQUENCE } });
    }
    Ok(format!("{carrier}.{letter}{seq:09}"))
}

/// Declares a WBCI request id `CC.ICC.<letter>[0-9A-Z]{9}` with `allocate` and `carrier`.
#[macro_export]
macro_rules! request_id {
    ($(#[$meta:meta])* $name:ident, $letter:literal, $expected:literal) => {
        $crate::text_type!(
            $(#[$meta])*
            $name,
            $crate::id::id_spec(stringify!($name), 16, 21, |s| $crate::id::is_request_id(s, $letter), $expected),
            plain
        );

        impl $name {
            /// Allocates the id for `seq` under `carrier`: the sequence is written as nine
            /// decimal digits, zero-padded.
            ///
            /// The caller owns the sequence and must never reuse a number (A1 V5.4.1 §2.9.1:
            /// „stellt sicher, dass die ID eindeutig ist").
            ///
            /// # Errors
            ///
            /// `seq` larger than `MAX_SEQUENCE` (999 999 999).
            pub fn allocate(carrier: &$crate::CarrierCode, seq: u64) -> Result<Self, $crate::ValueError> {
                $crate::id::encode_request_id(carrier, $letter, seq, stringify!($name)).map(Self)
            }

            /// The carrier that issued this id.
            #[must_use]
            pub fn carrier(&self) -> $crate::CarrierCode {
                $crate::id::carrier_of_valid_id(&self.0)
            }
        }
    };
}

request_id!(
    /// The id of a pre-agreement request, e.g. `DEU.EKP1.V000000013`.
    ///
    /// XSD `VorabstimmungsIdType` (WBCI): `[0-9A-Z]{3}\.[0-9A-Z]{1,6}\.V[0-9A-Z]{9}`. S/PRI
    /// carries it as `vorabstimmungId` (`VorabstimmungIdType`, 16–21 characters).
    VorabstimmungsId,
    'V',
    r"[0-9A-Z]{3}\.[0-9A-Z]{1,6}\.V[0-9A-Z]{9}"
);

#[cfg(test)]
mod tests {
    use super::*;

    fn cc(s: &str) -> CarrierCode {
        s.parse().unwrap()
    }

    #[test]
    fn carrier_codes() {
        for ok in ["DEU.EKP1", "DEU.D", "DEU.DTAG12"] {
            assert!(ok.parse::<CarrierCode>().is_ok(), "{ok}");
        }
        for bad in ["DE.EKP1", "DEU.ekp1", "DEU.", "DEU.EKP1234", "DEUEKP1", "DEU.EK P"] {
            assert!(bad.parse::<CarrierCode>().is_err(), "{bad}");
        }
    }

    #[test]
    fn allocation_matches_the_schema_example() {
        let id = VorabstimmungsId::allocate(&cc("DEU.EKP1"), 13).unwrap();
        assert_eq!(id.as_str(), "DEU.EKP1.V000000013");
        assert_eq!(id.carrier(), cc("DEU.EKP1"));
        assert_eq!(VorabstimmungsId::allocate(&cc("DEU.EKP1"), MAX_SEQUENCE).unwrap().as_str(), "DEU.EKP1.V999999999");
        let err = VorabstimmungsId::allocate(&cc("DEU.EKP1"), MAX_SEQUENCE + 1).unwrap_err();
        assert_eq!(err.kind, ValueErrorKind::Sequence { max: MAX_SEQUENCE });
    }

    #[test]
    fn request_ids_check_their_letter() {
        assert!("DEU.EKP1.V000000013".parse::<VorabstimmungsId>().is_ok());
        assert!("DEU.EKP1.S000000013".parse::<VorabstimmungsId>().is_err());
        assert!("DEU.EKP1.V00000013".parse::<VorabstimmungsId>().is_err());
        assert!("DEU.EKP1.v000000013".parse::<VorabstimmungsId>().is_err());
    }

    #[test]
    fn line_ids() {
        assert!("DEU.EKP1.1754639821".parse::<LineId>().is_ok());
        assert!("DEU.EKP1.00000".parse::<LineId>().is_ok());
        assert!("DEU.EKP1.12345678901".parse::<LineId>().is_err());
    }

    #[test]
    fn shared_ids_satisfy_the_spri_length_facets() {
        // S/PRI VorabstimmungIdType 16–21, LineIdType 7–21.
        let shortest = VorabstimmungsId::new("DEU.D.V000000001").unwrap();
        assert_eq!(shortest.as_str().len(), 16);
        const { assert!(VorabstimmungsId::SPEC.min >= 16 && VorabstimmungsId::SPEC.max <= 21) };
        const { assert!(LineId::SPEC.min >= 7 && LineId::SPEC.max <= 21) };
    }
}
