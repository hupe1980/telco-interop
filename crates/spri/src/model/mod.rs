//! Orders, messages and the values they are made of.
//!
//! Field names follow the XSD element names in `snake_case`; the XSD name is given where it
//! differs. Mandatory elements are plain fields, optional ones `Option`, repeated ones `Vec`
//! (the schema's bounds are checked when writing and reading). Abstract schema types are
//! enums ([`Geschaeftsfall`], [`Meldungsinhalt`]); the only product family the schema defines,
//! FTTx (`oss-product-FTTx.xsd`), is built in.

mod control;
mod message;
mod order;

pub use control::{Control, Payload, Release, SignaturId, Zeitstempel};
pub use message::{
    Abbm, AbbmPv, Abm, AbmPv, AkmPv, Auftragstyp, DiagnoseErgebnis, Entm, EntmPv, EntstoerErgebnis, Erlm, ErlmK,
    ErlmPv, FttxAnschluss, Kdle, Meldung, Meldungsinhalt, Position, PositionAbm, PositionQeb, PositionWita,
    ProviderwechselAnkuendigung, ProviderwechselBestaetigung, ProviderwechselRueckmeldung, Qeb, RuemPv, Tbk, Teq,
    Terminanforderung, VersandinfosOnt, Vzm, ZwmAg, ZwmLe,
};
pub use order::{
    AenderungPosition, AnsprechpartnerAuftrag, Auftrag, Bereitstellung, Diagnose, Entstoerung, FttxBereitstellung,
    FttxProviderwechsel, Geschaeftsfall, KontaktEndkunde, Kuendigung, Leistungsaenderung, LeistungsmerkmalAenderung,
    Montageleistung, OnkzRufnummer, PortierungAnschluss, Produkt, Providerwechsel, Reklamation, RufnummernPortierung,
    Rufnummernblock, Vormieter, VormieterPerson, WitaExtendedInfos, WitaInfos,
};

use core::fmt;

use jiff::civil::{Date, Time};

use crate::{
    Anlagentyp, Anrede, AnredeFirma, AnsprechpartnerRolle, Auftraggebernummer, Beschreibung, Dateiname, Dokumenttyp,
    Emailadresse, Firmenname, Gebaeudeteil, Hausnummer, HausnummernZusatz, LageTaeOnt, Land, Leistungsnummer, Nachname,
    Ortsname, Ortsteil, Postleitzahl, Projektkenner, Strassenname, Telefonnummer, Vorname, Zeitfenster,
};

/// A party by its customer and contract number (`AuftraggeberType`), used for `auftraggeber`
/// and `besteller`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Auftraggeber {
    /// `auftraggebernummer`.
    pub auftraggebernummer: Auftraggebernummer,
    /// `leistungsnummer`.
    pub leistungsnummer: Leistungsnummer,
}

/// A person (`PersonType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Person {
    /// `anrede`.
    pub anrede: Anrede,
    /// `vorname`.
    pub vorname: Option<Vorname>,
    /// `nachname`.
    pub nachname: Nachname,
}

/// A company (`FirmaType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Firma {
    /// `anrede`.
    pub anrede: AnredeFirma,
    /// `firmenname`.
    pub firmenname: Firmenname,
    /// `firmennameZweiterTeil`.
    pub firmenname_zweiter_teil: Option<Nachname>,
}

/// The person or company at a location (the choice `person` | `firma`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Inhaber {
    /// `person`.
    Person(Person),
    /// `firma`.
    Firma(Firma),
}

/// A contact person (`complex:AnsprechpartnerType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ansprechpartner {
    /// The person (`anrede`, `vorname`, `nachname`).
    pub person: Person,
    /// `telefonnummer`.
    pub telefonnummer: Telefonnummer,
    /// `mobilfunknummer`.
    pub mobilfunknummer: Option<Telefonnummer>,
    /// `faxnummer`.
    pub faxnummer: Option<Telefonnummer>,
    /// `emailadresse`.
    pub emailadresse: Option<Emailadresse>,
}

/// A contact person with a role (`AnsprechpartnerMitRolleType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnsprechpartnerMitRolle {
    /// The contact person.
    pub ansprechpartner: Ansprechpartner,
    /// `rolle`.
    pub rolle: AnsprechpartnerRolle,
}

/// `strasse` (`StrasseType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Strasse {
    /// `strassenname`.
    pub strassenname: Strassenname,
    /// `hausnummer`.
    pub hausnummer: Hausnummer,
    /// `hausnummernZusatz`.
    pub hausnummern_zusatz: Option<HausnummernZusatz>,
}

/// `ort` (`OrtType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ort {
    /// `ortsname`.
    pub ortsname: Ortsname,
    /// `ortsteil`.
    pub ortsteil: Option<Ortsteil>,
}

/// `gebaeudeteil` (`GebaeudeteilType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GebaeudeteilAngabe {
    /// `gebaeudeteilName`.
    pub name: Gebaeudeteil,
    /// `gebaeudeteilZusatz`.
    pub zusatz: Option<Gebaeudeteil>,
}

/// The line's location (`StandortAType`): who is there, the address and where the socket is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StandortA {
    /// `person` or `firma`.
    pub inhaber: Inhaber,
    /// `telefonnummer` of the person or company.
    pub telefonnummer: Option<Telefonnummer>,
    /// `mobilfunknummer` of the person or company.
    pub mobilfunknummer: Option<Telefonnummer>,
    /// `strasse`.
    pub strasse: Strasse,
    /// `gebaeudeteil`.
    pub gebaeudeteil: Option<GebaeudeteilAngabe>,
    /// `land`; absent means `de` (the schema default).
    pub land: Option<Land>,
    /// `postleitzahl`.
    pub postleitzahl: Postleitzahl,
    /// `ort`.
    pub ort: Ort,
    /// `lageTAE_ONT`.
    pub lage_tae_ont: Option<LageTaeOnt>,
}

/// Where to ship hardware (`StandortVersandType`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StandortVersand {
    /// `person` or `firma`.
    pub inhaber: Inhaber,
    /// `strasse`.
    pub strasse: Strasse,
    /// `land`; absent means `de`.
    pub land: Option<Land>,
    /// `postleitzahl`.
    pub postleitzahl: Postleitzahl,
    /// `ort`.
    pub ort: Ort,
}

/// A requested or binding date with an optional time window (`WunschterminType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Wunschtermin {
    /// `datum`.
    pub datum: Date,
    /// `zeitfenster`.
    pub zeitfenster: Option<Zeitfenster>,
}

impl From<Date> for Wunschtermin {
    fn from(datum: Date) -> Self {
        Self { datum, zeitfenster: None }
    }
}

/// An appointment with time and time window (`WunschterminMitZeitangabeUndZeitfensterType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Termin {
    /// `datum`.
    pub datum: Date,
    /// `zeit`.
    pub zeit: Option<Time>,
    /// `zeitfenster`.
    pub zeitfenster: Option<Zeitfenster>,
}

/// `projektId`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjektId {
    /// `projektkenner`.
    pub projektkenner: Projektkenner,
    /// `kopplungskenner`.
    pub kopplungskenner: Option<Projektkenner>,
}

/// The largest `auftragsklammer` the schema admits.
pub const MAX_AUFTRAGSKLAMMER: u64 = 9_999_999_999;

/// `auftragskenner`: groups several orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Auftragskenner {
    /// `auftragsklammer`: 0 to [`MAX_AUFTRAGSKLAMMER`] (`xsd:long` restricted).
    pub auftragsklammer: u64,
    /// `anzahlAuftraege` (`xsd:int`).
    pub anzahl_auftraege: i32,
}

/// `kenner` (`KennerType`): project and order grouping.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Kenner {
    /// `projektId`.
    pub projekt_id: Option<ProjektId>,
    /// `auftragskenner`.
    pub auftragskenner: Option<Auftragskenner>,
}

/// An attachment (`AnlageMitTypType`): a document embedded as base64.
///
/// `Debug` prints the size of the content, never the content or the file name.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Anlage {
    /// `dateiname`, as sent. Use [`Anlage::safe_file_name`] before storing it.
    pub dateiname: Dateiname,
    /// `dateityp`.
    pub dateityp: Dokumenttyp,
    /// `beschreibung`.
    pub beschreibung: Option<Beschreibung>,
    /// `inhalt`, decoded.
    pub inhalt: Vec<u8>,
    /// `anlagentyp`.
    pub anlagentyp: Anlagentyp,
}

impl fmt::Debug for Anlage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Anlage")
            .field("dateiname", &self.dateiname)
            .field("dateityp", &self.dateityp)
            .field("beschreibung", &self.beschreibung)
            .field("inhalt", &format_args!("<{} bytes>", self.inhalt.len()))
            .field("anlagentyp", &self.anlagentyp)
            .finish()
    }
}

impl Anlage {
    /// A file name that is safe to join to a folder: the last component of `dateiname` (after
    /// `/` and `\`), without control characters, characters Windows forbids (`:*?"<>|`) and
    /// leading dots or spaces; never `.` or `..`, never empty (`attachment` instead).
    ///
    /// The name comes from another operator and may try path traversal (`../../etc/passwd`,
    /// `C:\x\y.pdf`) or hide a file (`.hidden`).
    #[must_use]
    pub fn safe_file_name(&self) -> String {
        safe_file_name(self.dateiname.as_str())
    }
}

fn safe_file_name(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String =
        base.chars().filter(|c| !c.is_control() && !matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|')).collect();
    let trimmed = cleaned.trim_start_matches(['.', ' ']).trim_end_matches([' ', '.']);
    if trimmed.is_empty() { "attachment".to_owned() } else { trimmed.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anlage(name: &str) -> Anlage {
        Anlage {
            dateiname: Dateiname::new(name).unwrap(),
            dateityp: Dokumenttyp::Pdf,
            beschreibung: None,
            inhalt: b"%PDF-1.4 secret".to_vec(),
            anlagentyp: Anlagentyp::Sonstige,
        }
    }

    // spri-model/SR-003: the safe file name has no separator, parent reference, leading dot or
    // control character, and is never empty.
    #[test]
    fn safe_file_names() {
        for (raw, safe) in [
            ("../../etc/passwd", "passwd"),
            ("C:\\x\\y.pdf", "y.pdf"),
            (".hidden", "hidden"),
            ("..", "attachment"),
            ("dir/..", "attachment"),
            ("a/b/", "attachment"),
            ("auf\u{7}trag\u{1b}.pdf", "auftrag.pdf"),
            ("\u{85}x.pdf", "x.pdf"),
            ("C:auftrag.pdf", "Cauftrag.pdf"),
            ("...  ", "attachment"),
            ("Auftrag Müller.pdf", "Auftrag Müller.pdf"),
        ] {
            // `Dateiname` refuses control characters already; the function is checked directly
            // so it stays safe on its own.
            let got = Dateiname::new(raw).map_or_else(|_| safe_file_name(raw), |_| anlage(raw).safe_file_name());
            assert_eq!(got, safe, "{raw:?}");
            assert!(!got.contains(['/', '\\']) && !got.starts_with('.') && !got.chars().any(char::is_control));
        }
    }

    // spri-model/SR-002: attachment content and name stay out of Debug.
    #[test]
    fn anlage_debug_shows_size_only() {
        let text = format!("{:?}", anlage("Vertrag Mustermann.pdf"));
        assert!(text.contains("<15 bytes>"), "{text}");
        assert!(!text.contains("secret") && !text.contains("Mustermann") && !text.contains("37, 80"), "{text}");
    }
}
