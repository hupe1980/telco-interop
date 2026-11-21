//! The deadlines of an S/PRI order, explained.
//!
//! Every function takes the event that starts the clock (and, for counted days, a
//! [`Calendar`]) and returns the instant with its rule, source and arithmetic, so a person can
//! check it by hand. Days are German calendar days (`Europe/Berlin`); „n Werktage" ends at 23:59
//! of the n-th Werktag (Monday to Saturday without nationwide holidays, [`Calendar::werktage`])
//! after the starting day.

#[doc(inline)]
pub use carrier_core::count::Explanation;

use core::fmt;

use carrier_core::Calendar;
use carrier_core::count::{Berlin, forward, next_midnight, start_of};
use jiff::civil::Date;
use jiff::{SignedDuration, Timestamp};

use crate::process::Role;

/// A rule a deadline comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rule {
    /// Stable identifier, e.g. `tam.answer`.
    pub id: &'static str,
    /// Where the rule is written, e.g. `AH 2.9.5 §3.4`.
    pub source: &'static str,
    /// The rule in one sentence.
    pub text: &'static str,
}

macro_rules! rules {
    ($($name:ident = $id:literal, $source:literal, $text:literal;)+) => {
        /// The deadline rules of S/PRI.
        pub mod rules {
            use super::Rule;
            $(
                #[doc = $text]
                pub const $name: Rule = Rule { id: $id, source: $source, text: $text };
            )+
        }
    };
}

rules! {
    STORNO_LATEST = "storno.latest", "AH 2.9.5 §3.5",
        "A Storno is sent after the QEB and at least 36 hours before the binding date (VLT); after a TAM also later (interpretation: 36 hours of elapsed time before 0:00 of the VLT; the code fact sheets say 3 Arbeitstage).";
    TV_LATEST = "tv.latest", "AH 2.9.5 §3.4",
        "A Terminverschiebung is sent after the QEB and at least 36 hours before the binding date (VLT; KUE-AG: the termination date); after a TAM also later (interpretation: 36 hours of elapsed time before 0:00 of the VLT; the code fact sheets say 3 Arbeitstage).";
    TAM_ANSWER = "tam.answer", "AH 2.9.5 §3.4, §3.6",
        "The service provider answers a TAM within 10 Werktage (ERLM-K, Terminverschiebung or Storno); otherwise the network owner sends the MTAM.";
    MTAM_ANSWER = "mtam.answer", "AH 2.9.5 §3.4, §3.6",
        "The service provider answers an MTAM within 10 Werktage; otherwise the network owner aborts with ABBM 1198.";
    RUEM_PV = "ruem-pv.antwortfrist", "S/PRI 4.4 §5.5.1.1, §8.3.3",
        "The losing provider answers the AKM-PV with a RUEM-PV by the end of its antwortfrist; otherwise the network owner aborts with ABBM 0042 to the gaining and ABBM-PV 0040 to the losing provider.";
    TEQ = "teq.response", "S/PRI 4.4 §11.2.3, §11.4",
        "The receiver acknowledges an order or message with a TEQ within 30 seconds, 120 seconds with embedded documents.";
}

/// What a deadline is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DeadlineKind {
    /// The technical receipt of an order or message.
    TeqResponse,
    /// Last moment for a Storno (a window, not an obligation).
    StornoLatest,
    /// Last moment for a Terminverschiebung (a window, not an obligation).
    TerminverschiebungLatest,
    /// The service provider's answer to a TAM; when it passes, the MTAM is due.
    MtamDue,
    /// The service provider's answer to an MTAM; when it passes, ABBM 1198 is due.
    Abbm1198Due,
    /// The losing provider's RUEM-PV; when it passes, ABBM 0042 and ABBM-PV 0040 are due.
    RuemPv,
}

/// A point in time something must happen before, with its rule and arithmetic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deadline {
    /// What it is about.
    pub kind: DeadlineKind,
    /// Who must act, or whose window it is.
    pub party: Role,
    /// The act must happen before this instant.
    pub due: Timestamp,
    /// The rule.
    pub rule: Rule,
    /// The arithmetic.
    pub explanation: Explanation,
}

impl Deadline {
    /// Whether the deadline has passed at `now`.
    #[must_use]
    pub fn is_overdue(&self, now: Timestamp) -> bool {
        now >= self.due
    }

    /// Whether this is a window (the last moment for an optional act) rather than an
    /// obligation.
    #[must_use]
    pub const fn is_window(&self) -> bool {
        matches!(self.kind, DeadlineKind::StornoLatest | DeadlineKind::TerminverschiebungLatest)
    }
}

impl fmt::Display for Deadline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: by {} — {} ({})", self.party, Berlin(self.due), self.rule.text, self.rule.source)
    }
}

const HOURS_36: &str = "36 hours of elapsed time before 0:00 of the binding date";

fn before_vlt(kind: DeadlineKind, vlt: Date, rule: Rule) -> Deadline {
    let from = start_of(vlt);
    Deadline {
        kind,
        party: Role::Auftraggeber,
        due: from.checked_sub(SignedDuration::from_hours(36)).unwrap_or(Timestamp::MIN),
        rule,
        explanation: Explanation { start: "binding date", from, counted: vec![], skipped: vec![], method: HOURS_36 },
    }
}

/// The last moment for a Storno of an order confirmed for `liefertermin`: 36 hours before
/// ([`rules::STORNO_LATEST`], an interpretation).
#[must_use]
pub fn storno_latest(liefertermin: Date) -> Deadline {
    before_vlt(DeadlineKind::StornoLatest, liefertermin, rules::STORNO_LATEST)
}

/// The last moment for a Terminverschiebung of an order confirmed for `liefertermin`: 36
/// hours before ([`rules::TV_LATEST`], an interpretation).
#[must_use]
pub fn terminverschiebung_latest(liefertermin: Date) -> Deadline {
    before_vlt(DeadlineKind::TerminverschiebungLatest, liefertermin, rules::TV_LATEST)
}

fn werktage(kind: DeadlineKind, start: &'static str, at: Timestamp, rule: Rule, cal: &Calendar) -> Deadline {
    let (day, explanation) = forward(cal, start, at, 10);
    Deadline { kind, party: Role::Leistungserbringer, due: next_midnight(day), rule, explanation }
}

/// When the MTAM is due: 10 Werktage after the TAM sent at `tam_at` ([`rules::TAM_ANSWER`]).
/// `cal` should count Werktage ([`Calendar::werktage`]).
#[must_use]
pub fn mtam_due(tam_at: Timestamp, cal: &Calendar) -> Deadline {
    werktage(DeadlineKind::MtamDue, "TAM", tam_at, rules::TAM_ANSWER, cal)
}

/// When ABBM 1198 is due: 10 Werktage after the MTAM sent at `mtam_at`
/// ([`rules::MTAM_ANSWER`]). `cal` should count Werktage ([`Calendar::werktage`]).
#[must_use]
pub fn abbm_1198_due(mtam_at: Timestamp, cal: &Calendar) -> Deadline {
    werktage(DeadlineKind::Abbm1198Due, "MTAM", mtam_at, rules::MTAM_ANSWER, cal)
}

/// The losing provider's answer: by 23:59 of the AKM-PV's `antwortfrist` ([`rules::RUEM_PV`]).
#[must_use]
pub fn ruem_pv(antwortfrist: Date) -> Deadline {
    Deadline {
        kind: DeadlineKind::RuemPv,
        party: Role::AbgebenderProvider,
        due: next_midnight(antwortfrist),
        rule: rules::RUEM_PV,
        explanation: Explanation {
            start: "antwortfrist of the AKM-PV",
            from: start_of(antwortfrist),
            counted: vec![],
            skipped: vec![],
            method: "ends 23:59 of the antwortfrist",
        },
    }
}

/// The technical receipt of something sent at `sent_at` to `receiver`: 30 seconds, 120 with
/// embedded documents ([`rules::TEQ`]).
#[must_use]
pub fn teq_response(sent_at: Timestamp, receiver: Role, with_attachment: bool) -> Deadline {
    let (secs, method) = if with_attachment {
        (120, "120 seconds of elapsed time (embedded documents)")
    } else {
        (30, "30 seconds of elapsed time")
    };
    Deadline {
        kind: DeadlineKind::TeqResponse,
        party: receiver,
        due: sent_at.checked_add(SignedDuration::from_secs(secs)).unwrap_or(Timestamp::MAX),
        rule: rules::TEQ,
        explanation: Explanation { start: "sent", from: sent_at, counted: vec![], skipped: vec![], method },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    #[test]
    fn storno_window_is_36_hours_before_the_binding_date() {
        let d = storno_latest("2027-01-15".parse().unwrap());
        // 0:00 Berlin on 15.01. is 23:00Z on 14.01.; minus 36 h.
        assert_eq!(d.due, ts("2027-01-13T11:00:00Z"));
        assert!(d.is_window());
        assert_eq!(d.to_string(), format!("Auftraggeber: by Mi 13.01.2027 12:00 — {} (AH 2.9.5 §3.5)", d.rule.text));
        assert_eq!(terminverschiebung_latest("2027-01-15".parse().unwrap()).due, d.due);
    }

    #[test]
    fn mtam_counts_werktage() {
        // TAM on Friday 20.11.2026: 10 Werktage end Wednesday 02.12.2026 23:59.
        let d = mtam_due(ts("2026-11-20T10:00:00Z"), &Calendar::werktage());
        assert_eq!(Berlin(d.due).to_string(), "Mi 02.12.2026 23:59");
        assert_eq!(d.explanation.counted.len(), 10);
        assert!(d.explanation.to_string().contains("Sa 21.11.2026"));
        assert!(!d.is_window());
        let d = abbm_1198_due(ts("2026-12-21T10:00:00Z"), &Calendar::werktage());
        // Mo 21.12.: 22, 23, 24, (25, 26 holidays, 27 Sunday), 28, 29, 30, 31, (01.01. holiday), Sa 02.01., (03 Sunday), 04, 05.
        assert_eq!(Berlin(d.due).to_string(), "Di 05.01.2027 23:59");
        assert_eq!(d.explanation.skipped.len(), 5);
    }

    #[test]
    fn ruem_pv_ends_with_the_antwortfrist() {
        let d = ruem_pv("2026-12-01".parse().unwrap());
        assert_eq!(Berlin(d.due).to_string(), "Di 01.12.2026 23:59");
        assert_eq!(d.party, Role::AbgebenderProvider);
    }

    #[test]
    fn extreme_instants_saturate() {
        assert_eq!(teq_response(Timestamp::MAX, Role::Auftraggeber, false).due, Timestamp::MAX);
        let _ = storno_latest(Date::MIN);
        let _ = storno_latest(Date::MAX);
        let _ = ruem_pv(Date::MAX);
        let _ = mtam_due(Timestamp::MAX, &Calendar::werktage());
        let _ = abbm_1198_due(Timestamp::MIN, &Calendar::werktage());
    }

    #[test]
    fn teq_response_time() {
        let at = ts("2026-11-23T08:00:00Z");
        assert_eq!(teq_response(at, Role::Leistungserbringer, false).due, ts("2026-11-23T08:00:30Z"));
        assert_eq!(teq_response(at, Role::Leistungserbringer, true).due, ts("2026-11-23T08:02:00Z"));
    }
}
