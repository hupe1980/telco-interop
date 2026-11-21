//! # wbci — the WBCI 2.0 provider switch, in Rust
//!
//! When a customer in Germany moves a fixed line to another provider, the two providers
//! agree on the switch over WBCI („WITA Based Carrier Interface", AK S&P) before any line is
//! touched: the gaining provider (**EKPauf**) sends a pre-agreement (*Vorabstimmung*), the
//! losing provider (**EKPabg**) answers with a binding date or a coded refusal, the gaining
//! provider announces whether it takes over the line, and either side may cancel or postpone
//! — all under working-day deadlines and § 59 TKG.
//!
//! This crate gives you that protocol as values, without I/O:
//!
//! | You want to… | Use |
//! |---|---|
//! | build a request or an answer | [`Vorabstimmung`], [`Storno`], [`Terminverschiebung`]; [`RuemVa::zustimmung`], [`Abbm::ablehnung`], [`Erlm`], [`AkmTr::uebernahme`], [`Teq::for_anfrage`] |
//! | write or read the XML | [`Payload::to_xml`], [`Payload::to_soap`], [`Payload::from_xml`] |
//! | know whether the peer will refuse it | [`Anfrage::issues_at`], [`Meldung::issues`] → [`Issue`] |
//! | know what is due when, and why | [`deadline`], [`Calendar`] |
//! | follow a switch through its life | [`Process`] |
//! | answer a pre-agreement as the losing provider | [`pruefung::check`] |
//! | look up a code | [`Meldungscode`] |
//!
//! ## A switch in twenty lines
//!
//! ```
//! use wbci::*;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let ekp = Ekp { auf: "DEU.EKP1".parse()?, abg: "DEU.EKP2".parse()? };
//! let va = Vorabstimmung {
//!     id: VorabstimmungsId::allocate(&ekp.auf, 13)?, // DEU.EKP1.V000000013
//!     kundenwunschtermin: "2027-01-15".parse()?,
//!     endkunde: Kunde::person(Anrede::Frau, Some(Vorname::new("Erika")?), Nachname::new("Mustermann")?),
//!     weitere_anschlussinhaber: vec![],
//!     projekt: None,
//!     fall: VorabstimmungsFall::KueOrn {
//!         standort: Standort {
//!             strassenname: Strassenname::new("Hauptstraße")?,
//!             hausnummer: Hausnummer::new("12")?,
//!             hausnummern_zusatz: None,
//!             postleitzahl: Postleitzahl::new("10115")?,
//!             ort: Ortsname::new("Berlin")?,
//!         },
//!         anschlussidentifikation: None,
//!     },
//!     ekp,
//! };
//!
//! // Will the peer accept it? Lead times count from when it is sent.
//! let sent: Timestamp = "2026-11-23T08:00:00Z".parse()?;
//! let anfrage = Anfrage::from(va.clone());
//! assert!(anfrage.issues_at(sent, &Calendar::wbci()).is_empty());
//!
//! // The XML for the peer.
//! let signatur = SignaturId { issuer: SignaturText::new("CN=Example CA")?, serial: SignaturText::new("4711")? };
//! let xml = Payload::AnfrageRequest { control: Control::new(sent, signatur), anfrage }.to_xml()?;
//! assert!(xml.contains("<VorabstimmungsId>DEU.EKP1.V000000013</VorabstimmungsId>"));
//!
//! // Follow it: once acknowledged, the answer is due within 3 working days.
//! let mut process = Process::new(Role::EkpAuf, va.clone(), Direction::Sent, sent)?;
//! process.apply(Direction::Received, sent, Teq::for_anfrage(&va.into(), Meldungscode::Ok))?;
//! let due = &process.deadlines()[0];
//! assert_eq!(due.to_string().split(" — ").next(), Some("EKPabg: by Do 26.11.2026 23:59"));
//! # Ok(())
//! # }
//! ```
//!
//! ## What is covered
//!
//! WBCI 2.0 (namespaces `http://www.vatm.de/wbci/v1/…`, `majorRelease` `02`), with the rules of
//! Anlage 1 „Spezifikation Vorabstimmung" V5.4.1 and the Arbeitshandbuch WBCI 2.10: all six
//! requests (`VA-KUE-MRN`, `VA-KUE-ORN`, `VA-RRNP`, `STR-AUF`, `STR-AEN` from either side,
//! `TVS-VA`), all six messages (`RUEM-VA`, `ABBM`, `ERLM`, `AKM-TR`, `ABBM-TR`, `TEQ`), the 36
//! published message codes, every deadline of Anlage 1 §2.5–§2.11.
//!
//! Not covered, by design: SOAP transport, WS-Security signing, TLS, persistence, the
//! search of your customer database (the [`pruefung`] check compares the record you found).
//! [`Payload::to_soap`] leaves the header
//! empty for your signer; [`Control::signatur`] names the certificate.
//!
//! ## Safety for untrusted input
//!
//! The reader refuses DTDs, resolves only the five predefined entities, bounds input size,
//! nesting, element count, text length and every repetition, and never panics. Values that
//! may be personal data print redacted in `Debug`; errors and issues never contain them.
//!
//! ## Interpretations
//!
//! Where the specifications leave room, this crate decides and says so: working days are
//! Monday to Friday without the nationwide public holidays ([`Calendar::wbci`]; the literal
//! „Mo – Fr" is [`Calendar::mon_fri`]); „24 Stunden (bezogen auf Arbeitstage)" counts 24
//! hours on working days only; the character set follows the schema's pattern.
//!
//! Guides, background and a glossary of the German terms:
//! <https://hupe1980.github.io/telco-interop/docs/wbci/>.
//!
//! Unofficial; not affiliated with or endorsed by the AK S&P, VATM or BUGLAS.
#![cfg_attr(docsrs, feature(doc_cfg))]

mod answer;
pub mod calendar;
mod code;
pub mod deadline;
mod id;
mod model;
pub mod process;
pub mod pruefung;
mod text;
pub mod validate;
mod values;
pub mod xml;

pub use calendar::Calendar;
pub use code::{Geschaeftsfall, Meldungscode, Meldungstyp};
pub use id::{AenderungsId, CarrierCode, LineId, MAX_SEQUENCE, Portierungskennung, StornoId, VorabstimmungsId};
pub use model::*;
pub use process::{Direction, Process};
pub use text::{ValueError, ValueErrorKind};
pub use validate::{Issue, Severity};
pub use values::*;

#[doc(inline)]
pub use carrier_core::SignaturText;

pub use jiff::Timestamp;
pub use jiff::civil::Date;

/// The README and the site's pages, compiled and run as doctests so they cannot drift.
#[cfg(doctest)]
mod site_guides {
    #[doc = include_str!("../README.md")]
    struct Readme;
    #[doc = include_str!("../../../site/content/_index.md")]
    struct Landing;
    #[doc = include_str!("../../../site/content/docs/start/getting-started.md")]
    struct GettingStarted;
    #[doc = include_str!("../../../site/content/docs/wbci/requests-and-messages.md")]
    struct RequestsAndMessages;
    #[doc = include_str!("../../../site/content/docs/wbci/xml.md")]
    struct Xml;
    #[doc = include_str!("../../../site/content/docs/wbci/validation.md")]
    struct Validation;
    #[doc = include_str!("../../../site/content/docs/wbci/deadlines.md")]
    struct Deadlines;
    #[doc = include_str!("../../../site/content/docs/wbci/process.md")]
    struct Process;
    #[doc = include_str!("../../../site/content/docs/wbci/pruefung.md")]
    struct Pruefung;
}
