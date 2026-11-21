//! Business cases, message kinds and the Meldungscode table.
//!
//! Source: AH4WBCI 2.10 §5.3 (code list), §5.4 (fact sheets: owning message type) and §2
//! (codes per business case and process phase); WBCI 2.0 §11.5 (8116 for an unsupported
//! business case). XSD `GeschaeftsfallEnumType`.

use core::fmt;

crate::values::code_enum!(
    /// The business case (Geschäftsfall) of a request; messages repeat the case they answer.
    ///
    /// XSD `GeschaeftsfallEnumType`.
    Geschaeftsfall {
        /// `VA-KUE-MRN` — Kündigung mit Rufnummernportierung (switch with number porting).
        VaKueMrn = "VA-KUE-MRN",
        /// `VA-KUE-ORN` — Kündigung ohne Rufnummernportierung (switch without porting).
        VaKueOrn = "VA-KUE-ORN",
        /// `VA-RRNP` — reine Rufnummernportierung (porting only).
        VaRrnp = "VA-RRNP",
        /// `STR-AUF` — Storno (Aufhebung): cancel the pre-agreement.
        StrAuf = "STR-AUF",
        /// `STR-AEN` — Storno (Änderung): cancel to resubmit changed.
        StrAen = "STR-AEN",
        /// `TVS-VA` — Terminverschiebung: postpone the switch date.
        TvsVa = "TVS-VA",
    }
);

impl Geschaeftsfall {
    /// Whether this is one of the three pre-agreement requests.
    #[must_use]
    pub const fn is_vorabstimmung(self) -> bool {
        matches!(self, Self::VaKueMrn | Self::VaKueOrn | Self::VaRrnp)
    }

    /// Whether the switch ends the customer's contract (the two `VA-KUE-*` cases): only these
    /// take over a line (AKM-TR).
    #[must_use]
    pub const fn is_kuendigung(self) -> bool {
        matches!(self, Self::VaKueMrn | Self::VaKueOrn)
    }
}

/// The kind of a message (Meldungstyp).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Meldungstyp {
    /// Technische Quittung: the synchronous technical receipt.
    Teq,
    /// Rückmeldung Vorabstimmung: the losing provider agrees to the switch.
    RuemVa,
    /// Abbruchmeldung: a request is refused.
    Abbm,
    /// Erledigungsmeldung: a cancellation or postponement is done.
    Erlm,
    /// Mitteilung zur Übernahme der technischen Ressource (line takeover announcement).
    AkmTr,
    /// Abbruchmeldung zur AKM-TR: the takeover announcement is refused.
    AbbmTr,
}

impl Meldungstyp {
    /// The name used in the specifications, e.g. `RUEM-VA`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Teq => "TEQ",
            Self::RuemVa => "RUEM-VA",
            Self::Abbm => "ABBM",
            Self::Erlm => "ERLM",
            Self::AkmTr => "AKM-TR",
            Self::AbbmTr => "ABBM-TR",
        }
    }
}

impl fmt::Display for Meldungstyp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

use Geschaeftsfall as G;
use Meldungstyp as T;

const ALL: &[G] = &[G::VaKueMrn, G::VaKueOrn, G::VaRrnp, G::StrAuf, G::StrAen, G::TvsVa];
const VA: &[G] = &[G::VaKueMrn, G::VaKueOrn, G::VaRrnp];
const KUE: &[G] = &[G::VaKueMrn, G::VaKueOrn];
const STR: &[G] = &[G::StrAuf, G::StrAen];
const TVS: &[G] = &[G::TvsVa];

/// One row of the code table: the code, its official text, a short name if the
/// specifications use one, and where it may appear.
struct Row {
    code: &'static str,
    text: &'static str,
    short: Option<&'static str>,
    uses: &'static [(Meldungstyp, &'static [Geschaeftsfall])],
}

macro_rules! codes {
    ($($(#[$m:meta])* $variant:ident = $code:literal, $text:literal, $short:expr, [$(($ty:ident, $gf:ident)),*];)+) => {
        /// A message code (Meldungscode) with its official text.
        ///
        /// The 33 codes of AH4WBCI 2.10 §5.3 are variants; a code a peer sends that the list
        /// does not contain is kept as [`Meldungscode::Other`] (the schema admits any 1–10
        /// characters) and flagged by validation.
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum Meldungscode {
            $($(#[$m])* $variant,)+
            /// A code not in the published list.
            Other(Box<str>),
        }

        impl Meldungscode {
            fn row(&self) -> Option<&'static Row> {
                match self {
                    $(Self::$variant => Some(&Row { code: $code, text: $text, short: $short, uses: &[$((T::$ty, $gf)),*] }),)+
                    Self::Other(_) => None,
                }
            }

            /// Every published code, in the order of AH §5.3.
            pub const ALL: &'static [Meldungscode] = &[$(Self::$variant),+];

            fn from_known(s: &str) -> Option<Self> {
                match s { $($code => Some(Self::$variant),)+ _ => None }
            }
        }
    };
}

codes! {
    /// `OK` — positive technical receipt.
    Ok = "OK", "OK.", None, [(Teq, ALL)];
    /// `0993` — version wrong (major release not supported).
    VersionFehlerhaft = "0993", "Versionsnummer fehlerhaft", None, [(Teq, ALL)];
    /// `0995` — data structure invalid (schema or structural check failed).
    DatenstrukturUngueltig = "0995", "Datenstruktur ungültig", None, [(Teq, ALL)];
    /// `0999` — technical error at the receiver.
    TechnischerFehler = "0999", "Technischer Fehler", None, [(Teq, ALL)];
    /// `8001` — agreed as requested (ZWA).
    Zwa = "8001", "Auftragsbestätigung (ZWA)", Some("ZWA"), [(RuemVa, VA)];
    /// `8002` — agreed with a new, later date (NAT).
    Nat = "8002", "Auftragsbestätigung (NAT)", Some("NAT"), [(RuemVa, VA)];
    /// `8003` — postponement accepted.
    TerminverschiebungOk = "8003", "Terminverschiebung ok", None, [(Erlm, TVS)];
    /// `8004` — cancellation accepted.
    StornoOk = "8004", "Storno ok", None, [(Erlm, STR)];
    /// `8010` — the gaining provider's takeover announcement.
    AntwortAufnehmend = "8010", "Antwort des aufnehmenden Anbieters", None, [(AkmTr, KUE)];
    /// `8103` — number not switched (RNG).
    Rng = "8103", "Rufnummer nicht geschaltet (RNG)", Some("RNG"), [(Abbm, VA)];
    /// `8104` — address differs: postcode (ADA PLZ).
    AdaPlz = "8104", "Adresse abweichend PLZ (ADAPLZ)", Some("ADAPLZ"), [(RuemVa, KUE)];
    /// `8105` — address differs: town (ADA Ort).
    AdaOrt = "8105", "Adresse abweichend Ort (ADAORT)", Some("ADAORT"), [(RuemVa, KUE)];
    /// `8106` — address differs: street (ADA Straße).
    AdaStr = "8106", "Adresse abweichend Straße (ADASTR)", Some("ADASTR"), [(RuemVa, KUE)];
    /// `8107` — address differs: house number (ADA Hausnummer).
    AdaHsnr = "8107", "Adresse abweichend Hausnummer (ADAHSNR)", Some("ADAHSNR"), [(RuemVa, KUE)];
    /// `8108` — address wrong: postcode (ADF PLZ).
    AdfPlz = "8108", "Adresse falsch Postleitzahl (ADFPLZ)", Some("ADFPLZ"), [(Abbm, KUE)];
    /// `8109` — address wrong: town.
    AdfOrt = "8109", "Adresse falsch Ort (ADFORT)", Some("ADFORT"), [(Abbm, KUE)];
    /// `8110` — address wrong: street.
    AdfStr = "8110", "Adresse falsch Straße (ADFSTR)", Some("ADFSTR"), [(Abbm, KUE)];
    /// `8111` — address wrong: house number.
    AdfHsnr = "8111", "Adresse falsch Hausnummer (ADFHSNR)", Some("ADFHSNR"), [(Abbm, KUE)];
    /// `8112` — line holder wrong (AIF).
    Aif = "8112", "Anschlussinhaber falsch (AIF)", Some("AIF"), [(Abbm, VA)];
    /// `8113` — line holder's first name wrong (AIFVN).
    AifVn = "8113", "Anschlussinhaber falsch Vorname (AIFVN)", Some("AIFVN"), [(Abbm, VA)];
    /// `8114` — further line holders missing (WAI).
    Wai = "8114", "Weitere Anschlussinhaber (WAI)", Some("WAI"), [(Abbm, VA)];
    /// `8115` — the id is already used (duplicate request).
    IdBereitsVerwendet = "8115", "Vorabstimmungs-,Terminverschiebungs-,Storno-, ID wird bereits verwendet", None, [(Abbm, ALL)];
    /// `8116` — other reason (SON), always with a reason; also answers an unsupported case.
    Sonstiges = "8116", "Sonstiges", Some("SON"), [(Abbm, ALL)];
    /// `8117` — customer not identifiable (KNI).
    Kni = "8117", "Kunde nicht identifizierbar (KNI)", Some("KNI"), [(Abbm, VA)];
    /// `8118` — a pre-agreement already exists (VAE).
    Vae = "8118", "Vorabstimmung mit anderen oder gleichen EKP bereits erfolgt (VAE)", Some("VAE"), [(Abbm, VA)];
    /// `8140` — Vorabstimmungs-ID unknown.
    VorabstimmungsIdUnbekannt = "8140", "Vorabstimmungs ID nicht bekannt", None, [(Abbm, STR), (AbbmTr, KUE)];
    /// `8141` — postponement refused.
    TerminverschiebungAbgelehnt = "8141", "Terminverschiebung abgelehnt", None, [(Abbm, TVS)];
    /// `8142` — cancellation refused.
    StornoAbgelehnt = "8142", "Storno abgelehnt", None, [(Abbm, STR)];
    /// `8143` — WITA contract number unknown.
    VertragsnummerUnbekannt = "8143", "WITA Vertragsnummer unbekannt", None, [(AbbmTr, KUE)];
    /// `8144` — LineID unknown.
    LineIdUnbekannt = "8144", "Line ID unbekannt", None, [(AbbmTr, KUE)];
    /// `8145` — WITA contract number does not belong to the Vorabstimmungs-ID.
    VertragsnummerFremd = "8145", "WITA Vertragsnummer gehört nicht zur Vorabstimmungs ID", None, [(AbbmTr, KUE)];
    /// `8146` — LineID does not belong to the Vorabstimmungs-ID.
    LineIdFremd = "8146", "Line ID gehört nicht zur Vorabstimmungs ID", None, [(AbbmTr, KUE)];
    /// `8147` — takeover of the technical resource not possible.
    UebernahmeNichtMoeglich = "8147", "Übernahme der Technischen Ressource nicht möglich", None, [(AbbmTr, KUE)];
    /// `8148` — takeover already confirmed.
    UebernahmeBereitsBestaetigt = "8148", "Übernahme der Technischen Ressource wurde bereits bestätigt", None, [(AbbmTr, KUE)];
    /// `8149` — project identifier unknown.
    ProjektkennerUnbekannt = "8149", "Projektkenner unbekannt", None, [(Abbm, VA)];
    /// `8150` — the customer withdrew the switch order (NWE).
    Nwe = "8150", "Neue Willenserklärung des Endkunden (NWE)", Some("NWE"), [(Abbm, KUE)];
}

impl Meldungscode {
    /// Parses a code as sent on the wire; unknown codes become [`Meldungscode::Other`].
    #[must_use]
    pub fn parse(code: &str) -> Self {
        Self::from_known(code).unwrap_or_else(|| Self::Other(code.into()))
    }

    /// The code on the wire, e.g. `8103`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Other(code) => code,
            known => known.row().map_or("", |r| r.code),
        }
    }

    /// The official text (AH §5.3), or `None` for an unknown code.
    #[must_use]
    pub fn text(&self) -> Option<&'static str> {
        self.row().map(|r| r.text)
    }

    /// The short name the specifications use, e.g. `RNG`, `ZWA`, `NAT`.
    #[must_use]
    pub fn short(&self) -> Option<&'static str> {
        self.row().and_then(|r| r.short)
    }

    /// The message kinds that may carry this code.
    pub fn meldungstypen(&self) -> impl Iterator<Item = Meldungstyp> + '_ {
        self.row().into_iter().flat_map(|r| r.uses.iter().map(|(t, _)| *t))
    }

    /// Whether a message of kind `typ` answering business case `fall` may carry this code
    /// (AH §2; 8116 on any ABBM per WBCI 2.0 §11.5).
    #[must_use]
    pub fn applies(&self, typ: Meldungstyp, fall: Geschaeftsfall) -> bool {
        self.row().is_some_and(|r| r.uses.iter().any(|(t, cases)| *t == typ && cases.contains(&fall)))
    }

    /// Whether the code is a positive technical receipt.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        *self == Self::Ok
    }
}

impl core::str::FromStr for Meldungscode {
    type Err = core::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::parse(s))
    }
}

impl fmt::Display for Meldungscode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.text() {
            Some(text) => write!(f, "{} {text}", self.as_str()),
            None => write!(f, "{} (unknown code)", self.as_str()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_complete_and_unique() {
        assert_eq!(Meldungscode::ALL.len(), 36);
        let mut seen = std::collections::HashSet::new();
        for code in Meldungscode::ALL {
            assert!(seen.insert(code.as_str()), "duplicate {}", code.as_str());
            assert_eq!(Meldungscode::parse(code.as_str()), *code);
            assert!(code.meldungstypen().next().is_some());
            let text = code.text().unwrap();
            assert_eq!(
                crate::Meldungstext::new(text).unwrap().as_str(),
                text,
                "official text must be a valid Meldungstext"
            );
        }
    }

    #[test]
    fn applicability_follows_the_handbook() {
        use Meldungscode as C;
        assert!(C::Zwa.applies(T::RuemVa, G::VaRrnp));
        assert!(!C::AdaPlz.applies(T::RuemVa, G::VaRrnp));
        assert!(C::Nwe.applies(T::Abbm, G::VaKueOrn));
        assert!(!C::Nwe.applies(T::Abbm, G::VaRrnp));
        assert!(C::Sonstiges.applies(T::Abbm, G::TvsVa));
        assert!(C::VorabstimmungsIdUnbekannt.applies(T::AbbmTr, G::VaKueMrn));
        assert!(C::VorabstimmungsIdUnbekannt.applies(T::Abbm, G::StrAen));
        assert!(!C::Zwa.applies(T::Abbm, G::VaKueMrn));
        assert!(C::Ok.applies(T::Teq, G::TvsVa));
    }

    #[test]
    fn unknown_codes_are_kept() {
        let c = Meldungscode::parse("9999");
        assert_eq!(c.as_str(), "9999");
        assert_eq!(c.text(), None);
        assert_eq!(c.to_string(), "9999 (unknown code)");
        assert_eq!(Meldungscode::Rng.to_string(), "8103 Rufnummer nicht geschaltet (RNG)");
    }
}
