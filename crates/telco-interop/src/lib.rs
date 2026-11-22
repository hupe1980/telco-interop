//! # telco-interop — the German carrier interfaces in one dependency
//!
//! A provider switch in Germany crosses two interfaces: the gaining and the losing provider
//! agree it over **WBCI** (§ 59 TKG), then the gaining provider orders the line from the
//! network owner over **S/PRI** with a `PV` order that carries the WBCI Vorabstimmungs-ID.
//! This crate gives you both under one name and one version:
//!
//! | Module | Interface | Feature (default on) |
//! |---|---|---|
//! | [`wbci`] | WBCI 2.0 — provider ↔ provider | `wbci` |
//! | [`spri`] | S/PRI 4.x — service provider ↔ network owner | `spri` |
//!
//! The identifiers both interfaces carry are one type each, so a value from one goes into the
//! other as it is:
//!
//! ```
//! use telco_interop::{VorabstimmungsId, spri, wbci};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Agreed over WBCI …
//! let id: wbci::VorabstimmungsId = "DEU.EKP1.V000000013".parse()?;
//! // … and the very same type in an S/PRI provider-switch order.
//! let in_spri: spri::VorabstimmungsId = id.clone();
//! let shared: VorabstimmungsId = in_spri;
//! assert_eq!(shared.as_str(), "DEU.EKP1.V000000013");
//! # Ok(())
//! # }
//! ```
//!
//! ## One switch across both: [`Wechsel`]
//!
//! [`Wechsel`] (both features) holds the WBCI [`wbci::Process`] of one pre-agreement and every
//! S/PRI order carrying its Vorabstimmungs-ID, and recomputes from them on every call: one
//! [`timeline`](Wechsel::timeline) of both interfaces, their [`deadlines`](Wechsel::deadlines),
//! the [`next_actions`](Wechsel::next_actions) of each [`Partei`], and six
//! [`befunde`](Wechsel::befunde) rules that the interfaces agree (agreed before ordered, the line
//! the AKM-TR takes over, the dates, postponement in WBCI first, a WBCI cancellation reaching
//! S/PRI, completion within a working day of the switch date). See the [`wechsel`] module.
//!
//! ## The command line's output as a library: [`report`]
//!
//! [`report`] (feature `report`, not on by default) turns payload bytes into the lines
//! `telco-interop read`, `check` and `code` print — value-free, control characters replaced —
//! for another front end; the site's in-browser playground uses it.
//!
//! Need only one interface? Depend on [`wbci`](https://docs.rs/wbci) or
//! [`spri`](https://docs.rs/spri) directly, or turn the other feature off:
//! `telco-interop = { version = "0.1", default-features = false, features = ["spri"] }`.
//!
//! Guides: <https://hupe1980.github.io/telco-interop/>.
//!
//! Unofficial; not affiliated with or endorsed by the AK S&P, VATM, BUGLAS or Deutsche Telekom.
#![cfg_attr(docsrs, feature(doc_cfg))]

#[doc(inline)]
pub use carrier_core::{Calendar, CarrierCode, LineId, VorabstimmungsId};

/// WBCI 2.0: the § 59 TKG provider switch between providers.
#[cfg(feature = "wbci")]
#[cfg_attr(docsrs, doc(cfg(feature = "wbci")))]
pub use wbci;

/// S/PRI 4.x: open-access orders between service provider and network owner.
#[cfg(feature = "spri")]
#[cfg_attr(docsrs, doc(cfg(feature = "spri")))]
pub use spri;

#[cfg(all(feature = "wbci", feature = "spri"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "wbci", feature = "spri"))))]
pub mod wechsel;

#[cfg(feature = "report")]
#[cfg_attr(docsrs, doc(cfg(feature = "report")))]
pub mod report;

#[cfg(all(feature = "wbci", feature = "spri"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "wbci", feature = "spri"))))]
#[doc(inline)]
pub use wechsel::{Aktion, Befund, Eintrag, Frist, FristDetail, Mismatch, Partei, Richtung, Schnittstelle, Wechsel};

/// The README and the site's cross-interface page, compiled and run as a doctest so it cannot drift.
#[cfg(all(doctest, feature = "wbci", feature = "spri"))]
mod site_guides {
    // The README's samples include `report`.
    #[cfg(feature = "report")]
    #[doc = include_str!("../README.md")]
    struct Readme;
    #[doc = include_str!("../../../site/content/docs/across-both/one-switch.md")]
    struct Wechsel;
}
