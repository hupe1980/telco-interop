//! Constrained values: numbers, names, address parts, free text and the enumerations.
//!
//! Source: XSD 4.00.03 `oss-type-simple.xsd`, `oss-type-enum.xsd` and the anonymous
//! restrictions of `oss-type-complex.xsd`, `oss-order.xsd` and `oss-message.xsd`. S/PRI
//! restricts `xsd:string` by length and, for a few types, by pattern; the characters are XML
//! characters without controls ([`carrier_core::text::Charset::Xml`]). Values that may be
//! personal data redact themselves in `Debug`.

use carrier_core::text::{Charset, TextSpec, Ws, digits};
use carrier_core::{code_enum, text_type};

const fn xml(name: &'static str, min: usize, max: usize) -> TextSpec {
    TextSpec::xml(name, min, max)
}

const fn collapsed(name: &'static str, min: usize, max: usize) -> TextSpec {
    TextSpec { name, min, max, charset: Charset::Xml, ws: Ws::Collapse, pattern: None }
}

const fn digit_spec(
    name: &'static str,
    n: usize,
    m: usize,
    check: fn(&str) -> bool,
    expected: &'static str,
) -> TextSpec {
    TextSpec { name, min: n, max: m, charset: Charset::Xml, ws: Ws::Preserve, pattern: Some((check, expected)) }
}

text_type!(
    /// The service provider's customer number at the network owner. XSD `AuftraggebernummerType`: 1–10.
    Auftraggebernummer,
    xml("Auftraggebernummer", 1, 10),
    plain
);
text_type!(
    /// The framework contract the order is placed under. XSD `LeistungsnummerType`: 1–10.
    Leistungsnummer,
    xml("Leistungsnummer", 1, 10),
    plain
);
text_type!(
    /// The network owner's contract number for a line. XSD `VertragsnummerType`: 1–10.
    Vertragsnummer,
    xml("Vertragsnummer", 1, 10),
    plain
);
text_type!(
    /// The service provider's order number. XSD `ExterneAuftragsnummerType`: 1–20.
    ExterneAuftragsnummer,
    xml("ExterneAuftragsnummer", 1, 20),
    plain
);
text_type!(
    /// A product from the product catalogue (S/PRI Anlage 4). XSD `ProduktbezeichnerType`: 1–100.
    Produktbezeichner,
    xml("Produktbezeichner", 1, 100),
    plain
);
text_type!(
    /// The text of a message position. XSD `MeldungstextType`: at most 255 characters.
    Meldungstext,
    xml("Meldungstext", 0, 255),
    plain
);
text_type!(
    /// A phone, mobile or fax number, e.g. `030 1234567` or `+49 30 1234567`.
    ///
    /// XSD `TelefonnummerType`: `((\+\d{2} |0)\d{2,5} )?\d{1,14}`, at most 30, collapsed.
    Telefonnummer,
    TextSpec {
        name: "Telefonnummer",
        min: 1,
        max: 30,
        charset: Charset::Xml,
        ws: Ws::Collapse,
        pattern: Some((is_telefonnummer, r"((\+\d{2} |0)\d{2,5} )?\d{1,14}")),
    },
    redact
);
text_type!(
    /// An e-mail address. XSD `EmailadresseType`: `.+@(.+\.)+\w+`, at most 320, collapsed.
    Emailadresse,
    TextSpec {
        name: "Emailadresse",
        min: 1,
        max: 320,
        charset: Charset::Xml,
        ws: Ws::Collapse,
        pattern: Some((is_email, r".+@(.+\.)+\w+")),
    },
    redact
);
text_type!(
    /// A first name. XSD `VornameType`: 1–30.
    Vorname,
    xml("Vorname", 1, 30),
    redact
);
text_type!(
    /// A surname, or the second part of a company name. XSD `NachnameType`: 1–30.
    Nachname,
    xml("Nachname", 1, 30),
    redact
);
text_type!(
    /// A company name (`firmenname`, typed `VornameType` in the schema): 1–30.
    Firmenname,
    xml("Firmenname", 1, 30),
    redact
);
text_type!(
    /// A street name. XSD `StrassennameType`: 1–40.
    Strassenname,
    xml("Strassenname", 1, 40),
    redact
);
text_type!(
    /// A house number. XSD `HausnummerType`: `\d{1,4}`.
    Hausnummer,
    digit_spec("Hausnummer", 1, 4, |s| digits(s, 1, 4), r"\d{1,4}"),
    redact
);
text_type!(
    /// A house-number suffix, e.g. `a`. XSD `HausnummernZusatzType`: 1–6.
    HausnummernZusatz,
    xml("HausnummernZusatz", 1, 6),
    redact
);
text_type!(
    /// A town. XSD `OrtsnameType`: 1–40.
    Ortsname,
    xml("Ortsname", 1, 40),
    plain
);
text_type!(
    /// A district of a town. XSD `OrtsteilType`: 1–40.
    Ortsteil,
    xml("Ortsteil", 1, 40),
    plain
);
text_type!(
    /// A part of a building, e.g. `Haus B`. XSD `GebaeudeteilNameType` and `GebaeudeteilZusatzType`: 1–25.
    Gebaeudeteil,
    xml("Gebaeudeteil", 1, 25),
    plain
);
text_type!(
    /// A country code, `de` by default. XSD `LandType`: exactly 2, collapsed.
    Land,
    collapsed("Land", 2, 2),
    plain
);
text_type!(
    /// A postcode. XSD `PostleitzahlType`: 1–6, collapsed.
    Postleitzahl,
    collapsed("Postleitzahl", 1, 6),
    plain
);
text_type!(
    /// The serial number of an ONT (or a source MAC address). XSD `SeriennummerONTType`: 1–20, collapsed.
    SeriennummerOnt,
    collapsed("SeriennummerOnt", 1, 20),
    plain
);
text_type!(
    /// A provider's name in a provider switch. XSD `String60ZeichenType`: 1–60, collapsed.
    Providername,
    collapsed("Providername", 1, 60),
    plain
);
text_type!(
    /// A fault ticket number (`stoerungsNrAuftraggeber`, `stoerungsNrLeistungserbringer`): 1–30.
    StoerungsNr,
    xml("StoerungsNr", 1, 30),
    plain
);
text_type!(
    /// Free text of up to 1 024 characters: fault description, complaint, reason, note, message.
    ///
    /// XSD anonymous restrictions `minLength 1`, `maxLength 1024` (e.g. `stoerungsbeschreibung`,
    /// `nachricht`, `verzoegerungsgrund`). It may describe the end customer, so it is redacted.
    Freitext,
    xml("Freitext", 1, 1024),
    redact
);
text_type!(
    /// The file name of an attachment as sent: 1–255. Use [`crate::Anlage::safe_file_name`]
    /// before joining it to a folder.
    Dateiname,
    xml("Dateiname", 1, 255),
    redact
);
text_type!(
    /// The description of an attachment: 1–160.
    Beschreibung,
    xml("Beschreibung", 1, 160),
    redact
);
text_type!(
    /// A project (`projektkenner`) or coupling (`kopplungskenner`) identifier: 1–30.
    Projektkenner,
    xml("Projektkenner", 1, 30),
    plain
);
text_type!(
    /// The A10 network service provider of a line (`a10nsp`): 1–20.
    A10nsp,
    xml("A10nsp", 1, 20),
    plain
);
text_type!(
    /// The port of a line (`port`): 1–10.
    Port,
    xml("Port", 1, 10),
    plain
);
text_type!(
    /// An S-VLAN or C-VLAN id (`s-vlan`, `c-vlan`): 1–4.
    Vlan,
    xml("Vlan", 1, 4),
    plain
);
text_type!(
    /// A WITA contract number (`witaVertragsnummer`): 1–10.
    WitaVertragsnummer,
    xml("WitaVertragsnummer", 1, 10),
    plain
);
text_type!(
    /// The access technology (`technologie`): 1–25.
    Technologie,
    xml("Technologie", 1, 25),
    plain
);
text_type!(
    /// The number of a Home-ID (`homeIdNummer`): 1–15.
    HomeId,
    xml("HomeId", 1, 15),
    plain
);
text_type!(
    /// A parcel carrier (`unternehmen`): 1–10.
    Versandunternehmen,
    xml("Versandunternehmen", 1, 10),
    plain
);
text_type!(
    /// A parcel tracking number (`sendungsnummer`): 1–40.
    Sendungsnummer,
    xml("Sendungsnummer", 1, 40),
    plain
);
text_type!(
    /// The losing provider's answer code in a provider switch (`antwortcode`): 1–10.
    Antwortcode,
    xml("Antwortcode", 1, 10),
    plain
);
text_type!(
    /// The losing provider's answer text (`antworttext`): at most 255.
    Antworttext,
    xml("Antworttext", 0, 255),
    plain
);
text_type!(
    /// A note for the installer (`montagehinweis`): 1–100.
    Montagehinweis,
    xml("Montagehinweis", 1, 100),
    redact
);
text_type!(
    /// Where the TAE or ONT is (`lageTAE_ONT`): at most 60.
    LageTaeOnt,
    xml("LageTaeOnt", 0, 60),
    plain
);
text_type!(
    /// A diagnostic operation (`operation`) or result name (`name`): 1–20.
    Diagnosename,
    xml("Diagnosename", 1, 20),
    plain
);
text_type!(
    /// A diagnostic result value (`wert`): 1–1 024.
    Diagnosewert,
    xml("Diagnosewert", 1, 1024),
    plain
);
text_type!(
    /// The WITA code of a message position (`witaMeldungscode`): 1–10.
    WitaMeldungscode,
    xml("WitaMeldungscode", 1, 10),
    plain
);
text_type!(
    /// The WITA text of a message position (`witaMeldungstext`): at most 255.
    WitaMeldungstext,
    xml("WitaMeldungstext", 0, 255),
    plain
);
text_type!(
    /// The porting identifier of a number porting (`portierungskenner`, `xsd:string`).
    Portierungskenner,
    xml("Portierungskenner", 0, usize::MAX),
    plain
);
text_type!(
    /// An area code (`onkz`): `\d{2,5}`.
    Onkz,
    digit_spec("Onkz", 2, 5, |s| digits(s, 2, 5), r"\d{2,5}"),
    plain
);
text_type!(
    /// A subscriber number (`rufnummer`): `\d{1,14}`.
    Rufnummer,
    digit_spec("Rufnummer", 1, 14, |s| digits(s, 1, 14), r"\d{1,14}"),
    redact
);
text_type!(
    /// The direct-dial number of a PBX line (`durchwahlnummer`): `\d{1,8}`.
    Durchwahlnummer,
    digit_spec("Durchwahlnummer", 1, 8, |s| digits(s, 1, 8), r"\d{1,8}"),
    redact
);
text_type!(
    /// The operator position of a PBX line (`abfragestelle`): `\d{1,6}`.
    Abfragestelle,
    digit_spec("Abfragestelle", 1, 6, |s| digits(s, 1, 6), r"\d{1,6}"),
    plain
);
text_type!(
    /// A bound of a number block (`rnrBlockVon`, `rnrBlockBis`): `\d{1,6}`.
    RufnummernBlock,
    digit_spec("RufnummernBlock", 1, 6, |s| digits(s, 1, 6), r"\d{1,6}"),
    plain
);

impl Meldungstext {
    /// The empty text (the schema sets no minimum length).
    pub(crate) fn empty() -> Self {
        Self(String::new())
    }
}

fn is_telefonnummer(s: &str) -> bool {
    // ((\+\d{2} |0)\d{2,5} )?\d{1,14}
    let all_digits = |t: &str, n, m| digits(t, n, m);
    match s.rsplit_once(' ') {
        None => all_digits(s, 1, 14),
        Some((prefix, number)) => {
            if !all_digits(number, 1, 14) {
                return false;
            }
            if let Some(rest) = prefix.strip_prefix('+') {
                rest.split_once(' ').is_some_and(|(cc, area)| all_digits(cc, 2, 2) && all_digits(area, 2, 5))
            } else {
                prefix.strip_prefix('0').is_some_and(|area| all_digits(area, 2, 5))
            }
        }
    }
}

fn is_email(s: &str) -> bool {
    // .+@(.+\.)+\w+ — the last '@' must be followed by at least one dot with text on both sides
    // and a final run of word characters (checked as letters and digits, a subset of XSD `\w`, which excludes punctuation such as `_`); XSD `.` excludes line breaks.
    if s.contains(['\n', '\r']) {
        return false;
    }
    s.char_indices().filter(|(_, c)| *c == '@').any(|(at, _)| {
        let (local, domain) = (s.get(..at).unwrap_or(""), s.get(at + 1..).unwrap_or(""));
        !local.is_empty()
            && domain.rsplit_once('.').is_some_and(|(head, tld)| {
                !head.is_empty() && !tld.is_empty() && tld.chars().all(char::is_alphanumeric)
            })
    })
}

/// A time window for an appointment. XSD `ZeitfensterType`: the 125 values `1`–`99` and
/// `A`–`Z`. Only `1` (working days 8–14 h), `2` (working days 14–20 h) and `3` (Saturdays
/// 8–16 h) are assigned; the others are reserved („Nicht belegt").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Zeitfenster(ZeitfensterRepr);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum ZeitfensterRepr {
    Number(u8),
    Letter(u8),
}

impl Zeitfenster {
    /// `1` — working days 08:00–14:00.
    pub const WERKTAGS_VORMITTAG: Self = Self(ZeitfensterRepr::Number(1));
    /// `2` — working days 14:00–20:00.
    pub const WERKTAGS_NACHMITTAG: Self = Self(ZeitfensterRepr::Number(2));
    /// `3` — Saturdays 08:00–16:00.
    pub const SAMSTAGS: Self = Self(ZeitfensterRepr::Number(3));

    /// Parses a value of the enumeration.
    ///
    /// # Errors
    ///
    /// [`crate::ValueError`] for anything but `1`–`99` (no leading zero) and `A`–`Z`.
    pub fn new(s: &str) -> Result<Self, crate::ValueError> {
        let err = || crate::ValueError {
            type_name: "Zeitfenster",
            kind: crate::ValueErrorKind::Pattern { expected: "1–99 or A–Z" },
        };
        match s.as_bytes() {
            [c @ b'A'..=b'Z'] => Ok(Self(ZeitfensterRepr::Letter(*c))),
            [b'1'..=b'9'] | [b'1'..=b'9', b'0'..=b'9'] => {
                s.parse().map(|n| Self(ZeitfensterRepr::Number(n))).map_err(|_| err())
            }
            _ => Err(err()),
        }
    }

    /// Whether the window is assigned (`1`, `2`, `3`) rather than reserved.
    #[must_use]
    pub const fn is_assigned(self) -> bool {
        matches!(self.0, ZeitfensterRepr::Number(1..=3))
    }
}

impl core::str::FromStr for Zeitfenster {
    type Err = crate::ValueError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl core::fmt::Display for Zeitfenster {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.0 {
            ZeitfensterRepr::Number(n) => write!(f, "{n}"),
            ZeitfensterRepr::Letter(c) => write!(f, "{}", char::from(c)),
        }
    }
}

code_enum!(
    /// The kind of business case of an order. XSD `GeschaeftsfallArtType`.
    GeschaeftsfallArt {
        /// `Bereitstellung` — provisioning (NEU, PV).
        Bereitstellung = "Bereitstellung",
        /// `Kuendigung` — cancellation (KUE-AG).
        Kuendigung = "Kuendigung",
        /// `Aenderung` — change (LAE, AEN-LMAE).
        Aenderung = "Aenderung",
        /// `Entstoerung` — fault repair (EST).
        Entstoerung = "Entstoerung",
        /// `Endkundenanbieterwechsel` — provider switch (PV).
        Endkundenanbieterwechsel = "Endkundenanbieterwechsel",
        /// `Diagnose` — diagnostics (GET, SET).
        Diagnose = "Diagnose",
    }
);

code_enum!(
    /// Marks an order as standard, cancellation or postponement. XSD `AenderungskennzeichenType`.
    Aenderungskennzeichen {
        /// `Standard` — a standard order.
        Standard = "Standard",
        /// `Storno` — cancels the order with the same order number (S/PRI 4.4 §5.1.3).
        Storno = "Storno",
        /// `Terminverschiebung` — moves the requested date (S/PRI 4.4 §5.1.4).
        Terminverschiebung = "Terminverschiebung",
    }
);

code_enum!(
    /// The role of a contact person. XSD `AnsprechpartnerRolleType`.
    AnsprechpartnerRolle {
        /// `Auftragsmanagement` — order management.
        Auftragsmanagement = "Auftragsmanagement",
        /// `Technik` — technical contact.
        Technik = "Technik",
        /// `Endkunde` — the end customer, e.g. for fault repair.
        Endkunde = "Endkunde",
    }
);

code_enum!(
    /// What an attachment is. XSD `AnlagentypType`.
    Anlagentyp {
        /// `Kuendigungsschreiben` — a letter of cancellation.
        Kuendigungsschreiben = "Kuendigungsschreiben",
        /// `Lageplan` — a site plan.
        Lageplan = "Lageplan",
        /// `Kundenauftrag` — the customer's order.
        Kundenauftrag = "Kundenauftrag",
        /// `Portierungsanzeige` — a porting notice.
        Portierungsanzeige = "Portierungsanzeige",
        /// `Rechnung` — an invoice.
        Rechnung = "Rechnung",
        /// `Sonstige` — anything else.
        Sonstige = "Sonstige",
    }
);

code_enum!(
    /// The MIME type of an attachment. XSD `DokumenttypType`.
    Dokumenttyp {
        /// `image/tiff`.
        Tiff = "image/tiff",
        /// `image/jpeg`.
        Jpeg = "image/jpeg",
        /// `application/msword`.
        Msword = "application/msword",
        /// `application/excel`.
        Excel = "application/excel",
        /// `application/pdf`.
        Pdf = "application/pdf",
    }
);

code_enum!(
    /// What a change position does. XSD `AktionscodeType`.
    Aktionscode {
        /// `A` — Änderung (change).
        Aenderung = "A",
        /// `W` — Wegfall (removal).
        Wegfall = "W",
        /// `Z` — Zugang (addition).
        Zugang = "Z",
    }
);

code_enum!(
    /// Salutation of a person. XSD `AnredePersonType`.
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
    /// Salutation of a company. XSD `AnredeFirmaType`.
    AnredeFirma {
        /// `4` — Firma.
        Firma = "4",
        /// `9` — keine Anrede.
        Keine = "9",
    }
);

code_enum!(
    /// Yes or no. XSD `JaNeinType`.
    JaNein {
        /// `J` — ja.
        Ja = "J",
        /// `N` — nein.
        Nein = "N",
    }
);

code_enum!(
    /// The hand-over point at the customer. XSD `UebergabepunktKundeType`.
    UebergabepunktKunde {
        /// `TAE` — a TAE socket.
        Tae = "TAE",
        /// `RJ45` — an RJ45 socket.
        Rj45 = "RJ45",
    }
);

code_enum!(
    /// The outcome of a fault repair (`entstoerErgebnis/ergebnis`).
    Entstoerergebnis {
        /// `FehlerBeseitigt` — fault removed.
        FehlerBeseitigt = "FehlerBeseitigt",
        /// `KeinenFehlerGefunden` — no fault found.
        KeinenFehlerGefunden = "KeinenFehlerGefunden",
    }
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phone_numbers_follow_the_pattern() {
        for ok in ["1234567", "030 1234567", "+49 30 1234567", "  0301   1234 "] {
            assert!(Telefonnummer::new(ok).is_ok(), "{ok}");
        }
        for bad in ["", "030-1234567", "+4930 123", "30 123", "+49 3 123", "0301234567890123"] {
            assert!(Telefonnummer::new(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn email_addresses_follow_the_pattern() {
        for ok in ["a@b.de", "erika.mustermann@example.co.uk", "x@y@z.org"] {
            assert!(Emailadresse::new(ok).is_ok(), "{ok}");
        }
        for bad in ["a@b", "@b.de", "a@.de", "a@b.", "ab.de", "a@b.d-e"] {
            assert!(Emailadresse::new(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn zeitfenster_has_125_values() {
        let all: Vec<String> = (1..=99).map(|n: u8| n.to_string()).chain(('A'..='Z').map(String::from)).collect();
        assert_eq!(all.len(), 125);
        for v in &all {
            assert_eq!(Zeitfenster::new(v).unwrap().to_string(), *v);
        }
        for bad in ["0", "01", "100", "a", "AA", "", " 1"] {
            assert!(Zeitfenster::new(bad).is_err(), "{bad:?}");
        }
        assert!(Zeitfenster::SAMSTAGS.is_assigned());
        assert!(!Zeitfenster::new("Q").unwrap().is_assigned());
    }

    // spri-model/SR-002: personal data does not appear in Debug output.
    #[test]
    fn personal_values_are_redacted() {
        let name = Nachname::new("Mustermann").unwrap();
        assert_eq!(format!("{name:?}"), "Nachname(<redacted, 10 chars>)");
        assert!(!format!("{:?}", Telefonnummer::new("030 1234567").unwrap()).contains("1234567"));
        assert_eq!(format!("{:?}", Ortsname::new("Berlin").unwrap()), "Ortsname(\"Berlin\")");
    }
}
