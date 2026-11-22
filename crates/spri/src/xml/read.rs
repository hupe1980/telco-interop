//! Reading payloads from untrusted bytes.
//!
//! The input is parsed into a bounded element tree by the shared hardened parser (no DTD,
//! only the five predefined entities, limits on size, depth, element count, text length and
//! attachment size), then mapped onto the model in the schema's element order. Every failure
//! names the element path and maps to the receipt code a receiver answers with
//! ([`ReadError::teq_code`]): `0993` for an unsupported major release, `0995` for everything
//! else (S/PRI 4.4 §10.4, §11.2.3.1; AH4S/PRI 2.9.5 §6.3).

use core::fmt;

use carrier_core::SignaturId;
use carrier_core::xml::{Limits, Node, Seq, XmlError, XmlErrorKind, leaf, parse_zeitstempel, unwrap_soap};
use jiff::civil::Time;

use super::base64::{self, Base64Error};
use super::{NS_ENVELOPE, NS_FTTX, NS_MESSAGE, NS_ORDER, default_limits, max, meldungstyp_type, types};
use crate::model::{
    Abbm, AbbmPv, Abm, AbmPv, AenderungPosition, AkmPv, Anlage, Ansprechpartner, AnsprechpartnerAuftrag,
    AnsprechpartnerMitRolle, Auftrag, Auftraggeber, Auftragskenner, Auftragstyp, Bereitstellung, Control, Diagnose,
    DiagnoseErgebnis, Entm, EntmPv, EntstoerErgebnis, Entstoerung, Erlm, ErlmK, ErlmPv, Firma, FttxAnschluss,
    FttxBereitstellung, FttxProviderwechsel, GebaeudeteilAngabe, Geschaeftsfall, Inhaber, Kdle, Kenner,
    KontaktEndkunde, Kuendigung, Leistungsaenderung, LeistungsmerkmalAenderung, MAX_AUFTRAGSKLAMMER, Meldung,
    Meldungsinhalt, Montageleistung, OnkzRufnummer, Ort, Payload, Person, PortierungAnschluss, Position, PositionAbm,
    PositionQeb, PositionWita, Produkt, ProjektId, Providerwechsel, ProviderwechselAnkuendigung,
    ProviderwechselBestaetigung, ProviderwechselRueckmeldung, Qeb, Reklamation, Release, RuemPv, RufnummernPortierung,
    Rufnummernblock, StandortA, StandortVersand, Strasse, Tbk, Teq, Termin, Terminanforderung, VersandinfosOnt,
    Vormieter, VormieterPerson, Vzm, WitaExtendedInfos, WitaInfos, Wunschtermin, ZwmAg, ZwmLe,
};
use crate::{
    Aenderungskennzeichen, Aktionscode, Anlagentyp, Anrede, AnredeFirma, AnsprechpartnerRolle, Dokumenttyp,
    Entstoerergebnis, GeschaeftsfallArt, GeschaeftsfallCode, JaNein, Meldungscode, Meldungstyp, UebergabepunktKunde,
    ValueError, Zeitfenster,
};

/// Why a payload could not be read.
///
/// The message names the element path and, where the parser knows it, the byte offset; it
/// never repeats element content, which may be personal data.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub struct ReadError {
    /// Element path, e.g. `annehmenAuftragRequest/auftrag/geschaeftsfall/lineId`.
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
    /// An attachment is larger than [`Limits::max_attachment`] after decoding.
    #[error("attachment larger than {limit} bytes")]
    AttachmentTooLarge {
        /// The limit in bytes.
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
    /// A value of an enumeration, date, number, boolean or timestamp that does not parse.
    #[error("invalid {0}")]
    Invalid(&'static str),
    /// A major release this library does not implement.
    #[error("major release {0} is not supported (S/PRI 4.x is 04)")]
    Version(String),
}

impl ReadError {
    /// The receipt code a receiver answers this input with (S/PRI 4.4 §11.2.3.1: schema,
    /// signature and version are checked on receipt): `0993` for an unsupported major release,
    /// `0995` otherwise.
    #[must_use]
    pub const fn teq_code(&self) -> Meldungscode {
        match self.kind {
            ReadErrorKind::Version(_) => Meldungscode::VERSION_FEHLERHAFT,
            _ => Meldungscode::DATENSTRUKTUR_UNZULAESSIG,
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
            XmlErrorKind::TextTooLong { limit } => ReadErrorKind::TextTooLong { limit },
            XmlErrorKind::AttachmentTooLarge { limit } => ReadErrorKind::AttachmentTooLarge { limit },
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
    /// Reads a payload with [`default_limits`]: the bare payload element or a SOAP 1.1
    /// envelope whose body holds it.
    ///
    /// # Errors
    ///
    /// [`ReadError`]; [`ReadError::teq_code`] gives the receipt code to answer with.
    pub fn from_xml(input: &[u8]) -> Result<Self, ReadError> {
        Self::from_xml_with(input, &default_limits())
    }

    /// Reads a payload with explicit limits.
    ///
    /// # Errors
    ///
    /// As [`Payload::from_xml`].
    pub fn from_xml_with(input: &[u8], limits: &Limits) -> Result<Self, ReadError> {
        let root = carrier_core::xml::parse(input, limits, &["inhalt"])?;
        let payload = unwrap_soap(&root)?;
        Reader { max_attachment: limits.max_attachment }.payload(payload)
    }
}

type R<T> = Result<T, ReadError>;

fn seq(node: &Node, path: String) -> R<Seq<'_>> {
    Ok(Seq::new(node, path)?)
}

/// Each child `name` (between `min` and `max`) through `f`.
fn each<T>(
    s: &mut Seq<'_>,
    name: &'static str,
    min: usize,
    max: usize,
    mut f: impl FnMut(Seq<'_>) -> R<T>,
) -> R<Vec<T>> {
    s.many(name, min, max)?.into_iter().map(|(node, path)| f(seq(node, path)?)).collect()
}

fn opt<T>(s: &mut Seq<'_>, name: &'static str, f: impl FnOnce(Seq<'_>) -> R<T>) -> R<Option<T>> {
    match s.opt_seq(name)? {
        Some(inner) => f(inner).map(Some),
        None => Ok(None),
    }
}

fn req<T>(s: &mut Seq<'_>, name: &'static str, f: impl FnOnce(Seq<'_>) -> R<T>) -> R<T> {
    f(s.seq(name)?)
}

/// Runs `f` on the cursor, then requires that nothing is left.
fn finish<T>(mut s: Seq<'_>, f: impl FnOnce(&mut Seq<'_>) -> R<T>) -> R<T> {
    let v = f(&mut s)?;
    s.done()?;
    Ok(v)
}

fn xsi_in<'a>(node: &'a Node, path: &str, ns: &str) -> R<&'a str> {
    Ok(node.xsi(path, ns)?)
}

fn expect_xsi(node: &Node, path: &str, ns: &str, ty: &str) -> R<()> {
    let found = xsi_in(node, path, ns)?;
    if found == ty {
        Ok(())
    } else {
        Err(ReadError::at(path, ReadErrorKind::XsiType(format!("{found} where {ty} is required"))))
    }
}

fn zeitfenster(s: &mut Seq<'_>) -> R<Option<Zeitfenster>> {
    match s.opt("zeitfenster") {
        Some((node, path)) => Ok(Some(
            Zeitfenster::new(leaf(node, &path)?.trim()).map_err(|e| ReadError::at(&path, ReadErrorKind::Value(e)))?,
        )),
        None => Ok(None),
    }
}

/// `xsd:time`: `hh:mm:ss` with optional fraction; a time-zone suffix is accepted and dropped.
fn time(s: &mut Seq<'_>, name: &'static str) -> R<Option<Time>> {
    let Some((node, path)) = s.opt(name) else { return Ok(None) };
    let text = leaf(node, &path)?.trim();
    let invalid = || ReadError::at(&path, ReadErrorKind::Invalid("time (hh:mm:ss)"));
    let core = text.strip_suffix('Z').unwrap_or(text);
    let core = match core.len().checked_sub(6).and_then(|i| core.get(i..).map(|tz| (i, tz))) {
        Some((i, tz)) if (tz.starts_with('+') || tz.starts_with('-')) && tz.get(3..4) == Some(":") => {
            core.get(..i).unwrap_or(core)
        }
        _ => core,
    };
    if core.len() < 8 || core.get(2..3) != Some(":") || core.get(5..6) != Some(":") {
        return Err(invalid());
    }
    core.parse().map(Some).map_err(|_| invalid())
}

struct Reader {
    max_attachment: usize,
}

impl Reader {
    fn payload(&self, root: &Node) -> R<Payload> {
        let name = root.name.as_str();
        if root.ns.as_deref() != Some(NS_ENVELOPE) {
            return Err(ReadError::at(
                name,
                ReadErrorKind::Unexpected(format!("{name} (expected an S/PRI payload in {NS_ENVELOPE})")),
            ));
        }
        let mut s = seq(root, name.to_owned())?;
        let control = finish(s.seq("control")?, control)?;
        let payload = match name {
            "annehmenAuftragRequest" => Payload::AuftragRequest {
                control,
                auftrag: req(&mut s, "auftrag", |a| finish(a, |a| self.auftrag(a)))?,
            },
            "annehmenMeldungRequest" => Payload::MeldungRequest {
                control,
                meldung: req(&mut s, "meldung", |m| finish(m, |m| self.meldung(m)))?,
            },
            "annehmenAuftragResponse" => Payload::AuftragResponse { control, quittung: req(&mut s, "quittung", teq)? },
            "annehmenMeldungResponse" => Payload::MeldungResponse { control, quittung: req(&mut s, "quittung", teq)? },
            other => return Err(ReadError::at(other, ReadErrorKind::Unexpected(other.to_owned()))),
        };
        s.done()?;
        Ok(payload)
    }

    fn anlage(&self, s: &mut Seq<'_>) -> R<Option<Anlage>> {
        opt(s, "anlage", |a| {
            finish(a, |a| {
                let dateiname = a.val("dateiname")?;
                let dateityp = a.code("dateityp", "dateityp (DokumenttypType)", Dokumenttyp::from_code)?;
                let beschreibung = a.opt_val("beschreibung")?;
                let (text, path) = a.text("inhalt")?;
                let inhalt = base64::decode(text, self.max_attachment).map_err(|e| match e {
                    Base64Error::Invalid => ReadError::at(&path, ReadErrorKind::Invalid("base64")),
                    Base64Error::TooLarge => {
                        ReadError::at(&path, ReadErrorKind::AttachmentTooLarge { limit: self.max_attachment })
                    }
                })?;
                let anlagentyp = a.code("anlagentyp", "anlagentyp (AnlagentypType)", Anlagentyp::from_code)?;
                Ok(Anlage { dateiname, dateityp, beschreibung, inhalt, anlagentyp })
            })
        })
    }

    // -- orders ----------------------------------------------------------------------------

    fn auftrag(&self, s: &mut Seq<'_>) -> R<Auftrag> {
        let externe_auftragsnummer = s.val("externeAuftragsnummer")?;
        let auftraggeber = req(s, "auftraggeber", |a| finish(a, read_auftraggeber))?;
        let besteller = opt(s, "besteller", |a| finish(a, read_auftraggeber))?;
        let (node, path) = s.req("geschaeftsfall")?;
        let geschaeftsfall = self.geschaeftsfall(node, &path)?;
        let geschaeftsfall_art =
            s.code("geschaeftsfallArt", "geschaeftsfallArt (GeschaeftsfallArtType)", GeschaeftsfallArt::from_code)?;
        let aenderungskennzeichen = s.code(
            "aenderungskennzeichen",
            "aenderungskennzeichen (AenderungskennzeichenType)",
            Aenderungskennzeichen::from_code,
        )?;
        let termin_neu = s.opt_date("terminNeu")?;
        let kenner = opt(s, "kenner", |k| finish(k, kenner))?;
        Ok(Auftrag {
            externe_auftragsnummer,
            auftraggeber,
            besteller,
            geschaeftsfall,
            geschaeftsfall_art,
            aenderungskennzeichen,
            termin_neu,
            kenner,
        })
    }

    fn geschaeftsfall(&self, node: &Node, path: &str) -> R<Geschaeftsfall> {
        let ty = xsi_in(node, path, NS_ORDER)?.to_owned();
        let s = seq(node, path.to_owned())?;
        finish(s, |s| {
            Ok(match ty.as_str() {
                types::NEU => Geschaeftsfall::Neu(self.bereitstellung(s)?),
                types::KUE_AG => Geschaeftsfall::KueAg(self.kuendigung(s)?),
                types::LAE => {
                    let (vertragsnummer, line_id, anlage, ansprechpartner, wunschtermin) = self.change_head(s)?;
                    let (produkt, positionen) = req(s, "auftragsposition", |p| finish(p, |p| aenderung(p, 0)))?;
                    Geschaeftsfall::Lae(Leistungsaenderung {
                        vertragsnummer,
                        line_id,
                        anlage,
                        ansprechpartner,
                        wunschtermin,
                        produkt,
                        positionen,
                    })
                }
                types::AEN_LMAE => {
                    let (vertragsnummer, line_id, anlage, ansprechpartner, wunschtermin) = self.change_head(s)?;
                    let (produkt, positionen) = req(s, "auftragsposition", |p| finish(p, |p| aenderung(p, 1)))?;
                    Geschaeftsfall::AenLmae(LeistungsmerkmalAenderung {
                        vertragsnummer,
                        line_id,
                        anlage,
                        ansprechpartner,
                        wunschtermin,
                        produkt,
                        positionen,
                    })
                }
                types::EST => Geschaeftsfall::Est(self.entstoerung(s)?),
                types::PV => Geschaeftsfall::Pv(self.providerwechsel(s)?),
                types::GET => Geschaeftsfall::Get(diagnose(s)?),
                types::SET => Geschaeftsfall::Set(diagnose(s)?),
                other => {
                    return Err(ReadError::at(
                        path,
                        ReadErrorKind::XsiType(format!("{other} is not a business case type")),
                    ));
                }
            })
        })
    }

    #[allow(clippy::type_complexity)]
    fn change_head(
        &self,
        s: &mut Seq<'_>,
    ) -> R<(crate::Vertragsnummer, crate::LineId, Option<Anlage>, AnsprechpartnerAuftrag, Wunschtermin)> {
        let vertragsnummer = s.val("vertragsnummer")?;
        let line_id = s.val("lineId")?;
        let anlage = self.anlage(s)?;
        let ansprechpartner = req(s, "ansprechpartner", ansprechpartner_auftrag)?;
        let wunschtermin = termine(s)?;
        Ok((vertragsnummer, line_id, anlage, ansprechpartner, wunschtermin))
    }

    fn bereitstellung(&self, s: &mut Seq<'_>) -> R<Bereitstellung> {
        let vorabstimmungs_id = s.opt_val("vorabstimmungId")?;
        let anlage = self.anlage(s)?;
        let ansprechpartner = req(s, "ansprechpartner", ansprechpartner_auftrag)?;
        let wunschtermin = termine(s)?;
        let (produkt, fttx, positionen) = req(s, "auftragsposition", |p| {
            finish(p, |p| {
                let produkt = read_produkt(p)?;
                let (node, path) = p.req("geschaeftsfallProdukt")?;
                expect_xsi(node, &path, NS_FTTX, types::FTTX_NEU)?;
                let fttx = finish(seq(node, path)?, |f| {
                    let standort_a = req(f, "standortA", |x| finish(x, standort_a))?;
                    let standort_versand = opt(f, "standortVersand", |x| finish(x, standort_versand))?;
                    let vormieter = opt(f, "vormieter", |x| finish(x, vormieter))?;
                    let montageleistung = opt(f, "montageleistung", |x| finish(x, montageleistung))?;
                    let wita = wita_extended(f)?;
                    Ok(FttxBereitstellung { standort_a, standort_versand, vormieter, montageleistung, wita })
                })?;
                let positionen = each(p, "position", 0, max::UNBOUNDED, |x| finish(x, read_produkt))?;
                Ok((produkt, fttx, positionen))
            })
        })?;
        Ok(Bereitstellung { vorabstimmungs_id, anlage, ansprechpartner, wunschtermin, produkt, fttx, positionen })
    }

    fn kuendigung(&self, s: &mut Seq<'_>) -> R<Kuendigung> {
        let vertragsnummer = s.val("vertragsnummer")?;
        let line_id = s.val("lineId")?;
        let vorabstimmungs_id = s.opt_val("vorabstimmungId")?;
        let anlage = self.anlage(s)?;
        let ansprechpartner = req(s, "ansprechpartner", ansprechpartner_auftrag)?;
        let wunschtermin = termine(s)?;
        let fttx = opt(s, "auftragsposition", |p| {
            finish(p, |p| {
                let (node, path) = p.req("geschaeftsfallProdukt")?;
                expect_xsi(node, &path, NS_FTTX, types::FTTX_KUE)?;
                finish(seq(node, path)?, wita_infos)
            })
        })?;
        Ok(Kuendigung { vertragsnummer, line_id, vorabstimmungs_id, anlage, ansprechpartner, wunschtermin, fttx })
    }

    fn entstoerung(&self, s: &mut Seq<'_>) -> R<Entstoerung> {
        let vertragsnummer = s.val("vertragsnummer")?;
        let line_id = s.val("lineId")?;
        let anlage = self.anlage(s)?;
        let ansprechpartner = req(s, "ansprechpartner", ansprechpartner_auftrag)?;
        let kontakt_endkunde = req(s, "kontaktEndkunde", |k| {
            finish(k, |k| {
                Ok(KontaktEndkunde {
                    anrede: anrede(k)?,
                    vorname: k.val("vorname")?,
                    nachname: k.val("nachname")?,
                    telefonnummer: k.opt_val("telefonnummer")?,
                    mobilfunknummer: k.opt_val("mobilfunknummer")?,
                    faxnummer: k.opt_val("faxnummer")?,
                    emailadresse: k.opt_val("emailadresse")?,
                    kontaktaufnahme_erwuenscht: k.boolean("kontaktaufnahmeErwuenscht")?,
                })
            })
        })?;
        let wunschtermin = termine(s)?;
        let stoerungs_nr_auftraggeber = s.val("stoerungsNrAuftraggeber")?;
        let reklamation = opt(s, "reklamation", |r| {
            finish(r, |r| {
                Ok(Reklamation {
                    stoerungs_nr_leistungserbringer: r.val("stoerungsNrLeistungserbringer")?,
                    reklamationsgrund: r.val("reklamationsgrund")?,
                })
            })
        })?;
        let vorpruefung_erfolgt = s.boolean("vorpruefungErfolgt")?;
        let stoerungsbeschreibung = s.val("stoerungsbeschreibung")?;
        let expressentstoerung = s.opt_boolean("expressentstoerung")?;
        Ok(Entstoerung {
            vertragsnummer,
            line_id,
            anlage,
            ansprechpartner,
            kontakt_endkunde,
            wunschtermin,
            stoerungs_nr_auftraggeber,
            reklamation,
            vorpruefung_erfolgt,
            stoerungsbeschreibung,
            expressentstoerung,
        })
    }

    fn providerwechsel(&self, s: &mut Seq<'_>) -> R<Providerwechsel> {
        let line_id = s.val("lineId")?;
        let vorabstimmungs_id = s.val("vorabstimmungId")?;
        let anlage = self.anlage(s)?;
        let ansprechpartner = req(s, "ansprechpartner", ansprechpartner_auftrag)?;
        let wunschtermin = termine(s)?;
        let (produkt, fttx, positionen) = req(s, "auftragsposition", |p| {
            finish(p, |p| {
                let produkt = read_produkt(p)?;
                let (node, path) = p.req("geschaeftsfallProdukt")?;
                expect_xsi(node, &path, NS_FTTX, types::FTTX_PV)?;
                let fttx = finish(seq(node, path)?, |f| {
                    Ok(FttxProviderwechsel {
                        standort_a: req(f, "standortA", |x| finish(x, standort_a))?,
                        montageleistung: opt(f, "montageleistung", |x| finish(x, montageleistung))?,
                    })
                })?;
                let positionen = each(p, "position", 0, max::UNBOUNDED, |x| finish(x, read_produkt))?;
                Ok((produkt, fttx, positionen))
            })
        })?;
        Ok(Providerwechsel {
            line_id,
            vorabstimmungs_id,
            anlage,
            ansprechpartner,
            wunschtermin,
            produkt,
            fttx,
            positionen,
        })
    }

    // -- messages --------------------------------------------------------------------------

    fn meldung(&self, s: &mut Seq<'_>) -> R<Meldung> {
        let auftragstyp = req(s, "auftragstyp", |a| {
            finish(a, |a| {
                Ok(Auftragstyp {
                    geschaeftsfall: a.code(
                        "geschaeftsfall",
                        "geschaeftsfall (GeschaeftsfallMeldungType)",
                        GeschaeftsfallCode::from_code,
                    )?,
                    aenderungskennzeichen: a.code(
                        "aenderungsKennzeichen",
                        "aenderungsKennzeichen (AenderungskennzeichenType)",
                        Aenderungskennzeichen::from_code,
                    )?,
                    geschaeftsfallart: a.code(
                        "geschaeftsfallart",
                        "geschaeftsfallart (GeschaeftsfallArtType)",
                        GeschaeftsfallArt::from_code,
                    )?,
                })
            })
        })?;
        let (node, path) = s.req("meldungstyp")?;
        let found = xsi_in(node, &path, NS_MESSAGE)?;
        let typ = Meldungstyp::ALL
            .iter()
            .copied()
            .filter(|t| *t != Meldungstyp::Teq)
            .find(|t| meldungstyp_type(*t) == found)
            .ok_or_else(|| ReadError::at(&path, ReadErrorKind::XsiType(format!("{found} is not a message type"))))?;
        let inhalt = finish(seq(node, path)?, |m| {
            let mut a = m.seq("meldungsattribute")?;
            let inhalt = self.inhalt(typ, &mut a, m)?;
            a.done()?;
            Ok(inhalt)
        })?;
        Ok(Meldung { auftragstyp, inhalt })
    }

    /// Reads `meldungsattribute` (`a`, still open) and then `meldungspositionen` from `m`.
    #[allow(clippy::too_many_lines)]
    fn inhalt(&self, typ: Meldungstyp, a: &mut Seq<'_>, m: &mut Seq<'_>) -> R<Meldungsinhalt> {
        let besteller = |a: &mut Seq<'_>| -> R<_> { Ok(a.opt_val("auftraggebernummerBesteller")?) };
        let va = |a: &mut Seq<'_>| -> R<_> { Ok(a.opt_val("vorabstimmungId")?) };
        Ok(match typ {
            Meldungstyp::Qeb => {
                let externe_auftragsnummer = a.val("externeAuftragsnummer")?;
                let auftraggebernummer = a.val("auftraggebernummer")?;
                let auftraggebernummer_besteller = besteller(a)?;
                let vorabstimmungs_id = va(a)?;
                let stoerungs_nr_leistungserbringer = a.opt_val("stoerungsNrLeistungserbringer")?;
                let positionen = positions(m, |p| {
                    let position = position(p)?;
                    let ansprechpartner_leistungserbringer = positionsattribute(p)?;
                    Ok(PositionQeb { position, ansprechpartner_leistungserbringer })
                })?;
                Meldungsinhalt::Qeb(Qeb {
                    externe_auftragsnummer,
                    auftraggebernummer,
                    auftraggebernummer_besteller,
                    vorabstimmungs_id,
                    stoerungs_nr_leistungserbringer,
                    positionen,
                })
            }
            Meldungstyp::Abm => {
                let vertragsnummer = a.val("vertragsnummer")?;
                let externe_auftragsnummer = a.val("externeAuftragsnummer")?;
                let auftraggebernummer = a.val("auftraggebernummer")?;
                let auftraggebernummer_besteller = besteller(a)?;
                let vorabstimmungs_id = va(a)?;
                let verbindlicher_liefertermin = req(a, "verbindlicherLiefertermin", |t| finish(t, wunschtermin))?;
                let abgebender_provider = opt(a, "abgebenderProvider", |r| finish(r, rueckmeldung))?;
                let anschluss = req(a, "anschluss", |x| finish(x, fttx_anschluss))?;
                let positionen = positions(m, |p| {
                    let position = wita_position(p)?;
                    let ansprechpartner_leistungserbringer = positionsattribute(p)?;
                    Ok(PositionAbm { position, ansprechpartner_leistungserbringer })
                })?;
                Meldungsinhalt::Abm(Abm {
                    vertragsnummer,
                    externe_auftragsnummer,
                    auftraggebernummer,
                    auftraggebernummer_besteller,
                    vorabstimmungs_id,
                    verbindlicher_liefertermin,
                    abgebender_provider,
                    anschluss,
                    positionen,
                })
            }
            Meldungstyp::Abbm => Meldungsinhalt::Abbm(Abbm {
                vertragsnummer: a.opt_val("vertragsnummer")?,
                externe_auftragsnummer: a.val("externeAuftragsnummer")?,
                auftraggebernummer: a.val("auftraggebernummer")?,
                auftraggebernummer_besteller: besteller(a)?,
                vorabstimmungs_id: va(a)?,
                wiedervorlagetermin: a.opt_date("wiedervorlagetermin")?,
                abgebender_provider: opt(a, "abgebenderProvider", |r| finish(r, rueckmeldung))?,
                positionen: positions(m, wita_position)?,
            }),
            Meldungstyp::Erlm => Meldungsinhalt::Erlm(Erlm {
                vertragsnummer: a.opt_val("vertragsnummer")?,
                externe_auftragsnummer: a.val("externeAuftragsnummer")?,
                auftraggebernummer: a.val("auftraggebernummer")?,
                auftraggebernummer_besteller: besteller(a)?,
                vorabstimmungs_id: va(a)?,
                erledigungstermin: a.date("erledigungstermin")?,
                entstoer_ergebnis: opt(a, "entstoerErgebnis", |e| {
                    finish(e, |e| {
                        Ok(EntstoerErgebnis {
                            ergebnis: e.code("ergebnis", "ergebnis", Entstoerergebnis::from_code)?,
                            entstoer_zeitpunkt: e.opt_date("entstoerZeitpunkt")?,
                            anmerkung: e.opt_val("anmerkung")?,
                        })
                    })
                })?,
                anlage: self.anlage(a)?,
                diagnoseergebnisse: each(a, "dignoseergebnis", 0, max::UNBOUNDED, |d| {
                    finish(d, |d| Ok(DiagnoseErgebnis { name: d.val("name")?, wert: d.val("wert")? }))
                })?,
                positionen: positions(m, wita_position)?,
            }),
            Meldungstyp::Entm => Meldungsinhalt::Entm(Entm {
                vertragsnummer: a.val("vertragsnummer")?,
                externe_auftragsnummer: a.val("externeAuftragsnummer")?,
                auftraggebernummer: a.val("auftraggebernummer")?,
                auftraggebernummer_besteller: besteller(a)?,
                vorabstimmungs_id: va(a)?,
                entgelttermin: a.date("entgelttermin")?,
                anlage: self.anlage(a)?,
                positionen: positions(m, position)?,
            }),
            Meldungstyp::Vzm => Meldungsinhalt::Vzm(Vzm {
                vertragsnummer: a.opt_val("vertragsnummer")?,
                externe_auftragsnummer: a.val("externeAuftragsnummer")?,
                auftraggebernummer: a.val("auftraggebernummer")?,
                auftraggebernummer_besteller: besteller(a)?,
                vorabstimmungs_id: va(a)?,
                verzoegerungstermin: a.opt_date("verzoegerungstermin")?,
                verzoegerungsgrund: a.opt_val("verzoegerungsgrund")?,
                positionen: positions(m, wita_position)?,
            }),
            Meldungstyp::Tam | Meldungstyp::Mtam => {
                let t = Terminanforderung {
                    vertragsnummer: a.val("vertragsnummer")?,
                    externe_auftragsnummer: a.val("externeAuftragsnummer")?,
                    auftraggebernummer: a.val("auftraggebernummer")?,
                    auftraggebernummer_besteller: besteller(a)?,
                    vorabstimmungs_id: va(a)?,
                    positionen: positions(m, wita_position)?,
                };
                if typ == Meldungstyp::Tam { Meldungsinhalt::Tam(t) } else { Meldungsinhalt::Mtam(t) }
            }
            Meldungstyp::Kdle => Meldungsinhalt::Kdle(Kdle {
                vertragsnummer: a.val("vertragsnummer")?,
                externe_auftragsnummer: a.val("externeAuftragsnummer")?,
                auftraggebernummer: a.val("auftraggebernummer")?,
                abschaltungstermin: a.date("abschaltungstermin")?,
                abschaltungsgrund: a.opt_val("abschaltungsgrund")?,
                positionen: positions(m, position)?,
            }),
            Meldungstyp::ZwmLe => Meldungsinhalt::ZwmLe(ZwmLe {
                vertragsnummer: a.val("vertragsnummer")?,
                externe_auftragsnummer: a.val("externeAuftragsnummer")?,
                auftraggebernummer: a.val("auftraggebernummer")?,
                auftraggebernummer_besteller: besteller(a)?,
                vorabstimmungs_id: va(a)?,
                nachricht: a.val("nachricht")?,
                ansprechpartner: opt(a, "ansprechpartner", |x| finish(x, ansprechpartner_mit_rolle))?,
                anlage: self.anlage(a)?,
                versand_ont: opt(a, "versandONT", |v| {
                    finish(v, |v| {
                        let seriennummer_ont = v.val("seriennummerONT")?;
                        let (unternehmen, sendungsnummer) = req(v, "versand", |x| {
                            finish(x, |x| Ok((x.val("unternehmen")?, x.val("sendungsnummer")?)))
                        })?;
                        Ok(VersandinfosOnt { seriennummer_ont, unternehmen, sendungsnummer })
                    })
                })?,
                positionen: positions(m, position)?,
            }),
            Meldungstyp::ZwmAg => Meldungsinhalt::ZwmAg(ZwmAg {
                vertragsnummer: a.val("vertragsnummer")?,
                externe_auftragsnummer: a.val("externeAuftragsnummer")?,
                auftraggebernummer: a.val("auftraggebernummer")?,
                auftraggebernummer_besteller: besteller(a)?,
                nachricht: a.val("nachricht")?,
                ansprechpartner: opt(a, "ansprechpartner", |x| finish(x, ansprechpartner_mit_rolle))?,
                anlage: self.anlage(a)?,
                positionen: positions(m, position)?,
            }),
            Meldungstyp::TbkLe | Meldungstyp::TbkAg => {
                let t = Tbk {
                    vertragsnummer: a.val("vertragsnummer")?,
                    externe_auftragsnummer: a.val("externeAuftragsnummer")?,
                    auftraggebernummer: a.val("auftraggebernummer")?,
                    auftraggebernummer_besteller: besteller(a)?,
                    termin: req(a, "termin", |t| {
                        finish(t, |t| {
                            Ok(Termin { datum: t.date("datum")?, zeit: time(t, "zeit")?, zeitfenster: zeitfenster(t)? })
                        })
                    })?,
                    positionen: positions(m, position)?,
                };
                if typ == Meldungstyp::TbkLe { Meldungsinhalt::TbkLe(t) } else { Meldungsinhalt::TbkAg(t) }
            }
            Meldungstyp::AkmPv => Meldungsinhalt::AkmPv(AkmPv {
                vertragsnummer: a.val("vertragsnummer")?,
                externe_auftragsnummer: a.val("externeAuftragsnummer")?,
                auftraggebernummer: a.val("auftraggebernummer")?,
                vorabstimmungs_id: va(a)?,
                line_id: line(a)?,
                anlage: self.anlage(a)?,
                aufnehmender_provider: req(a, "aufnehmenderProvider", |p| {
                    finish(p, |p| {
                        Ok(ProviderwechselAnkuendigung {
                            providername: p.val("providername")?,
                            uebernahmedatum_geplant: p.date("uebernahmedatumGeplant")?,
                            antwortfrist: p.date("antwortfrist")?,
                        })
                    })
                })?,
                positionen: positions(m, position)?,
            }),
            Meldungstyp::RuemPv => Meldungsinhalt::RuemPv(RuemPv {
                vertragsnummer: a.val("vertragsnummer")?,
                externe_auftragsnummer: a.val("externeAuftragsnummer")?,
                auftraggebernummer: a.val("auftraggebernummer")?,
                vorabstimmungs_id: va(a)?,
                line_id: line(a)?,
                abgebender_provider: req(a, "abgebenderProvider", |r| finish(r, rueckmeldung))?,
                positionen: positions(m, position)?,
            }),
            Meldungstyp::AbmPv => Meldungsinhalt::AbmPv(AbmPv {
                vertragsnummer: a.val("vertragsnummer")?,
                externe_auftragsnummer: a.val("externeAuftragsnummer")?,
                auftraggebernummer: a.val("auftraggebernummer")?,
                line_id: line(a)?,
                aufnehmender_provider: req(a, "aufnehmenderProvider", |p| {
                    finish(p, |p| {
                        Ok(ProviderwechselBestaetigung {
                            providername: p.val("providername")?,
                            uebernahmedatum_verbindlich: p.date("uebernahmedatumVerbindlich")?,
                        })
                    })
                })?,
                positionen: positions(m, position)?,
            }),
            Meldungstyp::ErlmPv => Meldungsinhalt::ErlmPv(ErlmPv {
                vertragsnummer: a.val("vertragsnummer")?,
                externe_auftragsnummer: a.val("externeAuftragsnummer")?,
                auftraggebernummer: a.val("auftraggebernummer")?,
                erledigungstermin: a.date("erledigungstermin")?,
                positionen: positions(m, position)?,
            }),
            Meldungstyp::EntmPv => Meldungsinhalt::EntmPv(EntmPv {
                vertragsnummer: a.val("vertragsnummer")?,
                externe_auftragsnummer: a.val("externeAuftragsnummer")?,
                auftraggebernummer: a.val("auftraggebernummer")?,
                entgelttermin: a.date("entgelttermin")?,
                positionen: positions(m, position)?,
            }),
            Meldungstyp::AbbmPv => Meldungsinhalt::AbbmPv(AbbmPv {
                vertragsnummer: a.opt_val("vertragsnummer")?,
                externe_auftragsnummer: a.val("externeAuftragsnummer")?,
                auftraggebernummer: a.val("auftraggebernummer")?,
                positionen: positions(m, position)?,
            }),
            Meldungstyp::Teq => {
                return Err(ReadError::at(&m.path, ReadErrorKind::XsiType("TEQ is not a message type".into())));
            }
            Meldungstyp::ErlmK => Meldungsinhalt::ErlmK(ErlmK {
                vertragsnummer: a.val("vertragsnummer")?,
                externe_auftragsnummer: a.val("externeAuftragsnummer")?,
                auftraggebernummer: a.val("auftraggebernummer")?,
                auftraggebernummer_besteller: besteller(a)?,
                vorabstimmungs_id: va(a)?,
                positionen: positions(m, position)?,
            }),
        })
    }
}

// -- shared complex types ------------------------------------------------------------------

fn control(s: &mut Seq<'_>) -> R<Control> {
    let (text, path) = s.text("zeitstempel")?;
    let zeitstempel = parse_zeitstempel(text, &path)?;
    let (major_text, path) = s.text("majorRelease")?;
    let major = version(major_text).ok_or_else(|| ReadError::at(&path, ReadErrorKind::Invalid("majorRelease")))?;
    if major != Release::SPRI_4_4.major {
        return Err(ReadError::at(&path, ReadErrorKind::Version(major_text.trim().to_owned())));
    }
    let (minor_text, path) = s.text("minorRelease")?;
    let minor = version(minor_text).ok_or_else(|| ReadError::at(&path, ReadErrorKind::Invalid("minorRelease")))?;
    let signatur = req(s, "signaturId", |sig| {
        finish(sig, |sig| Ok(SignaturId { issuer: sig.val("issuer")?, serial: sig.val("serial")? }))
    })?;
    Ok(Control { zeitstempel, release: Release { major, minor }, signatur })
}

/// One or two digits (S/PRI 4.4 §10.4 „AN 2", digits only; the schema collapses whitespace).
fn version(text: &str) -> Option<u8> {
    let t = text.trim();
    (matches!(t.len(), 1 | 2) && t.bytes().all(|b| b.is_ascii_digit())).then(|| t.parse().ok()).flatten()
}

fn teq(s: Seq<'_>) -> R<Teq> {
    finish(s, |s| {
        let meldungstyp = s.code("meldungsTyp", "meldungsTyp (MeldungstypType)", Meldungstyp::from_code)?;
        let externe_auftragsnummer = s.opt_val("externeAuftragsnummer")?;
        let auftraggebernummer = s.opt_val("auftraggebernummer")?;
        let positionen = req(s, "meldungspositionen", |m| {
            finish(m, |m| each(m, "position", 1, max::UNBOUNDED, |p| finish(p, position)))
        })?;
        Ok(Teq { meldungstyp, externe_auftragsnummer, auftraggebernummer, positionen })
    })
}

fn read_auftraggeber(s: &mut Seq<'_>) -> R<Auftraggeber> {
    Ok(Auftraggeber { auftraggebernummer: s.val("auftraggebernummer")?, leistungsnummer: s.val("leistungsnummer")? })
}

fn kenner(s: &mut Seq<'_>) -> R<Kenner> {
    let projekt_id = opt(s, "projektId", |p| {
        finish(p, |p| {
            Ok(ProjektId { projektkenner: p.val("projektkenner")?, kopplungskenner: p.opt_val("kopplungskenner")? })
        })
    })?;
    let auftragskenner = opt(s, "auftragskenner", |a| {
        finish(a, |a| {
            let (text, path) = a.text("auftragsklammer")?;
            let auftragsklammer = text
                .trim()
                .trim_start_matches('+')
                .parse::<u64>()
                .ok()
                .filter(|n| *n <= MAX_AUFTRAGSKLAMMER)
                .ok_or_else(|| ReadError::at(&path, ReadErrorKind::Invalid("auftragsklammer (0–9999999999)")))?;
            let (text, path) = a.text("anzahlAuftraege")?;
            let anzahl_auftraege = text
                .trim()
                .parse()
                .map_err(|_| ReadError::at(&path, ReadErrorKind::Invalid("anzahlAuftraege (xsd:int)")))?;
            Ok(Auftragskenner { auftragsklammer, anzahl_auftraege })
        })
    })?;
    Ok(Kenner { projekt_id, auftragskenner })
}

fn anrede(s: &mut Seq<'_>) -> R<Anrede> {
    Ok(s.code("anrede", "anrede (1, 2, 9)", Anrede::from_code)?)
}

fn person(s: &mut Seq<'_>) -> R<Person> {
    Ok(Person { anrede: anrede(s)?, vorname: s.opt_val("vorname")?, nachname: s.val("nachname")? })
}

fn ansprechpartner(s: &mut Seq<'_>) -> R<Ansprechpartner> {
    Ok(Ansprechpartner {
        person: person(s)?,
        telefonnummer: s.val("telefonnummer")?,
        mobilfunknummer: s.opt_val("mobilfunknummer")?,
        faxnummer: s.opt_val("faxnummer")?,
        emailadresse: s.opt_val("emailadresse")?,
    })
}

fn ansprechpartner_mit_rolle(s: &mut Seq<'_>) -> R<AnsprechpartnerMitRolle> {
    Ok(AnsprechpartnerMitRolle {
        ansprechpartner: ansprechpartner(s)?,
        rolle: s.code("rolle", "rolle (AnsprechpartnerRolleType)", AnsprechpartnerRolle::from_code)?,
    })
}

fn ansprechpartner_auftrag(s: Seq<'_>) -> R<AnsprechpartnerAuftrag> {
    finish(s, |s| {
        Ok(AnsprechpartnerAuftrag {
            auftragsmanagement: req(s, "auftragsmanagement", |a| finish(a, ansprechpartner))?,
            weitere: each(s, "ansprechpartner", 0, max::UNBOUNDED, |a| finish(a, ansprechpartner_mit_rolle))?,
        })
    })
}

fn wunschtermin(s: &mut Seq<'_>) -> R<Wunschtermin> {
    Ok(Wunschtermin { datum: s.date("datum")?, zeitfenster: zeitfenster(s)? })
}

fn termine(s: &mut Seq<'_>) -> R<Wunschtermin> {
    req(s, "termine", |t| finish(t, |t| req(t, "auftraggeberWunschtermin", |w| finish(w, wunschtermin))))
}

fn read_produkt(s: &mut Seq<'_>) -> R<Produkt> {
    let (node, path) = s.req("produkt")?;
    expect_xsi(node, &path, NS_FTTX, types::PRODUKT)?;
    finish(seq(node, path)?, |p| Ok(Produkt { bezeichner: p.val("bezeichner")? }))
}

fn aenderung(s: &mut Seq<'_>, min: usize) -> R<(Produkt, Vec<AenderungPosition>)> {
    let p = read_produkt(s)?;
    let positionen = each(s, "position", min, max::UNBOUNDED, |x| {
        finish(x, |x| {
            Ok(AenderungPosition {
                produkt: read_produkt(x)?,
                aktionscode: x.code("aktionscode", "aktionscode (A, W, Z)", Aktionscode::from_code)?,
            })
        })
    })?;
    Ok((p, positionen))
}

fn diagnose(s: &mut Seq<'_>) -> R<Diagnose> {
    let vertragsnummer = s.val("vertragsnummer")?;
    let line_id = s.val("lineId")?;
    let operationen = req(s, "auftragsposition", |p| {
        finish(p, |p| {
            req(p, "parameter", |x| {
                finish(x, |x| {
                    x.many("operation", 1, max::UNBOUNDED)?
                        .into_iter()
                        .map(|(node, path)| {
                            leaf(node, &path)?.parse().map_err(|e| ReadError::at(&path, ReadErrorKind::Value(e)))
                        })
                        .collect()
                })
            })
        })
    })?;
    Ok(Diagnose { vertragsnummer, line_id, operationen })
}

fn inhaber(
    s: &mut Seq<'_>,
    with_phones: bool,
) -> R<(Inhaber, Option<crate::Telefonnummer>, Option<crate::Telefonnummer>)> {
    let phones = |x: &mut Seq<'_>| -> R<_> {
        if with_phones { Ok((x.opt_val("telefonnummer")?, x.opt_val("mobilfunknummer")?)) } else { Ok((None, None)) }
    };
    if let Some(p) = s.opt_seq("person")? {
        finish(p, |p| {
            let person = person(p)?;
            let (tel, mobil) = phones(p)?;
            Ok((Inhaber::Person(person), tel, mobil))
        })
    } else {
        req(s, "firma", |f| {
            finish(f, |f| {
                let firma = Firma {
                    anrede: f.code("anrede", "anrede (4, 9)", AnredeFirma::from_code)?,
                    firmenname: f.val("firmenname")?,
                    firmenname_zweiter_teil: f.opt_val("firmennameZweiterTeil")?,
                };
                let (tel, mobil) = phones(f)?;
                Ok((Inhaber::Firma(firma), tel, mobil))
            })
        })
    }
}

fn strasse(s: &mut Seq<'_>) -> R<Strasse> {
    req(s, "strasse", |x| {
        finish(x, |x| {
            Ok(Strasse {
                strassenname: x.val("strassenname")?,
                hausnummer: x.val("hausnummer")?,
                hausnummern_zusatz: x.opt_val("hausnummernZusatz")?,
            })
        })
    })
}

fn ort(s: &mut Seq<'_>) -> R<Ort> {
    req(s, "ort", |x| finish(x, |x| Ok(Ort { ortsname: x.val("ortsname")?, ortsteil: x.opt_val("ortsteil")? })))
}

fn standort_a(s: &mut Seq<'_>) -> R<StandortA> {
    let (inhaber, telefonnummer, mobilfunknummer) = inhaber(s, true)?;
    let strasse = strasse(s)?;
    let gebaeudeteil = opt(s, "gebaeudeteil", |g| {
        finish(g, |g| {
            Ok(GebaeudeteilAngabe { name: g.val("gebaeudeteilName")?, zusatz: g.opt_val("gebaeudeteilZusatz")? })
        })
    })?;
    let land = s.opt_val("land")?;
    let postleitzahl = s.val("postleitzahl")?;
    let ort = ort(s)?;
    let lage_tae_ont = s.opt_val("lageTAE_ONT")?;
    Ok(StandortA {
        inhaber,
        telefonnummer,
        mobilfunknummer,
        strasse,
        gebaeudeteil,
        land,
        postleitzahl,
        ort,
        lage_tae_ont,
    })
}

fn standort_versand(s: &mut Seq<'_>) -> R<StandortVersand> {
    let (inhaber, _, _) = inhaber(s, false)?;
    Ok(StandortVersand {
        inhaber,
        strasse: strasse(s)?,
        land: s.opt_val("land")?,
        postleitzahl: s.val("postleitzahl")?,
        ort: ort(s)?,
    })
}

fn vormieter(s: &mut Seq<'_>) -> R<Vormieter> {
    Ok(Vormieter {
        person: opt(s, "person", |p| {
            finish(p, |p| Ok(VormieterPerson { vorname: p.opt_val("vorname")?, nachname: p.val("nachname")? }))
        })?,
        home_id: opt(s, "homeId", |h| finish(h, |h| Ok(h.val("homeIdNummer")?)))?,
        seriennummer_ont: s.opt_val("seriennummerONT")?,
    })
}

fn montageleistung(s: &mut Seq<'_>) -> R<Montageleistung> {
    Ok(Montageleistung {
        ansprechpartner: req(s, "ansprechpartner", |a| finish(a, ansprechpartner))?,
        montagehinweis: s.opt_val("montagehinweis")?,
    })
}

fn wita_infos(s: &mut Seq<'_>) -> R<WitaInfos> {
    Ok(WitaInfos { wita_vertragsnummer: s.opt_val("witaVertragsnummer")?, technologie: s.opt_val("technologie")? })
}

fn onkz_rufnummer(s: &mut Seq<'_>) -> R<OnkzRufnummer> {
    Ok(OnkzRufnummer { onkz: s.val("onkz")?, rufnummer: s.val("rufnummer")? })
}

fn wita_extended(s: &mut Seq<'_>) -> R<WitaExtendedInfos> {
    let wita = wita_infos(s)?;
    let bestandssuche = opt(s, "bestandssuche", |b| finish(b, onkz_rufnummer))?;
    let rufnummern_portierung = opt(s, "rufnummernPortierung", |r| {
        finish(r, |r| {
            let anschluss = if let Some(a) = r.opt_seq("anlagenanschluss")? {
                finish(a, |a| {
                    let (onkz, durchwahlnummer, abfragestelle) = req(a, "onkzDurchwahlAbfragestelle", |k| {
                        finish(k, |k| Ok((k.val("onkz")?, k.val("durchwahlnummer")?, k.val("abfragestelle")?)))
                    })?;
                    let bloecke = each(a, "zuPortierenderRufnummernblock", 1, max::BLOCKS, |b| {
                        finish(b, |b| Ok(Rufnummernblock { von: b.val("rnrBlockVon")?, bis: b.val("rnrBlockBis")? }))
                    })?;
                    Ok(PortierungAnschluss::Anlage { onkz, durchwahlnummer, abfragestelle, bloecke })
                })?
            } else {
                req(r, "einzelanschluss", |e| {
                    finish(e, |e| {
                        if let Some(l) = e.opt_seq("rufnummernliste")? {
                            finish(l, |l| {
                                Ok(PortierungAnschluss::Rufnummern(each(
                                    l,
                                    "zuPortierendeOnkzRnr",
                                    1,
                                    max::NUMBERS,
                                    |n| finish(n, onkz_rufnummer),
                                )?))
                            })
                        } else {
                            let (text, path) = e.text("alleRufnummern")?;
                            if text != "J" {
                                return Err(ReadError::at(&path, ReadErrorKind::Invalid("alleRufnummern (J)")));
                            }
                            Ok(PortierungAnschluss::AlleRufnummern)
                        }
                    })
                })?
            };
            Ok(RufnummernPortierung { anschluss, portierungskenner: r.val("portierungskenner")? })
        })
    })?;
    let abgebender_ekp = s.opt_boolean("abgebenderEKP")?;
    Ok(WitaExtendedInfos { wita, bestandssuche, rufnummern_portierung, abgebender_ekp })
}

fn rueckmeldung(s: &mut Seq<'_>) -> R<ProviderwechselRueckmeldung> {
    Ok(ProviderwechselRueckmeldung {
        providername: s.opt_val("providername")?,
        zustimmung: s.code("zustimmungProviderwechsel", "zustimmungProviderwechsel (J, N)", JaNein::from_code)?,
        antwortcode: s.opt_val("antwortcode")?,
        antworttext: s.opt_val("antworttext")?,
    })
}

fn fttx_anschluss(s: &mut Seq<'_>) -> R<FttxAnschluss> {
    Ok(FttxAnschluss {
        a10nsp: s.val("a10nsp")?,
        port: s.val("port")?,
        s_vlan: s.opt_val("s-vlan")?,
        c_vlan: s.opt_val("c-vlan")?,
        line_id: s.val("lineId")?,
        uebergabepunkt_kunde: s.code(
            "uebergabepunktKunde",
            "uebergabepunktKunde (TAE, RJ45)",
            UebergabepunktKunde::from_code,
        )?,
        wita_vertragsnummer: s.opt_val("witaVertragsnummer")?,
    })
}

/// `anschluss/lineId` (`FTTxAnschlussEinfachType`).
fn line(s: &mut Seq<'_>) -> R<crate::LineId> {
    req(s, "anschluss", |a| finish(a, |a| Ok(a.val("lineId")?)))
}

// -- positions -----------------------------------------------------------------------------

/// `meldungspositionen` with at least one `position`, each read by `f`.
fn positions<T>(m: &mut Seq<'_>, mut f: impl FnMut(&mut Seq<'_>) -> R<T>) -> R<Vec<T>> {
    req(m, "meldungspositionen", |p| finish(p, |p| each(p, "position", 1, max::UNBOUNDED, |x| finish(x, &mut f))))
}

fn position(p: &mut Seq<'_>) -> R<Position> {
    let (code, path) = p.text("meldungscode")?;
    let code = code.trim();
    if code.is_empty() || code.chars().count() > 10 {
        return Err(ReadError::at(&path, ReadErrorKind::Invalid("meldungscode (1–10 characters)")));
    }
    Ok(Position { code: Meldungscode::parse(code), text: p.val("meldungstext")? })
}

fn wita_position(p: &mut Seq<'_>) -> R<PositionWita> {
    Ok(PositionWita {
        position: position(p)?,
        wita_meldungscode: p.opt_val("witaMeldungscode")?,
        wita_meldungstext: p.opt_val("witaMeldungstext")?,
    })
}

/// `positionsattribute`: either derived type (QEB or ABM) is accepted; both carry the same
/// contact.
fn positionsattribute(p: &mut Seq<'_>) -> R<Option<Ansprechpartner>> {
    let Some((node, path)) = p.opt("positionsattribute") else { return Ok(None) };
    let ty = xsi_in(node, &path, NS_MESSAGE)?;
    if ty != types::POS_QEB && ty != types::POS_ABM {
        return Err(ReadError::at(&path, ReadErrorKind::XsiType(format!("{ty} is not a position attribute type"))));
    }
    finish(seq(node, path)?, |a| req(a, "ansprechpartnerLeistungserbringer", |x| finish(x, ansprechpartner))).map(Some)
}
