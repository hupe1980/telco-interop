//! Working days (Arbeitstage, AT).
//!
//! A1 V5.4.1 §2.5 counts deadlines in „Arbeitstage (Mo – Fr)"; WBCI 2.0 §12.6 keeps the
//! interface available „an bundeseinheitlichen Arbeitstagen". [`Calendar::wbci`] reads both
//! together: Monday to Friday without the nine nationwide public holidays. That is an
//! interpretation until the certifier confirms it; [`Calendar::mon_fri`] is the literal
//! reading, and [`Calendar::with_closed`] adds regional or company closing days.

#[doc(inline)]
pub use carrier_core::calendar::{Calendar, Closed, berlin, easter_sunday, holiday};
