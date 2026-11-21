//! The losing provider's check: from a received pre-agreement and your own record of the
//! customer contract, the complete answer — and why.
//!
//! [`check`] applies the „Prüflogik" of Anl. 1 V5.4.1 §2.4 with the word rules of the AK S&P
//! „Infodokument WBCI Phonetische Prüfung" v1.1 and the order of the „Beispiel Prüfbaum" V1.1,
//! and returns a [`Pruefung`]: a RUEM-VA (8001 or 8002 with the binding date, ADA hints, the
//! resources and the numbers to port) or an ABBM listing every reason found (§2.3: „alle …
//! festgestellten Gründe"), plus one [`Befund`] per field compared. Finding the record is
//! your database query; [`not_found`] answers when there is none.
//!
//! ```
//! use wbci::pruefung::{self, Bestand};
//! use wbci::*;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let xml = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/va-kue-orn.xml"))?;
//! # let Payload::AnfrageRequest { anfrage: Anfrage::Vorabstimmung(va), .. } = Payload::from_xml(xml.as_bytes())? else { unreachable!() };
//! // Your record of the contract the request names (found by your own query).
//! let mut bestand = Bestand::new(Kunde::firma(Firmenname::new("Beispiel GmbH")?), "D123".parse()?);
//! bestand.standort = va.standort().cloned();
//! let pruefung = pruefung::check(&va, &bestand, "2026-11-23T08:00:00Z".parse()?, &Calendar::wbci());
//! assert!(pruefung.is_agreed());
//! println!("{pruefung}"); // one line per field compared, no personal data
//! # Ok(())
//! # }
//! ```

mod phonetik;

use core::fmt;

use jiff::Timestamp;
use jiff::civil::Date;

pub use phonetik::{koelner_phonetik, same_name};
use phonetik::{name_codes, street_codes};

use crate::deadline;
use crate::model::{
    Abbm, AdresseAbweichend, Anfrage, Anlagenkopf, Anschluss, Kunde, Meldung, OnkzRufnummer, PortierteRufnummern,
    Position, PositionAbbm, PositionRuemVa, RuemVa, Rufnummernblock, Standort, TechnischeRessource, Vorabstimmung,
    VorabstimmungsFall,
};
use crate::{Calendar, Freitext, Geschaeftsfall, Meldungscode, Meldungstext, Portierungskennung, Technologie};

/// A PBX line: head and number blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anlage {
    /// ONKZ, direct-dial number, operator position.
    pub kopf: Anlagenkopf,
    /// The number blocks.
    pub bloecke: Vec<Rufnummernblock>,
}

/// Your record of the contract a pre-agreement names: what the check compares against.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
#[allow(clippy::struct_excessive_bools)] // independent facts of Anl. 1 §2.3, each with its own code
pub struct Bestand {
    /// The contract holders; the first is the main one.
    pub inhaber: Vec<Kunde>,
    /// Your contract terms require every holder to be named (else 8114 WAI, Anl. 1 §2.3).
    pub alle_inhaber_noetig: bool,
    /// The location of the line.
    pub standort: Option<Standort>,
    /// The single-line numbers of the contract.
    pub rufnummern: Vec<OnkzRufnummer>,
    /// The PBX line, if any.
    pub anlage: Option<Anlage>,
    /// The earliest end of the contract; a later date than requested gives 8002 (NAT).
    pub vertragsende: Option<Date>,
    /// The technical resources, returned in the RUEM-VA.
    pub ressourcen: Vec<TechnischeRessource>,
    /// The technology (Anlage 5), returned in the RUEM-VA.
    pub technologie: Option<Technologie>,
    /// Your porting id (`PKIabg`), returned with every ported number.
    pub pki_abg: Portierungskennung,
    /// A pre-agreement of another provider is open or agreed for this line (8118 VAE).
    pub andere_vorabstimmung: bool,
    /// The customer withdrew the switch order with you (8150 NWE).
    pub neue_willenserklaerung: bool,
    /// Several lines at the location: a VA-KUE-ORN without a number cannot identify one (8117).
    pub mehrere_anschluesse: bool,
    /// When the number was switched off, for a later porting (VA-RRNP, 65 AT).
    pub abgeschaltet: Option<Date>,
}

impl Bestand {
    /// A record with one holder and your porting id; set the other fields as you know them.
    #[must_use]
    pub fn new(inhaber: Kunde, pki_abg: Portierungskennung) -> Self {
        Self {
            inhaber: vec![inhaber],
            alle_inhaber_noetig: false,
            standort: None,
            rufnummern: Vec::new(),
            anlage: None,
            vertragsende: None,
            ressourcen: Vec::new(),
            technologie: None,
            pki_abg,
            andere_vorabstimmung: false,
            neue_willenserklaerung: false,
            mehrere_anschluesse: false,
            abgeschaltet: None,
        }
    }
}

/// The field a [`Befund`] is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Feld {
    /// The surname (or a swapped name field).
    Nachname,
    /// The first name.
    Vorname,
    /// The company name.
    Firmenname,
    /// Further line holders.
    WeitereInhaber,
    /// `postleitzahl`.
    Postleitzahl,
    /// `ort`.
    Ort,
    /// `strassenname`.
    Strasse,
    /// `hausnummer`.
    Hausnummer,
    /// `hausnummernZusatz` (never checked, Anl. 1 §2.3).
    HausnummernZusatz,
    /// The numbers to port.
    Rufnummern,
    /// The PBX head and blocks.
    Anlage,
    /// The optional number of a VA-KUE-ORN.
    Anschlussidentifikation,
    /// The contract end against the requested date.
    Vertragsende,
    /// When a number was switched off (VA-RRNP).
    Abschaltung,
    /// Another provider's pre-agreement.
    AndereVorabstimmung,
    /// The customer's withdrawal.
    Willenserklaerung,
    /// Several lines at the location.
    MehrereAnschluesse,
}

/// How a field was compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Methode {
    /// Word by word, by sound (Infodokument v1.1, Kölner Phonetik).
    Phonetic,
    /// Character by character („bitgenau").
    Exact,
    /// A fact from your record.
    Fact,
    /// Not compared (by rule, or nothing to compare with).
    NotChecked,
}

/// What a comparison found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ergebnis {
    /// The field matches.
    Match,
    /// Some of it matches.
    Partial,
    /// The field does not match (or the fact applies).
    Mismatch,
    /// The request or the record has nothing to compare.
    Absent,
}

/// One field compared: what, how, with what result and which code it gave. Holds no value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Befund {
    /// The field.
    pub feld: Feld,
    /// How it was compared.
    pub methode: Methode,
    /// The result.
    pub ergebnis: Ergebnis,
    /// The code it produced, if any.
    pub code: Option<Meldungscode>,
}

impl fmt::Display for Befund {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let methode = match self.methode {
            Methode::Phonetic => "phonetic",
            Methode::Exact => "exact",
            Methode::Fact => "fact",
            Methode::NotChecked => "not checked",
        };
        let ergebnis = match self.ergebnis {
            Ergebnis::Match => "match",
            Ergebnis::Partial => "partial",
            Ergebnis::Mismatch => "mismatch",
            Ergebnis::Absent => "absent",
        };
        write!(f, "{:?}: {methode} → {ergebnis}", self.feld)?;
        if let Some(code) = &self.code {
            write!(f, " ({})", code.as_str())?;
        }
        Ok(())
    }
}

/// The decision: the answer to send and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pruefung {
    /// A RUEM-VA (agreed) or an ABBM (refused), ready to send.
    pub antwort: Meldung,
    /// Every field compared, in the order of the Prüfbaum.
    pub befunde: Vec<Befund>,
}

impl Pruefung {
    /// Whether the answer agrees to the switch.
    #[must_use]
    pub const fn is_agreed(&self) -> bool {
        matches!(self.antwort, Meldung::RuemVa(_))
    }
}

impl fmt::Display for Pruefung {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let codes: Vec<&str> = self.antwort.codes().map(Meldungscode::as_str).collect();
        let verdict = if self.is_agreed() { "agreed" } else { "refused" };
        write!(f, "{} {verdict} ({})", self.antwort.kopf().geschaeftsfall, codes.join(", "))?;
        for b in &self.befunde {
            write!(f, "\n  {b}")?;
        }
        Ok(())
    }
}

/// The answer when you find no contract for the request: ABBM 8117 (KNI).
#[must_use]
pub fn not_found(va: &Vorabstimmung) -> Pruefung {
    let anfrage = Anfrage::Vorabstimmung(va.clone());
    Pruefung { antwort: Abbm::ablehnung(&anfrage, Meldungscode::Kni).into(), befunde: Vec::new() }
}

/// How the request's names compare with one holder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum NameMatch {
    None,
    SurnameOnly,
    Full,
}

fn pool(k: &Kunde) -> Vec<String> {
    match k {
        Kunde::Person(p) => {
            let mut v = p.vorname.as_ref().map(|n| name_codes(n.as_str())).unwrap_or_default();
            v.extend(name_codes(p.nachname.as_str()));
            v
        }
        Kunde::Firma(f) => {
            let mut v = name_codes(f.name.as_str());
            v.extend(f.name_zweiter_teil.as_ref().map(|n| name_codes(n.as_str())).unwrap_or_default());
            v
        }
    }
}

/// One holder against the words of the request (Infodokument §5.4–§5.6; Anl. 1 §2.4: one
/// part of a double surname and one of several first names suffice).
fn compare(holder: &Kunde, given: &[String]) -> NameMatch {
    let found = |codes: &[String]| codes.iter().any(|c| given.contains(c));
    match holder {
        Kunde::Person(p) => {
            let sur = name_codes(p.nachname.as_str());
            if sur.is_empty() || !found(&sur) {
                return NameMatch::None;
            }
            let first = p.vorname.as_ref().map(|n| name_codes(n.as_str())).unwrap_or_default();
            if first.is_empty() || found(&first) { NameMatch::Full } else { NameMatch::SurnameOnly }
        }
        Kunde::Firma(_) => {
            let all = pool(holder);
            if !all.is_empty() && all.iter().all(|c| given.contains(c)) { NameMatch::Full } else { NameMatch::None }
        }
    }
}

struct Address {
    ok: Option<bool>,
    mismatches: Vec<(AdresseAbweichend, Meldungscode, Meldungscode)>,
}

fn address(given: Option<&Standort>, record: Option<&Standort>, befunde: &mut Vec<Befund>) -> Address {
    let (Some(g), Some(r)) = (given, record) else {
        if given.is_some() {
            for feld in [Feld::Postleitzahl, Feld::Ort, Feld::Strasse, Feld::Hausnummer] {
                befunde.push(Befund { feld, methode: Methode::NotChecked, ergebnis: Ergebnis::Absent, code: None });
            }
        }
        return Address { ok: None, mismatches: Vec::new() };
    };
    let words_found =
        |stored: &[String], given: &[String]| !stored.is_empty() && stored.iter().all(|c| given.contains(c));
    let checks = [
        (
            Feld::Postleitzahl,
            Methode::Exact,
            g.postleitzahl == r.postleitzahl,
            AdresseAbweichend::Postleitzahl(r.postleitzahl.clone()),
            Meldungscode::AdaPlz,
            Meldungscode::AdfPlz,
        ),
        (
            Feld::Ort,
            Methode::Phonetic,
            words_found(&name_codes(r.ort.as_str()), &name_codes(g.ort.as_str())),
            AdresseAbweichend::Ort(r.ort.clone()),
            Meldungscode::AdaOrt,
            Meldungscode::AdfOrt,
        ),
        (
            Feld::Strasse,
            Methode::Phonetic,
            words_found(&street_codes(r.strassenname.as_str()), &street_codes(g.strassenname.as_str())),
            AdresseAbweichend::Strassenname(r.strassenname.clone()),
            Meldungscode::AdaStr,
            Meldungscode::AdfStr,
        ),
        (
            Feld::Hausnummer,
            Methode::Exact,
            g.hausnummer == r.hausnummer,
            AdresseAbweichend::Hausnummer(r.hausnummer.clone()),
            Meldungscode::AdaHsnr,
            Meldungscode::AdfHsnr,
        ),
    ];
    let mut mismatches = Vec::new();
    for (feld, methode, ok, value, ada, adf) in checks {
        let ergebnis = if ok { Ergebnis::Match } else { Ergebnis::Mismatch };
        befunde.push(Befund { feld, methode, ergebnis, code: None });
        if !ok {
            mismatches.push((value, ada, adf));
        }
    }
    befunde.push(Befund {
        feld: Feld::HausnummernZusatz,
        methode: Methode::NotChecked,
        ergebnis: Ergebnis::Absent,
        code: None,
    });
    Address { ok: Some(mismatches.is_empty()), mismatches }
}

struct Numbers {
    ok: bool,
    any_right: bool,
    /// The record's numbers to name in an RNG, when the rules allow it.
    list_on_refusal: bool,
    block_hint: Option<String>,
    ported: Option<PortierteRufnummern>,
}

fn numbers(r: &crate::Rufnummernportierung, b: &Bestand) -> Numbers {
    match &r.anschluss {
        Anschluss::Einzel { rufnummern, alle_rufnummern } => {
            let right = rufnummern.iter().filter(|n| b.rufnummern.contains(n)).count();
            let ok = if *alle_rufnummern { right > 0 } else { right == rufnummern.len() && right > 0 };
            let source: Vec<&OnkzRufnummer> =
                if *alle_rufnummern { b.rufnummern.iter().collect() } else { rufnummern.iter().collect() };
            let ported = PortierteRufnummern::Rufnummern(
                source.into_iter().take(10).map(|n| (n.clone(), b.pki_abg.clone())).collect(),
            );
            Numbers {
                ok,
                any_right: right > 0,
                list_on_refusal: !alle_rufnummern && right > 0,
                block_hint: None,
                ported: Some(ported),
            }
        }
        Anschluss::Anlage { kopf, bloecke } => {
            let Some(a) = &b.anlage else {
                return Numbers { ok: false, any_right: false, list_on_refusal: false, block_hint: None, ported: None };
            };
            let head = a.kopf == *kopf;
            let blocks = bloecke.len() == a.bloecke.len() && bloecke.iter().all(|x| a.bloecke.contains(x));
            // „Stelligkeit des angefragten Rufnummernblocks nicht korrekt" (Anl. 1 §2.3): the
            // head is right but a requested block has another number of digits than the PBX's.
            let hint = (head && !blocks)
                .then(|| {
                    a.bloecke.iter().find(|rb| bloecke.iter().any(|qb| qb.von.as_str().len() != rb.von.as_str().len()))
                })
                .flatten()
                .map(|rb| format!("korrekter Rufnummernblock: {} - {}", rb.von, rb.bis));
            let ported = PortierteRufnummern::Bloecke {
                kopf: a.kopf.clone(),
                bloecke: a.bloecke.iter().take(5).map(|x| (x.clone(), b.pki_abg.clone())).collect(),
            };
            Numbers {
                ok: head && blocks,
                any_right: head,
                list_on_refusal: false,
                block_hint: hint,
                ported: Some(ported),
            }
        }
    }
}

/// Decides the answer to `va` from your record `bestand`, received at `received_at`.
///
/// The answer validates clean under [`Meldung::issues`]; record values (your numbers, the
/// corrected address) appear in it only when the customer is identified by name and a second
/// feature. See the module documentation for the rules and their sources.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn check(va: &Vorabstimmung, bestand: &Bestand, received_at: Timestamp, cal: &Calendar) -> Pruefung {
    let fall = va.geschaeftsfall();
    let mut befunde = Vec::new();
    let mut codes: Vec<(Meldungscode, Option<String>)> = Vec::new();

    // Numbers first (Prüfbaum: KUE-MRN, RRNP).
    let nums = va.rufnummernportierung().map(|r| numbers(r, bestand));
    if let Some(n) = &nums {
        let feld = if matches!(va.rufnummernportierung().map(|r| &r.anschluss), Some(Anschluss::Anlage { .. })) {
            Feld::Anlage
        } else {
            Feld::Rufnummern
        };
        let ergebnis = if n.ok {
            Ergebnis::Match
        } else if n.any_right {
            Ergebnis::Partial
        } else {
            Ergebnis::Mismatch
        };
        befunde.push(Befund { feld, methode: Methode::Exact, ergebnis, code: (!n.ok).then_some(Meldungscode::Rng) });
    }
    let ident_number = match &va.fall {
        VorabstimmungsFall::KueOrn { anschlussidentifikation: Some(n), .. } => {
            let ok = bestand.rufnummern.contains(n);
            befunde.push(Befund {
                feld: Feld::Anschlussidentifikation,
                methode: Methode::Exact,
                ergebnis: if ok { Ergebnis::Match } else { Ergebnis::Mismatch },
                code: None,
            });
            Some(ok)
        }
        _ => None,
    };

    // Names.
    let mut given = pool(&va.endkunde);
    for k in &va.weitere_anschlussinhaber {
        given.extend(pool(k));
    }
    let main_given = pool(&va.endkunde);
    let best = bestand.inhaber.iter().map(|h| compare(h, &main_given)).max().unwrap_or(NameMatch::None);
    let is_firma = matches!(bestand.inhaber.first(), Some(Kunde::Firma(_)));
    let name_ok = best == NameMatch::Full;
    if is_firma {
        befunde.push(Befund {
            feld: Feld::Firmenname,
            methode: Methode::Phonetic,
            ergebnis: if name_ok { Ergebnis::Match } else { Ergebnis::Mismatch },
            code: None,
        });
    } else {
        let sur = if best >= NameMatch::SurnameOnly { Ergebnis::Match } else { Ergebnis::Mismatch };
        befunde.push(Befund { feld: Feld::Nachname, methode: Methode::Phonetic, ergebnis: sur, code: None });
        let first = match best {
            NameMatch::Full => Ergebnis::Match,
            NameMatch::SurnameOnly => Ergebnis::Mismatch,
            NameMatch::None => Ergebnis::Absent,
        };
        befunde.push(Befund { feld: Feld::Vorname, methode: Methode::Phonetic, ergebnis: first, code: None });
    }
    let wai = bestand.alle_inhaber_noetig
        && bestand.inhaber.len() > 1
        && !bestand.inhaber.iter().all(|h| compare(h, &given) == NameMatch::Full);
    if bestand.alle_inhaber_noetig && bestand.inhaber.len() > 1 {
        befunde.push(Befund {
            feld: Feld::WeitereInhaber,
            methode: Methode::Phonetic,
            ergebnis: if wai { Ergebnis::Mismatch } else { Ergebnis::Match },
            code: wai.then_some(Meldungscode::Wai),
        });
    }

    // Address.
    let addr = address(va.standort(), bestand.standort.as_ref(), &mut befunde);

    // Identification (Anl. 1 §2.4).
    let second = match fall {
        Geschaeftsfall::VaKueOrn => ident_number == Some(true) || addr.ok == Some(true),
        _ => nums.as_ref().is_some_and(|n| n.ok),
    };
    let number_feature = match fall {
        Geschaeftsfall::VaKueOrn => ident_number == Some(true),
        _ => nums.as_ref().is_some_and(|n| n.ok),
    };
    let identified = name_ok && second;
    let any_feature = best > NameMatch::None
        || nums.as_ref().is_some_and(|n| n.any_right)
        || ident_number == Some(true)
        || addr.ok == Some(true);

    if any_feature {
        if let Some(n) = &nums {
            if !n.ok {
                codes.push((Meldungscode::Rng, None));
            }
            if let (Some(hint), true) = (&n.block_hint, name_ok) {
                codes.push((Meldungscode::Sonstiges, Some(hint.clone())));
            }
        }
        if fall == Geschaeftsfall::VaRrnp
            && let Some(off) = bestand.abgeschaltet
        {
            let late = deadline::later_porting(off, cal).is_overdue(received_at);
            befunde.push(Befund {
                feld: Feld::Abschaltung,
                methode: Methode::Fact,
                ergebnis: if late { Ergebnis::Mismatch } else { Ergebnis::Match },
                code: late.then_some(Meldungscode::Rng),
            });
            if late && !codes.iter().any(|(c, _)| *c == Meldungscode::Rng) {
                codes.push((Meldungscode::Rng, Some("Nachträgliche Portierung nicht möglich".into())));
            }
        }
        // A VA-KUE-ORN whose identifying number is wrong and whose line the address does not
        // identify either is refused with RNG (Anl. 1 §2.3); numbers come first (Prüfbaum).
        if fall == Geschaeftsfall::VaKueOrn && ident_number == Some(false) && addr.ok != Some(true) {
            codes.push((Meldungscode::Rng, None));
        }
        match best {
            NameMatch::None => codes.push((Meldungscode::Aif, None)),
            NameMatch::SurnameOnly => codes.push((Meldungscode::AifVn, None)),
            NameMatch::Full => {}
        }
        if wai {
            codes.push((Meldungscode::Wai, None));
        }
        // A wrong address refuses only when no number identifies the line (ADF); otherwise
        // it is a hint in the agreement (ADA).
        if !number_feature {
            let unidentified_orn = fall == Geschaeftsfall::VaKueOrn && addr.ok == Some(false);
            if fall != Geschaeftsfall::VaKueOrn || unidentified_orn {
                for (_, _, adf) in &addr.mismatches {
                    codes.push((adf.clone(), None));
                }
            }
        }
        if fall == Geschaeftsfall::VaKueOrn && ident_number.is_none() && bestand.mehrere_anschluesse {
            befunde.push(Befund {
                feld: Feld::MehrereAnschluesse,
                methode: Methode::Fact,
                ergebnis: Ergebnis::Mismatch,
                code: Some(Meldungscode::Kni),
            });
            codes.push((Meldungscode::Kni, None));
        }
    } else {
        codes.push((Meldungscode::Kni, None));
    }
    if bestand.andere_vorabstimmung {
        befunde.push(Befund {
            feld: Feld::AndereVorabstimmung,
            methode: Methode::Fact,
            ergebnis: Ergebnis::Mismatch,
            code: Some(Meldungscode::Vae),
        });
        codes.push((Meldungscode::Vae, None));
    }
    if bestand.neue_willenserklaerung && fall.is_kuendigung() {
        befunde.push(Befund {
            feld: Feld::Willenserklaerung,
            methode: Methode::Fact,
            ergebnis: Ergebnis::Mismatch,
            code: Some(Meldungscode::Nwe),
        });
        codes.push((Meldungscode::Nwe, None));
    }
    // Never agree without identification: nothing else refused, so nothing identifies.
    if codes.is_empty() && !identified {
        codes.push((Meldungscode::Kni, None));
    }
    for b in &mut befunde {
        if b.code.is_none() && b.ergebnis == Ergebnis::Mismatch {
            b.code = match b.feld {
                Feld::Nachname | Feld::Firmenname => {
                    codes.iter().find(|(c, _)| *c == Meldungscode::Aif).map(|(c, _)| c.clone())
                }
                Feld::Vorname => codes.iter().find(|(c, _)| *c == Meldungscode::AifVn).map(|(c, _)| c.clone()),
                Feld::Postleitzahl | Feld::Ort | Feld::Strasse | Feld::Hausnummer => {
                    let i = [Feld::Postleitzahl, Feld::Ort, Feld::Strasse, Feld::Hausnummer]
                        .iter()
                        .position(|f| *f == b.feld);
                    i.and_then(|i| {
                        let (ada, adf) = [
                            (Meldungscode::AdaPlz, Meldungscode::AdfPlz),
                            (Meldungscode::AdaOrt, Meldungscode::AdfOrt),
                            (Meldungscode::AdaStr, Meldungscode::AdfStr),
                            (Meldungscode::AdaHsnr, Meldungscode::AdfHsnr),
                        ]
                        .get(i)?
                        .clone();
                        if codes.iter().any(|(c, _)| *c == adf) {
                            Some(adf)
                        } else if codes.is_empty() {
                            Some(ada)
                        } else {
                            None
                        }
                    })
                }
                _ => None,
            };
        }
    }

    let anfrage = Anfrage::Vorabstimmung(va.clone());
    if codes.is_empty() {
        return Pruefung { antwort: agree(va, bestand, nums, &addr, received_at, cal, &mut befunde).into(), befunde };
    }
    Pruefung { antwort: refuse(&anfrage, bestand, &codes, nums.as_ref(), name_ok).into(), befunde }
}

fn agree(
    va: &Vorabstimmung,
    bestand: &Bestand,
    nums: Option<Numbers>,
    addr: &Address,
    received_at: Timestamp,
    cal: &Calendar,
    befunde: &mut Vec<Befund>,
) -> RuemVa {
    let kuendigung = va.geschaeftsfall().is_kuendigung();
    let mut date = va.kundenwunschtermin;
    if kuendigung {
        date = date.max(deadline::earliest_wechseltermin(received_at, va.kundenwunschtermin, cal).date);
        if let Some(end) = bestand.vertragsende {
            let later = end > date;
            befunde.push(Befund {
                feld: Feld::Vertragsende,
                methode: Methode::Fact,
                ergebnis: if later { Ergebnis::Mismatch } else { Ergebnis::Match },
                code: later.then_some(Meldungscode::Nat),
            });
            date = date.max(end);
        }
    }
    while !cal.is_working_day(date) {
        let Some(next) = date.tomorrow().ok() else { break };
        date = next;
    }
    let mut ruem = RuemVa::zustimmung(va, date);
    for (value, ada, _) in &addr.mismatches {
        ruem.positionen
            .push(PositionRuemVa { position: Position::new(ada.clone()), adresse_abweichend: Some(value.clone()) });
    }
    if kuendigung {
        ruem.ressourcen.clone_from(&bestand.ressourcen);
        ruem.ressourcen.truncate(99);
        ruem.technologie.clone_from(&bestand.technologie);
    }
    ruem.rufnummernportierung = nums.and_then(|n| n.ported);
    ruem
}

fn refuse(
    anfrage: &Anfrage,
    bestand: &Bestand,
    codes: &[(Meldungscode, Option<String>)],
    nums: Option<&Numbers>,
    name_ok: bool,
) -> Abbm {
    let first = codes.first().map_or(Meldungscode::Kni, |(c, _)| c.clone());
    let mut abbm = Abbm::ablehnung(anfrage, first);
    abbm.positionen.clear();
    let list = name_ok && nums.is_some_and(|n| n.list_on_refusal);
    for (code, text) in codes {
        let position = match text.as_deref().and_then(|t| Meldungstext::new(t).ok()) {
            Some(t) => Position::with_text(code.clone(), t),
            None => Position::new(code.clone()),
        };
        let rufnummern = if *code == Meldungscode::Rng && list {
            bestand.rufnummern.iter().take(10).map(|n| n.rufnummer.clone()).collect()
        } else {
            Vec::new()
        };
        abbm.positionen.push(PositionAbbm { position, rufnummern });
        if *code == Meldungscode::Sonstiges {
            abbm.begruendung = text.as_deref().and_then(|t| Freitext::new(t).ok());
        }
    }
    abbm
}
