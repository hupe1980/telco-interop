//! # spri — S/PRI 4.x orders and messages, in Rust
//!
//! S/PRI is the carrier interface of the German open-access fibre market, maintained by the
//! AK S&P: a service provider (Auftraggeber) orders lines from a network owner
//! (Leistungserbringer) — provisioning, cancellation, changes, fault repair, diagnostics and the
//! provider switch that follows a WBCI pre-agreement — and the network owner answers with
//! messages. This crate gives you those orders and messages as typed values, read and written
//! as XML, without I/O:
//!
//! | You want to… | Use |
//! |---|---|
//! | build an order | [`Auftrag`] with a [`Geschaeftsfall`] (`NEU`, `KUE-AG`, `LAE`, `AEN-LMAE`, `EST`, `PV`, `GET`, `SET`) |
//! | build or read a message | [`Meldung`] with a [`Meldungsinhalt`] (`QEB`, `ABM`, `ABBM`, `ERLM`, … 20 types) |
//! | answer with a technical receipt | [`Teq::new`], [`xml::ReadError::teq_code`] |
//! | write or read the XML | [`Payload::to_xml`], [`Payload::to_soap`], [`Payload::from_xml`] |
//! | look up a code | [`Meldungscode`] |
//! | store an attachment | [`Anlage::safe_file_name`] |
//! | follow an order's lifecycle: what may come next, what is due | [`process::Process`], [`deadline`], [`Abbm::outcome`] |
//! | test your side against a simulated network owner and losing provider | [`sim::Leistungserbringer`], [`sim::AbgebenderProvider`] |
//! | know whether the network owner will refuse it | [`Auftrag::issues_at`], [`Auftrag::issues_against`], [`Meldung::issues`] → [`Issue`] |
//!
//! ## A provider switch after WBCI
//!
//! The Vorabstimmungs-ID is the same type as in the `wbci` crate, so the id of an agreed
//! pre-agreement goes into the `PV` order without conversion.
//!
//! ```
//! use spri::*;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let kontakt = Ansprechpartner {
//!     person: Person { anrede: Anrede::Frau, vorname: None, nachname: Nachname::new("Auftragsmanagement")? },
//!     telefonnummer: Telefonnummer::new("030 1234567")?,
//!     mobilfunknummer: None,
//!     faxnummer: None,
//!     emailadresse: None,
//! };
//! let pv = Providerwechsel {
//!     line_id: "DEU.EKP2.1754639821".parse()?,
//!     vorabstimmungs_id: "DEU.EKP1.V000000013".parse()?, // from the WBCI RUEM-VA
//!     anlage: None,
//!     ansprechpartner: AnsprechpartnerAuftrag { auftragsmanagement: kontakt, weitere: vec![] },
//!     wunschtermin: "2027-01-15".parse::<Date>()?.into(),
//!     produkt: Produkt { bezeichner: Produktbezeichner::new("FTTH 1000")? },
//!     fttx: FttxProviderwechsel {
//!         standort_a: StandortA {
//!             inhaber: Inhaber::Person(Person {
//!                 anrede: Anrede::Frau,
//!                 vorname: Some(Vorname::new("Erika")?),
//!                 nachname: Nachname::new("Mustermann")?,
//!             }),
//!             telefonnummer: None,
//!             mobilfunknummer: None,
//!             strasse: Strasse {
//!                 strassenname: Strassenname::new("Hauptstraße")?,
//!                 hausnummer: Hausnummer::new("12")?,
//!                 hausnummern_zusatz: None,
//!             },
//!             gebaeudeteil: None,
//!             land: None,
//!             postleitzahl: Postleitzahl::new("10115")?,
//!             ort: Ort { ortsname: Ortsname::new("Berlin")?, ortsteil: None },
//!             lage_tae_ont: None,
//!         },
//!         montageleistung: None,
//!     },
//!     positionen: vec![],
//! };
//! let auftrag = Auftrag {
//!     externe_auftragsnummer: ExterneAuftragsnummer::new("A-2026-0001")?,
//!     auftraggeber: Auftraggeber {
//!         auftraggebernummer: Auftraggebernummer::new("4711")?,
//!         leistungsnummer: Leistungsnummer::new("RV-1")?,
//!     },
//!     besteller: None,
//!     geschaeftsfall: Geschaeftsfall::Pv(pv),
//!     geschaeftsfall_art: GeschaeftsfallArt::Endkundenanbieterwechsel,
//!     aenderungskennzeichen: Aenderungskennzeichen::Standard,
//!     termin_neu: None,
//!     kenner: None,
//! };
//!
//! let sent: Timestamp = "2026-11-23T08:00:00Z".parse()?;
//! let signatur = SignaturId { issuer: SignaturText::new("CN=Example CA")?, serial: SignaturText::new("4711")? };
//! let payload = Payload::AuftragRequest { control: Control::new(sent, signatur), auftrag };
//! let xml = payload.to_xml()?;
//! assert!(xml.contains(r#"<geschaeftsfall xsi:type="order:ProviderwechselType">"#));
//! assert!(xml.contains("<vorabstimmungId>DEU.EKP1.V000000013</vorabstimmungId>"));
//!
//! // The network owner reads it back as the same value.
//! assert_eq!(Payload::from_xml(xml.as_bytes())?, payload);
//! # Ok(())
//! # }
//! ```
//!
//! ## What is covered
//!
//! S/PRI 4.x with the schema 4.00.03 (namespaces `http://spri.telekom.de/oss/v4/…`, major
//! release `04`) and the FTTx products it defines, the Providervereinbarung S/PRI V4.4 and the
//! message codes of the Arbeitshandbuch S/PRI 4.x V2.9.5 (215 codes, §6.3/§6.4): all eight
//! order business cases, all twenty message types, the technical receipt and the four
//! operation payloads (`annehmenAuftrag`, `annehmenMeldung`, request and response).
//!
//! The order lifecycle follows the EPKs of Anlage 7 (V.13): [`process::Process`] checks which
//! order, message and receipt may follow which per business case, with Storno,
//! Terminverschiebung, the TAM/MTAM loop and the provider-switch leg with the losing provider,
//! and reports the deadlines ([`deadline`]); its interpretations are listed in [`process`].
//! [`validate`] checks an order or message before it is sent, with 34 cited rules; its
//! interpretations are marked in its rule table.
//!
//! Not covered, by design: SOAP transport, WS-Security signing, TLS, persistence.
//!
//! ## Safety for untrusted input
//!
//! The reader refuses DTDs, resolves only the five predefined entities, bounds input size,
//! nesting, element count, text length, repetitions and attachment size ([`xml::Limits`]), and
//! never panics. Names, phone numbers, e-mail addresses, street, house number, free text and
//! attachment content print redacted in `Debug`; errors never contain values.
//!
//! ## Interpretations
//!
//! Where the specifications leave room, this crate decides and says so: the Vorabstimmungs-ID
//! and the LineID use the stricter WBCI/AK S&P formats (the S/PRI schema restricts only their
//! length); the certificate issuer and serial use the WBCI character set (they are shared with
//! WBCI); the release is written with two digits (`04`); the code list's misprinted rows for
//! `1586` and `2001` take the fact sheets' texts; attachments are bounded to 1 MiB by default
//! (the documents name no size).
//!
//! Guides: <https://hupe1980.github.io/telco-interop/docs/spri/>.
//!
//! Unofficial; not affiliated with or endorsed by the AK S&P, VATM, BUGLAS or Deutsche Telekom.

mod code;
pub mod deadline;
mod model;
pub mod process;
pub mod sim;
pub mod validate;
mod values;
pub mod xml;

#[doc(inline)]
pub use carrier_core::{CarrierCode, LineId, SignaturText, ValueError, ValueErrorKind, VorabstimmungsId};
pub use code::{AbbmOutcome, AbbmPhase, GeschaeftsfallCode, Meldungscode, Meldungstyp};
pub use model::*;
pub use validate::{Issue, Severity};
pub use values::*;

pub use carrier_core::Calendar;
pub use jiff::Timestamp;
pub use jiff::civil::{Date, Time};

/// The README and the site's S/PRI pages, compiled and run as doctests so they cannot drift.
#[cfg(doctest)]
mod site_guides {
    #[doc = include_str!("../README.md")]
    struct Readme;
    #[doc = include_str!("../../../site/content/docs/spri/orders-and-messages.md")]
    struct OrdersAndMessages;
    #[doc = include_str!("../../../site/content/docs/spri/lifecycle.md")]
    struct Lifecycle;
    #[doc = include_str!("../../../site/content/docs/spri/codes.md")]
    struct Codes;
    #[doc = include_str!("../../../site/content/docs/spri/validation.md")]
    struct Validation;
}
