//! Lexical facets of the WBCI schema, from the shared foundation.
//!
//! Source: XSD `wbci-type-simple.xsd` (V2.00, schema version 01.00.02),
//! `ErlaubterZeichenBereichType` and `ErlaubterZeichenBereichMehrzeiligType`.

pub(crate) use carrier_core::text::{Charset, TextSpec, Ws, digits};
#[doc(inline)]
pub use carrier_core::text::{ValueError, ValueErrorKind};
pub(crate) use carrier_core::text_type;
