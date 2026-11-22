//! Reading payloads from untrusted bytes.
//!
//! The input is first parsed into a bounded element tree (no DTD, only the five predefined
//! entities and character references, limits on size, depth, element count and text length),
//! then mapped onto the model in the schema's element order. Every failure names the element
//! path and maps to the receipt code a receiver answers with: `0993` for an unsupported major
//! release, `0995` for everything else (WBCI 2.0 §3.2, AH §5.3).

use core::fmt;

use carrier_core::xml::{Limits, Node, Seq, XmlError, XmlErrorKind, leaf, parse_zeitstempel, unwrap_soap};
use jiff::civil::Date;

use super::{NS_ENVELOPE, NS_MESSAGE, NS_REQUEST, max, types};
use crate::model::{
    Abbm, AbbmTr, AdresseAbweichend, AkmTr, Anfrage, Anlagenkopf, Anschluss, Bezug, Control, Ekp, Erlm, Firma, Kunde,
    Meldung, MeldungKopf, OnkzRufnummer, Payload, Person, PortierteRufnummern, Position, PositionAbbm, PositionRuemVa,
    ProjektId, Release, RuemVa, Rufnummernblock, Rufnummernportierung, SignaturId, Standort, Storno, StornoArt,
    TechnischeRessource, Teq, Terminverschiebung, UebernahmeLeitung, Vorabstimmung, VorabstimmungsFall,
};
use crate::{Anrede, AnredeFirma, CarrierCode, Geschaeftsfall, Meldungscode, Portierungszeitfenster, ValueError};

/// Why a payload could not be read.
///
/// The message names the element path and, where the parser knows it, the byte offset; it
/// never repeats element content, which may be personal data.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub struct ReadError {
    /// Element path, e.g. `annehmenAnfrageRequest/Anfrage/Endkunde/person/nachname`.
    pub path: String,
    /// Byte offset in the input, if known.
    pub offset: Option<u64>,
    /// What is wrong.
    pub kind: ReadErrorKind,
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            write!(f, "{}", self.kind)?;
        } else {
            write!(f, "{}: {}", self.path, self.kind)?;
        }
        if let Some(offset) = self.offset {
            write!(f, " (byte {offset})")?;
        }
        Ok(())
    }
}

/// What a [`ReadError`] reports.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ReadErrorKind {
    /// The input is larger than [`Limits::max_bytes`].
    #[error("input of {actual} bytes exceeds the limit of {limit}")]
    TooLarge {
        /// The limit.
        limit: usize,
        /// The input size.
        actual: usize,
    },
    /// Elements nest deeper than [`Limits::max_depth`].
    #[error("elements nest deeper than {limit}")]
    TooDeep {
        /// The limit.
        limit: usize,
    },
    /// More elements than [`Limits::max_elements`].
    #[error("more than {limit} elements")]
    TooManyElements {
        /// The limit.
        limit: usize,
    },
    /// An element's text is longer than [`Limits::max_text`].
    #[error("text longer than {limit} characters")]
    TextTooLong {
        /// The limit.
        limit: usize,
    },
    /// An element repeats more often than the schema allows.
    #[error("{element} occurs more than {max} times")]
    TooMany {
        /// The element.
        element: &'static str,
        /// The schema's maximum.
        max: usize,
    },
    /// A document type declaration: refused, so no entity can be declared or expanded.
    #[error("DOCTYPE declarations are not accepted")]
    Doctype,
    /// An entity reference other than the five predefined ones.
    #[error("unknown entity reference")]
    Entity,
    /// The document is not well-formed or not UTF-8.
    #[error("malformed XML: {0}")]
    Malformed(String),
    /// A mandatory element is missing.
    #[error("missing element {0}")]
    Missing(&'static str),
    /// An element the schema does not allow here.
    #[error("unexpected element {0}")]
    Unexpected(String),
    /// Text where only elements are allowed, or elements in a text-only element.
    #[error("mixed content is not allowed")]
    MixedContent,
    /// An `xsi:type` that is missing, unknown or in the wrong namespace.
    #[error("xsi:type {0}")]
    XsiType(String),
    /// A value that violates its schema type.
    #[error(transparent)]
    Value(ValueError),
    /// A value of an enumeration, date, boolean or timestamp that does not parse.
    #[error("invalid {0}")]
    Invalid(&'static str),
    /// A major release this library does not implement.
    #[error("major release {0} is not supported (WBCI 2.0 is 02)")]
    Version(String),
    /// A request whose `Absender` is not the side its type says sends it.
    #[error("Absender must be the {0} for this request type")]
    Absender(&'static str),
}

impl ReadError {
    /// The receipt code a receiver answers this input with: `0993` for an unsupported
    /// release, `0995` otherwise.
    #[must_use]
    pub const fn teq_code(&self) -> Meldungscode {
        match self.kind {
            ReadErrorKind::Version(_) => Meldungscode::VersionFehlerhaft,
            _ => Meldungscode::DatenstrukturUngueltig,
        }
    }

    fn at(path: &str, kind: ReadErrorKind) -> Self {
        Self { path: path.to_owned(), offset: None, kind }
    }
}

impl From<XmlError> for ReadError {
    fn from(e: XmlError) -> Self {
        let kind = match e.kind {
            XmlErrorKind::TooLarge { limit, actual } => ReadErrorKind::TooLarge { limit, actual },
            XmlErrorKind::TooDeep { limit } => ReadErrorKind::TooDeep { limit },
            XmlErrorKind::TooManyElements { limit } => ReadErrorKind::TooManyElements { limit },
            // WBCI declares no embedded documents; the parser reports their size as text length.
            XmlErrorKind::TextTooLong { limit } | XmlErrorKind::AttachmentTooLarge { limit } => {
                ReadErrorKind::TextTooLong { limit }
            }
            XmlErrorKind::TooMany { element, max } => ReadErrorKind::TooMany { element, max },
            XmlErrorKind::Doctype => ReadErrorKind::Doctype,
            XmlErrorKind::Entity => ReadErrorKind::Entity,
            XmlErrorKind::Missing(name) => ReadErrorKind::Missing(name),
            XmlErrorKind::Unexpected(name) => ReadErrorKind::Unexpected(name),
            XmlErrorKind::MixedContent => ReadErrorKind::MixedContent,
            XmlErrorKind::XsiType(why) => ReadErrorKind::XsiType(why),
            XmlErrorKind::Value(v) => ReadErrorKind::Value(v),
            XmlErrorKind::Invalid(what) => ReadErrorKind::Invalid(what),
            XmlErrorKind::Malformed(why) => ReadErrorKind::Malformed(why),
            other => ReadErrorKind::Malformed(other.to_string()),
        };
        Self { path: e.path, offset: e.offset, kind }
    }
}

impl Payload {
    /// Reads a payload with the default [`Limits`]: the bare payload element or a SOAP 1.1
    /// envelope whose body holds it.
    ///
    /// # Errors
    ///
    /// [`ReadError`]; [`ReadError::teq_code`] gives the receipt code to answer with.
    pub fn from_xml(input: &[u8]) -> Result<Self, ReadError> {
        Self::from_xml_with(input, &Limits::default())
    }

    /// Reads a payload with explicit limits.
    ///
    /// # Errors
    ///
    /// As [`Payload::from_xml`].
    pub fn from_xml_with(input: &[u8], limits: &Limits) -> Result<Self, ReadError> {
        let root = carrier_core::xml::parse(input, limits, &[])?;
        let payload = unwrap_soap(&root)?;
        map_payload(payload)
    }
}

// ---------------------------------------------------------------------------------------
// Mapping

fn code_enum<T>(
    seq: &mut Seq<'_>,
    name: &'static str,
    what: &'static str,
    parse: fn(&str) -> Option<T>,
) -> Result<T, ReadError> {
    Ok(seq.code(name, what, parse)?)
}

fn seq(node: &Node, path: String) -> Result<Seq<'_>, ReadError> {
    Ok(Seq::new(node, path)?)
}

fn xsi<'a>(node: &'a Node, path: &str, ns: &str) -> Result<&'a str, ReadError> {
    Ok(node.xsi(path, ns)?)
}

fn map_payload(root: &Node) -> Result<Payload, ReadError> {
    let name = root.name.as_str();
    if root.ns.as_deref() != Some(NS_ENVELOPE) {
        return Err(ReadError::at(
            name,
            ReadErrorKind::Unexpected(format!("{name} (expected a WBCI payload in {NS_ENVELOPE})")),
        ));
    }
    let mut seq = Seq::new(root, name.to_owned())?;
    let mut c = seq.seq("Control")?;
    let control = control(&mut c)?;
    c.done()?;
    let payload = match name {
        "annehmenAnfrageRequest" => {
            let (node, path) = seq.req("Anfrage")?;
            Payload::AnfrageRequest { control, anfrage: anfrage(node, &path)? }
        }
        "annehmenMeldungRequest" => {
            let (node, path) = seq.req("Meldung")?;
            Payload::MeldungRequest { control, meldung: meldung(node, &path)? }
        }
        "annehmenAnfrageResponse" => Payload::AnfrageResponse { control, quittung: teq(seq.seq("Quittung")?)? },
        "annehmenMeldungResponse" => Payload::MeldungResponse { control, quittung: teq(seq.seq("Quittung")?)? },
        other => {
            return Err(ReadError::at(other, ReadErrorKind::Unexpected(other.to_owned())));
        }
    };
    seq.done()?;
    Ok(payload)
}

fn two_digits(text: &str) -> Option<u8> {
    (text.len() == 2 && text.bytes().all(|b| b.is_ascii_digit())).then(|| text.parse().ok()).flatten()
}

fn control(seq: &mut Seq<'_>) -> Result<Control, ReadError> {
    let (text, path) = seq.text("zeitstempel")?;
    let zeitstempel = parse_zeitstempel(text, &path)?;
    let (major_text, path) = seq.text("majorRelease")?;
    let major =
        two_digits(major_text.trim()).ok_or_else(|| ReadError::at(&path, ReadErrorKind::Invalid("majorRelease")))?;
    if major != Release::WBCI_2_0.major {
        return Err(ReadError::at(&path, ReadErrorKind::Version(major_text.trim().to_owned())));
    }
    let (minor_text, path) = seq.text("minorRelease")?;
    let minor =
        two_digits(minor_text.trim()).ok_or_else(|| ReadError::at(&path, ReadErrorKind::Invalid("minorRelease")))?;
    let mut sig = seq.seq("signaturId")?;
    let signatur = SignaturId { issuer: sig.val("issuer")?, serial: sig.val("serial")? };
    sig.done()?;
    Ok(Control { zeitstempel, release: Release { major, minor }, signatur })
}

fn carrier(seq: &mut Seq<'_>, name: &'static str) -> Result<CarrierCode, ReadError> {
    let mut s = seq.seq(name)?;
    let code = s.val("CarrierCode")?;
    s.done()?;
    Ok(code)
}

fn ekp(seq: &mut Seq<'_>) -> Result<Ekp, ReadError> {
    let mut s = seq.seq("Endkundenvertragspartner")?;
    let ekp = Ekp { auf: carrier(&mut s, "EKPauf")?, abg: carrier(&mut s, "EKPabg")? };
    s.done()?;
    Ok(ekp)
}

fn kunde(mut s: Seq<'_>) -> Result<Kunde, ReadError> {
    let kunde = if let Some(mut p) = s.opt_seq("person")? {
        let person = Person {
            anrede: code_enum(&mut p, "anrede", "anrede (1, 2, 9)", Anrede::from_code)?,
            vorname: p.opt_val("vorname")?,
            nachname: p.val("nachname")?,
        };
        p.done()?;
        Kunde::Person(person)
    } else {
        let mut f = s.seq("firma")?;
        let firma = Firma {
            anrede: code_enum(&mut f, "anrede", "anrede (4, 9)", AnredeFirma::from_code)?,
            name: f.val("firmenname")?,
            name_zweiter_teil: f.opt_val("firmennameZweiterTeil")?,
        };
        f.done()?;
        Kunde::Firma(firma)
    };
    s.done()?;
    Ok(kunde)
}

fn standort(mut s: Seq<'_>) -> Result<Standort, ReadError> {
    let mut str = s.seq("strasse")?;
    let strassenname = str.val("strassenname")?;
    let hausnummer = str.val("hausnummer")?;
    let hausnummern_zusatz = str.opt_val("hausnummernZusatz")?;
    str.done()?;
    let standort = Standort {
        strassenname,
        hausnummer,
        hausnummern_zusatz,
        postleitzahl: s.val("postleitzahl")?,
        ort: s.val("ort")?,
    };
    s.done()?;
    Ok(standort)
}

fn onkz_rufnummer(mut s: Seq<'_>) -> Result<OnkzRufnummer, ReadError> {
    let n = OnkzRufnummer { onkz: s.val("ONKZ")?, rufnummer: s.val("rufnummer")? };
    s.done()?;
    Ok(n)
}

fn anlagenkopf(seq: &mut Seq<'_>) -> Result<Anlagenkopf, ReadError> {
    let mut s = seq.seq("onkzDurchwahlAbfragestelle")?;
    let k = Anlagenkopf {
        onkz: s.val("ONKZ")?,
        durchwahl: s.val("durchwahlnummer")?,
        abfragestelle: s.val("abfragestelle")?,
    };
    s.done()?;
    Ok(k)
}

fn block(s: &mut Seq<'_>) -> Result<Rufnummernblock, ReadError> {
    Ok(Rufnummernblock { von: s.val("rnrBlockVon")?, bis: s.val("rnrBlockBis")? })
}

/// `RufnummernportierungType`; the caller reads a trailing `PortierungskennungPKIauf`.
fn rufnummernportierung(s: &mut Seq<'_>) -> Result<Rufnummernportierung, ReadError> {
    let anschluss = if let Some(mut a) = s.opt_seq("anlagenanschluss")? {
        let kopf = anlagenkopf(&mut a)?;
        let mut bloecke = Vec::new();
        for (node, path) in a.many("zuPortierenderRufnummernblock", 1, max::BLOCKS)? {
            let mut b = Seq::new(node, path)?;
            bloecke.push(block(&mut b)?);
            b.done()?;
        }
        a.done()?;
        Anschluss::Anlage { kopf, bloecke }
    } else {
        let mut e = s.seq("einzelanschluss")?;
        let mut liste = e.seq("rufnummernliste")?;
        let rufnummern = liste
            .many("zuPortierendeOnkzRnr", 1, max::NUMBERS)?
            .into_iter()
            .map(|(node, path)| seq(node, path).and_then(onkz_rufnummer))
            .collect::<Result<_, _>>()?;
        liste.done()?;
        let alle_rufnummern = if e.peek("alleRufnummern") { e.boolean("alleRufnummern")? } else { false };
        e.done()?;
        Anschluss::Einzel { rufnummern, alle_rufnummern }
    };
    let zeitfenster = code_enum(
        s,
        "Portierungszeitfenster",
        "Portierungszeitfenster (ZF1, ZF2, ZF3)",
        Portierungszeitfenster::from_code,
    )?;
    Ok(Rufnummernportierung { anschluss, zeitfenster })
}

fn anfrage(node: &Node, path: &str) -> Result<Anfrage, ReadError> {
    let ty = xsi(node, path, NS_REQUEST)?.to_owned();
    let mut s = Seq::new(node, path.to_owned())?;
    let absender = carrier(&mut s, "Absender")?;
    let ekp = ekp(&mut s)?;
    let anfrage = match ty.as_str() {
        types::KUE_MRN | types::KUE_ORN | types::RRNP => Anfrage::Vorabstimmung(vorabstimmung_body(&mut s, &ty, ekp)?),
        types::STR_AUF_AUF | types::STR_AUF_ABG | types::STR_AEN_AUF | types::STR_AEN_ABG => {
            let storno_id = s.val("StornoId")?;
            let bezug = bezug(&mut s)?;
            let art = match ty.as_str() {
                types::STR_AUF_AUF => StornoArt::AufhebungAuf,
                types::STR_AEN_AUF => {
                    let name = s.opt_seq("Name")?.map(kunde).transpose()?;
                    StornoArt::AenderungAuf { name, standort: s.opt_seq("Standort")?.map(standort).transpose()? }
                }
                _ => {
                    let name = s.opt_seq("Name")?.map(kunde).transpose()?;
                    let standort = s.opt_seq("Standort")?.map(standort).transpose()?;
                    let stornogrund = s.val("Stornogrund")?;
                    if ty == types::STR_AUF_ABG {
                        StornoArt::AufhebungAbg { name, standort, stornogrund }
                    } else {
                        StornoArt::AenderungAbg { name, standort, stornogrund }
                    }
                }
            };
            Anfrage::Storno(Storno { ekp, storno_id, bezug, art })
        }
        types::TVS => {
            let aenderungs_id = s.val("AenderungsId")?;
            let bezug = bezug(&mut s)?;
            let name = s.opt_seq("Name")?.map(kunde).transpose()?;
            let neuer_kundenwunschtermin = s.date("NeuerKundenwunschtermin")?;
            Anfrage::Terminverschiebung(Terminverschiebung {
                ekp,
                aenderungs_id,
                bezug,
                name,
                neuer_kundenwunschtermin,
            })
        }
        other => {
            return Err(ReadError::at(path, ReadErrorKind::XsiType(format!("{other} is not a request type"))));
        }
    };
    s.done()?;
    if absender != *anfrage.absender() {
        return Err(ReadError::at(&format!("{path}/Absender"), ReadErrorKind::Absender(anfrage.sender().as_str())));
    }
    Ok(anfrage)
}

fn vorabstimmung_body(s: &mut Seq<'_>, ty: &str, ekp: Ekp) -> Result<Vorabstimmung, ReadError> {
    let id = s.val("VorabstimmungsId")?;
    let kundenwunschtermin = s.date("Kundenwunschtermin")?;
    let endkunde = kunde(s.seq("Endkunde")?)?;
    let weitere_anschlussinhaber = s
        .many("WeitereAnschlussinhaber", 0, max::MANY)?
        .into_iter()
        .map(|(node, path)| seq(node, path).and_then(kunde))
        .collect::<Result<_, _>>()?;
    let projekt = match s.opt_seq("ProjektID")? {
        Some(mut p) => {
            let projekt =
                ProjektId { projektkenner: p.val("projektkenner")?, kopplungskenner: p.opt_val("kopplungskenner")? };
            p.done()?;
            Some(projekt)
        }
        None => None,
    };
    let fall = match ty {
        types::KUE_MRN => {
            let standort = standort(s.seq("Standort")?)?;
            let mut r = s.seq("RufnummernPortierung")?;
            let rufnummernportierung = rufnummernportierung(&mut r)?;
            r.done()?;
            VorabstimmungsFall::KueMrn { standort, rufnummernportierung }
        }
        types::KUE_ORN => {
            let standort = standort(s.seq("Standort")?)?;
            let anschlussidentifikation = s.opt_seq("Anschlussidentifikation")?.map(onkz_rufnummer).transpose()?;
            VorabstimmungsFall::KueOrn { standort, anschlussidentifikation }
        }
        _ => {
            let mut r = s.seq("RufnummernPortierung")?;
            let rufnummernportierung = rufnummernportierung(&mut r)?;
            let pki_auf = r.val("PortierungskennungPKIauf")?;
            r.done()?;
            VorabstimmungsFall::Rrnp { rufnummernportierung, pki_auf }
        }
    };
    Ok(Vorabstimmung { ekp, id, kundenwunschtermin, endkunde, weitere_anschlussinhaber, projekt, fall })
}

fn bezug(s: &mut Seq<'_>) -> Result<Bezug, ReadError> {
    let mut d = s.seq("VorabstimmungsDaten")?;
    let bezug = if d.peek("VorabstimmungsIdRef") {
        Bezug::Id(d.val("VorabstimmungsIdRef")?)
    } else {
        let (node, path) = d.req("Originalanfrage")?;
        match anfrage(node, &path)? {
            Anfrage::Vorabstimmung(va) => Bezug::Originalanfrage(Box::new(va)),
            _ => {
                return Err(ReadError::at(
                    &path,
                    ReadErrorKind::XsiType("Originalanfrage must be a Vorabstimmung".into()),
                ));
            }
        }
    };
    d.done()?;
    Ok(bezug)
}

fn position(p: &mut Seq<'_>) -> Result<Position, ReadError> {
    let (code, _) = p.text("Meldungscode")?;
    let code = code.trim();
    if code.is_empty() || code.chars().count() > 10 {
        return Err(ReadError::at(&p.path, ReadErrorKind::Invalid("Meldungscode (1–10 characters)")));
    }
    Ok(Position { code: Meldungscode::parse(code), text: p.val("Meldungstext")? })
}

fn positions<'a>(s: &mut Seq<'a>, ty: &str) -> Result<Vec<Seq<'a>>, ReadError> {
    s.many("Position", 1, max::MANY)?
        .into_iter()
        .map(|(node, path)| {
            let found = xsi(node, &path, NS_MESSAGE)?;
            if found != ty {
                return Err(ReadError::at(&path, ReadErrorKind::XsiType(format!("{found} where {ty} is required"))));
            }
            seq(node, path)
        })
        .collect()
}

fn plain_positions(s: &mut Seq<'_>, ty: &str) -> Result<Vec<Position>, ReadError> {
    positions(s, ty)?
        .into_iter()
        .map(|mut p| {
            let pos = position(&mut p)?;
            p.done()?;
            Ok(pos)
        })
        .collect()
}

/// `Wechseltermin`, `StornoIdRef`, `AenderungsIdRef`.
type Refs = (Option<Date>, Option<crate::StornoId>, Option<crate::AenderungsId>);

fn opt_refs(s: &mut Seq<'_>) -> Result<Refs, ReadError> {
    Ok((s.opt_date("Wechseltermin")?, s.opt_val("StornoIdRef")?, s.opt_val("AenderungsIdRef")?))
}

#[allow(clippy::too_many_lines)]
fn meldung(node: &Node, path: &str) -> Result<Meldung, ReadError> {
    let ty = xsi(node, path, NS_MESSAGE)?.to_owned();
    let mut s = Seq::new(node, path.to_owned())?;
    let absender = carrier(&mut s, "Absender")?;
    let vorabstimmungs_id = s.val("VorabstimmungsIdRef")?;
    let pos_ty = match ty.as_str() {
        types::RUEM_VA => types::POS_RUEM_VA,
        types::ABBM => types::POS_ABBM,
        types::ERLM => types::POS_ERLM,
        types::AKM_TR => types::POS_AKM_TR,
        types::ABBM_TR => types::POS_ABBM_TR,
        other => {
            return Err(ReadError::at(path, ReadErrorKind::XsiType(format!("{other} is not a message type"))));
        }
    };
    let pos = positions(&mut s, pos_ty)?;
    let ekp = ekp(&mut s)?;
    let geschaeftsfall = code_enum(&mut s, "Geschaeftsfall", "Geschaeftsfall", Geschaeftsfall::from_code)?;
    let kopf = MeldungKopf { absender, vorabstimmungs_id, ekp, geschaeftsfall };
    let meldung = match ty.as_str() {
        types::RUEM_VA => {
            let positionen = pos
                .into_iter()
                .map(|mut p| {
                    let position = position(&mut p)?;
                    let adresse_abweichend = match p.opt_seq("AdresseAbweichend")? {
                        Some(mut a) => {
                            let v = if a.peek("strassenname") {
                                AdresseAbweichend::Strassenname(a.val("strassenname")?)
                            } else if a.peek("postleitzahl") {
                                AdresseAbweichend::Postleitzahl(a.val("postleitzahl")?)
                            } else if a.peek("ort") {
                                AdresseAbweichend::Ort(a.val("ort")?)
                            } else {
                                AdresseAbweichend::Hausnummer(a.val("hausnummer")?)
                            };
                            a.done()?;
                            Some(v)
                        }
                        None => None,
                    };
                    p.done()?;
                    Ok(PositionRuemVa { position, adresse_abweichend })
                })
                .collect::<Result<_, ReadError>>()?;
            let ressourcen = s
                .many("Ressource", 0, max::MANY)?
                .into_iter()
                .map(|(node, path)| {
                    let mut r = Seq::new(node, path)?;
                    let res = TechnischeRessource {
                        vertragsnummer: r.opt_val("Vertragsnummer")?,
                        line_id: r.opt_val("LineID")?,
                        identifizierer: r.opt_val("Identifizierer")?,
                        kennung_tnb_abg: r.opt_val("KennungTNBabg")?,
                    };
                    r.done()?;
                    Ok(res)
                })
                .collect::<Result<_, ReadError>>()?;
            let technologie = s.opt_val("Technologie")?;
            let wechseltermin = s.date("Wechseltermin")?;
            let rufnummernportierung = s.opt_seq("RufnummernPortierung")?.map(portierte).transpose()?;
            Meldung::RuemVa(RuemVa { kopf, positionen, ressourcen, technologie, wechseltermin, rufnummernportierung })
        }
        types::ABBM => {
            let positionen = pos
                .into_iter()
                .map(|mut p| {
                    let position = position(&mut p)?;
                    let rufnummern = p
                        .many("Rufnummer", 0, max::NUMBERS)?
                        .into_iter()
                        .map(|(node, path)| {
                            leaf(node, &path)?.parse().map_err(|e| ReadError::at(&path, ReadErrorKind::Value(e)))
                        })
                        .collect::<Result<_, _>>()?;
                    p.done()?;
                    Ok(PositionAbbm { position, rufnummern })
                })
                .collect::<Result<_, ReadError>>()?;
            let (wechseltermin, storno_id, aenderungs_id) = opt_refs(&mut s)?;
            let begruendung = s.opt_val("Begruendung")?;
            Meldung::Abbm(Abbm { kopf, positionen, wechseltermin, storno_id, aenderungs_id, begruendung })
        }
        types::ERLM => {
            let positionen = finish_positions(pos)?;
            let (wechseltermin, storno_id, aenderungs_id) = opt_refs(&mut s)?;
            Meldung::Erlm(Erlm { kopf, positionen, wechseltermin, storno_id, aenderungs_id })
        }
        types::AKM_TR => {
            let positionen = finish_positions(pos)?;
            let ressourcenuebernahme = s.boolean("Ressourcenuebernahme")?;
            let uebernahme_leitung = s
                .many("UebernahmeLeitung", 0, max::MANY)?
                .into_iter()
                .map(|(node, path)| {
                    let mut l = Seq::new(node, path)?;
                    let v = if l.peek("Vertragsnummer") {
                        UebernahmeLeitung::Vertragsnummer(l.val("Vertragsnummer")?)
                    } else {
                        UebernahmeLeitung::LineId(l.val("LineID")?)
                    };
                    l.done()?;
                    Ok(v)
                })
                .collect::<Result<_, ReadError>>()?;
            let sicherer_hafen = s.boolean("SichererHafen")?;
            let pki_auf = s.opt_val("PortierungskennungPKIauf")?;
            Meldung::AkmTr(AkmTr {
                kopf,
                positionen,
                ressourcenuebernahme,
                uebernahme_leitung,
                sicherer_hafen,
                pki_auf,
            })
        }
        _ => Meldung::AbbmTr(AbbmTr { kopf, positionen: finish_positions(pos)? }),
    };
    s.done()?;
    Ok(meldung)
}

fn finish_positions(pos: Vec<Seq<'_>>) -> Result<Vec<Position>, ReadError> {
    pos.into_iter()
        .map(|mut p| {
            let position = position(&mut p)?;
            p.done()?;
            Ok(position)
        })
        .collect()
}

fn portierte(mut s: Seq<'_>) -> Result<PortierteRufnummern, ReadError> {
    let r = if let Some(mut b) = s.opt_seq("portierungRufnummernbloecke")? {
        let kopf = anlagenkopf(&mut b)?;
        let mut bloecke = Vec::new();
        for (node, path) in b.many("zuPortierenderRufnummernblock", 1, max::BLOCKS)? {
            let mut x = Seq::new(node, path)?;
            let blk = block(&mut x)?;
            let pki = x.val("PortierungskennungPKIabg")?;
            x.done()?;
            bloecke.push((blk, pki));
        }
        b.done()?;
        PortierteRufnummern::Bloecke { kopf, bloecke }
    } else {
        let mut r = s.seq("portierungRufnummern")?;
        let mut list = Vec::new();
        for (node, path) in r.many("zuPortierendeOnkzRnr", 1, max::NUMBERS)? {
            let mut x = Seq::new(node, path)?;
            let n = OnkzRufnummer { onkz: x.val("ONKZ")?, rufnummer: x.val("rufnummer")? };
            let pki = x.val("PortierungskennungPKIabg")?;
            x.done()?;
            list.push((n, pki));
        }
        r.done()?;
        PortierteRufnummern::Rufnummern(list)
    };
    s.done()?;
    Ok(r)
}

fn teq(mut s: Seq<'_>) -> Result<Teq, ReadError> {
    let absender = carrier(&mut s, "Absender")?;
    let vorabstimmungs_id = s.val("VorabstimmungsIdRef")?;
    let positionen = plain_positions(&mut s, types::POS_TEQ)?;
    let storno_id = s.opt_val("StornoIdRef")?;
    let aenderungs_id = s.opt_val("AenderungsIdRef")?;
    s.done()?;
    Ok(Teq { absender, vorabstimmungs_id, positionen, storno_id, aenderungs_id })
}
