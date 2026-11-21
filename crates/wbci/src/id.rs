//! Identifiers: carrier codes, the three request ids, LineID and Portierungskennung.
//!
//! Source: XSD `wbci-type-simple.xsd` (`CarrierCode`, `VorabstimmungsIdType`, `StornoIdType`,
//! `AenderungsIdType`, `LineIDType`, `PortierungskennungType`); id format A1 V5.4.1 §2.9.1.

use crate::text::{Charset, TextSpec, Ws, text_type};
#[doc(inline)]
pub use carrier_core::id::{CarrierCode, LineId, MAX_SEQUENCE, VorabstimmungsId};
use carrier_core::request_id;

const fn spec(name: &'static str, min: usize, max: usize, check: fn(&str) -> bool, expected: &'static str) -> TextSpec {
    TextSpec { name, min, max, charset: Charset::Wbci, ws: Ws::Preserve, pattern: Some((check, expected)) }
}

text_type!(
    /// A porting identifier from the BNetzA numbering plan, e.g. `D001`.
    ///
    /// XSD `PortierungskennungType`: `D[0-9A-F]{3}`.
    Portierungskennung,
    spec(
        "Portierungskennung",
        4,
        4,
        |s| s.starts_with('D') && s.bytes().skip(1).all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b)),
        "D[0-9A-F]{3}"
    ),
    plain
);

request_id!(
    /// The id of a cancellation request, e.g. `DEU.EKP1.S000000013`.
    ///
    /// XSD `StornoIdType`: `[0-9A-Z]{3}\.[0-9A-Z]{1,6}\.S[0-9A-Z]{9}`.
    StornoId,
    'S',
    r"[0-9A-Z]{3}\.[0-9A-Z]{1,6}\.S[0-9A-Z]{9}"
);
request_id!(
    /// The id of a postponement request, e.g. `DEU.EKP1.T000000013`.
    ///
    /// XSD `AenderungsIdType`: `[0-9A-Z]{3}\.[0-9A-Z]{1,6}\.T[0-9A-Z]{9}`.
    AenderungsId,
    'T',
    r"[0-9A-Z]{3}\.[0-9A-Z]{1,6}\.T[0-9A-Z]{9}"
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ValueErrorKind;

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
        assert_eq!(StornoId::allocate(&cc("DEU.EKP1"), 7).unwrap().as_str(), "DEU.EKP1.S000000007");
        assert_eq!(AenderungsId::allocate(&cc("DEU.D"), 42).unwrap().as_str(), "DEU.D.T000000042");
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
    fn line_ids_and_porting_ids() {
        assert!("DEU.EKP1.1754639821".parse::<LineId>().is_ok());
        assert!("DEU.EKP1.00000".parse::<LineId>().is_ok());
        assert!("DEU.EKP1.12345678901".parse::<LineId>().is_err());
        assert!("D001".parse::<Portierungskennung>().is_ok());
        assert!("D0AF".parse::<Portierungskennung>().is_ok());
        assert!("D0af".parse::<Portierungskennung>().is_err());
        assert!("E001".parse::<Portierungskennung>().is_err());
    }
}
