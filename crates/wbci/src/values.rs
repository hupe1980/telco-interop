//! Constrained values: numbers, names, address parts, free text and the small enumerations.
//!
//! Source: XSD `wbci-type-simple.xsd` and `wbci-type-enum.xsd` (V2.00). Values that may be
//! personal data redact themselves in `Debug`.

use crate::text::{Charset, TextSpec, Ws, digits, text_type};

const fn digits_spec(
    name: &'static str,
    min: usize,
    max: usize,
    check: fn(&str) -> bool,
    expected: &'static str,
) -> TextSpec {
    TextSpec { name, min, max, charset: Charset::Wbci, ws: Ws::Preserve, pattern: Some((check, expected)) }
}

const fn wbci(name: &'static str, min: usize, max: usize, ws: Ws) -> TextSpec {
    TextSpec { name, min, max, charset: Charset::Wbci, ws, pattern: None }
}

text_type!(
    /// Ortsnetzkennzahl (area code) without the leading `0`, e.g. `30` for Berlin.
    ///
    /// XSD `OrtsnetzkennzahlType`: `[1-9]\d{1,4}`.
    Onkz,
    digits_spec("Onkz", 2, 5, |s| digits(s, 2, 5) && !s.starts_with('0'), r"[1-9]\d{1,4}"),
    plain
);

text_type!(
    /// A subscriber number without the area code. XSD `RufnummerType`: `\d{1,14}`.
    Rufnummer,
    digits_spec("Rufnummer", 1, 14, |s| digits(s, 1, 14), r"\d{1,14}"),
    redact
);

text_type!(
    /// The direct-dial number of a PBX line. XSD `DurchwahlnummerType`: `\d{1,8}`.
    Durchwahlnummer,
    digits_spec("Durchwahlnummer", 1, 8, |s| digits(s, 1, 8), r"\d{1,8}"),
    redact
);

text_type!(
    /// The operator position of a PBX line, e.g. `0`. XSD `AbfragestelleType`: `\d{1,6}`.
    Abfragestelle,
    digits_spec("Abfragestelle", 1, 6, |s| digits(s, 1, 6), r"\d{1,6}"),
    plain
);

text_type!(
    /// A bound of a number block of a PBX line. XSD `RufnummernBlockType`: `\d{1,6}`.
    RufnummernBlock,
    digits_spec("RufnummernBlock", 1, 6, |s| digits(s, 1, 6), r"\d{1,6}"),
    plain
);

text_type!(
    /// A German postcode. XSD `PostleitzahlType`: `\d{5}`.
    Postleitzahl,
    digits_spec("Postleitzahl", 5, 5, |s| digits(s, 5, 5), r"\d{5}"),
    plain
);

text_type!(
    /// A house number. XSD `HausnummerType`: `\d{1,4}`.
    Hausnummer,
    digits_spec("Hausnummer", 1, 4, |s| digits(s, 1, 4), r"\d{1,4}"),
    redact
);

text_type!(
    /// A WITA contract number. XSD `VertragsnummerType`: 1–10 characters.
    Vertragsnummer,
    TextSpec { name: "Vertragsnummer", min: 1, max: 10, charset: Charset::Xml, ws: Ws::Preserve, pattern: None },
    plain
);

text_type!(
    /// A technology from Anlage 5, e.g. `001 TAL ISDN`. XSD `TechnologieType`: `[0-9]{3} .*`, 1–25.
    Technologie,
    TextSpec {
        name: "Technologie",
        min: 1,
        max: 25,
        charset: Charset::Xml,
        ws: Ws::Preserve,
        pattern: Some((|s| s.get(..3).is_some_and(|d| digits(d, 3, 3)) && s.get(3..4) == Some(" "), "[0-9]{3} .*")),
    },
    plain
);

text_type!(
    /// A first name. XSD `VornameType`: WBCI charset, 1–30, whitespace collapsed.
    Vorname,
    wbci("Vorname", 1, 30, Ws::Collapse),
    redact
);
text_type!(
    /// A surname. XSD `NachnameType`: WBCI charset, 1–30, whitespace collapsed.
    Nachname,
    wbci("Nachname", 1, 30, Ws::Collapse),
    redact
);
text_type!(
    /// A company name (`firmenname`, typed `VornameType` in the schema): 1–30, collapsed.
    Firmenname,
    wbci("Firmenname", 1, 30, Ws::Collapse),
    redact
);
text_type!(
    /// A street name. XSD `StrassennameType`: WBCI charset, 1–40, collapsed.
    Strassenname,
    wbci("Strassenname", 1, 40, Ws::Collapse),
    redact
);
text_type!(
    /// A town. XSD `OrtsnameType`: WBCI charset, 1–40, collapsed.
    Ortsname,
    wbci("Ortsname", 1, 40, Ws::Collapse),
    plain
);
text_type!(
    /// A house-number suffix, e.g. `a`. XSD `HausnummernZusatzType`: 1–6, collapsed.
    HausnummernZusatz,
    wbci("HausnummernZusatz", 1, 6, Ws::Collapse),
    redact
);
text_type!(
    /// The losing provider's own reference for the order. XSD `IdentifiziererType`: 1–20.
    Identifizierer,
    wbci("Identifizierer", 1, 20, Ws::Preserve),
    plain
);
text_type!(
    /// A project. XSD `ProjektkennerType`: WBCI charset, 1–30.
    Projektkenner,
    wbci("Projektkenner", 1, 30, Ws::Preserve),
    plain
);
text_type!(
    /// Couples orders within a project. XSD `KopplungskennerType`: WBCI charset, 1–30.
    Kopplungskenner,
    wbci("Kopplungskenner", 1, 30, Ws::Preserve),
    plain
);
text_type!(
    /// Free text such as a reason, 1–255 characters, line breaks allowed.
    ///
    /// XSD `Freitext255Type`.
    Freitext,
    TextSpec { name: "Freitext", min: 1, max: 255, charset: Charset::WbciMultiline, ws: Ws::Preserve, pattern: None },
    redact
);
text_type!(
    /// The text of a message position. XSD `MeldungstextType`: 1–255, collapsed.
    Meldungstext,
    wbci("Meldungstext", 1, 255, Ws::Collapse),
    plain
);
pub(crate) use carrier_core::code_enum;

impl Meldungstext {
    /// `-`, the fallback text for a code without an official one.
    pub(crate) fn dash() -> Self {
        Self("-".to_owned())
    }
}

code_enum!(
    /// Salutation of a person. XSD `AnredePersonEnumType`.
    Anrede {
        /// `1` — Herr.
        Herr = "1",
        /// `2` — Frau.
        Frau = "2",
        /// `9` — keine Anrede.
        Keine = "9",
    }
);

code_enum!(
    /// Salutation of a company. XSD `AnredeFirmaEnumType`.
    AnredeFirma {
        /// `4` — Firma.
        Firma = "4",
        /// `9` — keine Anrede.
        Keine = "9",
    }
);

code_enum!(
    /// The time window in which numbers are ported on the switch date (A1 V5.4.1 §3.1).
    ///
    /// XSD `PortierungszeitfensterEnumType`.
    Portierungszeitfenster {
        /// `ZF1` — 6 to 8 o'clock, porting without line takeover.
        Zf1 = "ZF1",
        /// `ZF2` — 6 to 12 o'clock, the standard window.
        Zf2 = "ZF2",
        /// `ZF3` — freely chosen; only if agreed bilaterally beforehand.
        Zf3 = "ZF3",
    }
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert!("30".parse::<Onkz>().is_ok());
        assert!("030".parse::<Onkz>().is_err());
        assert!("3".parse::<Onkz>().is_err());
        assert!("12345678901234".parse::<Rufnummer>().is_ok());
        assert!("123456789012345".parse::<Rufnummer>().is_err());
        assert!("12a".parse::<Rufnummer>().is_err());
        assert!("10115".parse::<Postleitzahl>().is_ok());
        assert!("1011".parse::<Postleitzahl>().is_err());
        assert!("12345".parse::<Hausnummer>().is_err());
    }

    #[test]
    fn technologie_needs_three_digits_and_a_space() {
        assert!("001 TAL ISDN".parse::<Technologie>().is_ok());
        assert!("01 TAL".parse::<Technologie>().is_err());
        assert!("001TAL".parse::<Technologie>().is_err());
    }

    // wbci-switch/SR-005: personal data does not appear in Debug output.
    #[test]
    fn personal_data_is_redacted_in_debug() {
        let name = Nachname::new("Mustermann").unwrap();
        assert_eq!(format!("{name:?}"), "Nachname(<redacted, 10 chars>)");
        assert_eq!(name.to_string(), "Mustermann");
        let nr = Rufnummer::new("1234567").unwrap();
        assert!(!format!("{nr:?}").contains("1234567"));
        assert_eq!(format!("{:?}", Ortsname::new("Berlin").unwrap()), "Ortsname(\"Berlin\")");
    }

    #[test]
    fn enums_round_trip_their_codes() {
        assert_eq!(Anrede::from_code("2"), Some(Anrede::Frau));
        assert_eq!(AnredeFirma::Firma.as_str(), "4");
        assert_eq!(Portierungszeitfenster::from_code("ZF2"), Some(Portierungszeitfenster::Zf2));
        assert_eq!(Portierungszeitfenster::from_code("ZF4"), None);
    }
}
