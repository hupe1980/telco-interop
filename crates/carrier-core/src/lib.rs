//! # carrier-core — what the German carrier interfaces share
//!
//! WBCI (provider ↔ provider) and S/PRI (network owner ↔ service provider) are separate
//! interfaces of the AK S&P, but they carry the same identifiers, count the same working days,
//! wrap their payloads in the same kind of envelope and must read the same hostile XML safely.
//! This crate holds that common ground, so a fix in one interface is a fix in both:
//!
//! | Module | What |
//! |---|---|
//! | [`text`] | schema facets, [`ValueError`], the [`text_type!`] and [`code_enum!`] macros |
//! | [`id`] | [`CarrierCode`], [`VorabstimmungsId`], [`LineId`] |
//! | [`calendar`] | working days: weekends, nationwide public holidays, closed days |
//! | [`count`] | working-day and working-hour arithmetic with an [`count::Explanation`] |
//! | [`envelope`] | [`Zeitstempel`], [`SignaturId`], [`SignaturText`] |
//! | [`xml`] | the bounded tree parser, the schema-order cursor and the writer |
//!
//! Its API serves the sibling crates `wbci` and `spri`, which re-export what their users need;
//! it is not stable on its own.
//!
//! ```
//! use carrier_core::{CarrierCode, VorabstimmungsId};
//!
//! let carrier: CarrierCode = "DEU.EKP1".parse()?;
//! let id = VorabstimmungsId::allocate(&carrier, 13)?;
//! assert_eq!(id.as_str(), "DEU.EKP1.V000000013");
//! # Ok::<(), carrier_core::ValueError>(())
//! ```
//!
//! Unofficial; not affiliated with or endorsed by the AK S&P, VATM or BUGLAS.

pub mod calendar;
pub mod count;
pub mod envelope;
pub mod id;
pub mod text;
pub mod xml;

pub use calendar::Calendar;
pub use envelope::{SignaturId, SignaturText, Zeitstempel};
pub use id::{CarrierCode, LineId, MAX_SEQUENCE, VorabstimmungsId};
pub use text::{ValueError, ValueErrorKind};
