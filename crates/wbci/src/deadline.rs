//! Deadlines that explain themselves.
//!
//! Every function takes the event that starts the clock and a [`Calendar`], and returns the
//! instant or date with the rule, its source and the days counted and skipped, so a person
//! can check the arithmetic by hand. All arithmetic happens in `Europe/Berlin`.
//!
//! Counting (A1 V5.4.1 §2.5): forwards, „X+n AT" ends at 23:59 of the n-th working day after
//! X; backwards, „X−n AT" means before 0:00 of the n-th working day before X. „24 Stunden
//! (bezogen auf Arbeitstage)" is read as 24 hours of elapsed time counted on working days
//! only (an interpretation).

#[doc(inline)]
pub use carrier_core::count::Explanation;

use core::fmt;

use carrier_core::count::{Berlin, Day, backward, forward, next_midnight, start_of, working_hours};
use jiff::civil::Date;
use jiff::{SignedDuration, Timestamp};

use crate::calendar::Calendar;
use crate::{Geschaeftsfall, Role};

/// A rule a deadline comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rule {
    /// Stable identifier, e.g. `va.answer`.
    pub id: &'static str,
    /// Where the rule is written, e.g. `Anl. 1 V5.4.1 §2.5`.
    pub source: &'static str,
    /// The rule in one sentence.
    pub text: &'static str,
}

macro_rules! rules {
    ($($name:ident = $id:literal, $source:literal, $text:literal;)+) => {
        /// The deadline rules of WBCI.
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
    VA_LEAD = "va.lead", "Anl. 1 V5.4.1 §2.5",
        "A VA-KUE-MRN or VA-KUE-ORN asks for a date at least 20 AT ahead, a VA-RRNP at least 7 AT.";
    VA_ANSWER = "va.answer", "Anl. 1 V5.4.1 §2.5, §2.6.1",
        "The EKPabg answers a pre-agreement request within 3 AT after the positive TEQ.";
    WECHSELTERMIN = "ruem-va.date", "Anl. 1 V5.4.1 §2.5",
        "An agreed VA-KUE switch date is at least 15 AT ahead and not before the requested date.";
    TEQ = "teq.escalation", "Anl. 1 V5.4.1 §2.6.1, §2.11.1",
        "Without a technical receipt 2 hours after sending, the sender escalates.";
    STR_AUF_LEAD = "str-auf.lead", "Anl. 1 V5.4.1 §2.9.1, §2.9.2",
        "A cancellation (Aufhebung) arrives at least 3 AT before the agreed switch date.";
    STR_AEN_LEAD = "str-aen.lead", "Anl. 1 V5.4.1 §2.10",
        "A cancellation for change (Änderung) arrives at least 8 AT before the agreed switch date.";
    TVS_LEAD = "tvs.lead", "Anl. 1 V5.4.1 §2.11",
        "A postponement arrives at least 3 AT before the agreed switch date (24 hours on working days when EKPauf and EKPabg share the network operator).";
    ANSWER_24H = "answer.24h", "Anl. 1 V5.4.1 §2.9.1, §2.10, §2.11",
        "A cancellation or postponement is answered within 24 hours, counted on working days, after the positive TEQ.";
    AKM_TR_REMINDER = "akm-tr.reminder", "Anl. 1 V5.4.1 §2.6.2",
        "Without an AKM-TR, the EKPabg reminds 10 AT before the switch date.";
    AKM_TR_ESCALATION = "akm-tr.escalation", "Anl. 1 V5.4.1 §2.6.2",
        "Without an AKM-TR 4 AT before the switch date, the EKPabg escalates.";
    NEW_VA = "str-aen.new-va", "Anl. 1 V5.4.1 §2.10",
        "After an agreed change, the EKPauf submits the new pre-agreement by 7 AT before the switch date, or the first result counts as cancelled.";
    LATER_PORTING = "rrnp.later", "Anl. 1 V5.4.1 §2.3",
        "A later porting (VA-RRNP) is agreed if the number was switched off at most 65 AT before.";
}

/// What a deadline is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DeadlineKind {
    /// The request's technical receipt is overdue: escalate.
    TeqEscalation,
    /// Answer to a pre-agreement request (RUEM-VA or ABBM).
    VaAnswer,
    /// Answer to a cancellation or postponement (ERLM or ABBM).
    Answer24h,
    /// Last moment for a cancellation (Aufhebung).
    StornoAufhebungLatest,
    /// Last moment for a cancellation for change.
    StornoAenderungLatest,
    /// Last moment for a postponement.
    TerminverschiebungLatest,
    /// The AKM-TR should be sent: from here the EKPabg reminds.
    AkmTrReminder,
    /// The AKM-TR is overdue: from here the EKPabg escalates.
    AkmTrEscalation,
    /// The new pre-agreement after a change is due.
    NewVorabstimmung,
    /// Last day a later porting is still agreed.
    LaterPorting,
}

/// A point in time something must happen before, with its rule and arithmetic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deadline {
    /// What it is about.
    pub kind: DeadlineKind,
    /// Who must act.
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
}

impl fmt::Display for Deadline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: by {} — {} ({})", self.party, Berlin(self.due), self.rule.text, self.rule.source)
    }
}

/// The earliest permitted date, with its rule and arithmetic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarliestDate {
    /// The earliest permitted date.
    pub date: Date,
    /// The rule.
    pub rule: Rule,
    /// The arithmetic.
    pub explanation: Explanation,
}

impl fmt::Display for EarliestDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not before {} — {} ({})", Day(self.date), self.rule.text, self.rule.source)
    }
}

/// The earliest `Kundenwunschtermin` for a pre-agreement sent at `sent_at`: 20 AT ahead for
/// `VA-KUE-*`, 7 AT for `VA-RRNP` ([`rules::VA_LEAD`]). `None` for a case without lead time.
#[must_use]
pub fn earliest_kundenwunschtermin(fall: Geschaeftsfall, sent_at: Timestamp, cal: &Calendar) -> Option<EarliestDate> {
    let n = match fall {
        Geschaeftsfall::VaKueMrn | Geschaeftsfall::VaKueOrn => 20,
        Geschaeftsfall::VaRrnp => 7,
        _ => return None,
    };
    let (date, explanation) = forward(cal, "pre-agreement sent", sent_at, n);
    Some(EarliestDate { date, rule: rules::VA_LEAD, explanation })
}

/// The earliest switch date the EKPabg may confirm for a `VA-KUE-*` answered at `answered_at`:
/// 15 AT ahead and not before `requested` ([`rules::WECHSELTERMIN`]).
#[must_use]
pub fn earliest_wechseltermin(answered_at: Timestamp, requested: Date, cal: &Calendar) -> EarliestDate {
    let (date, explanation) = forward(cal, "RUEM-VA sent", answered_at, 15);
    EarliestDate { date: date.max(requested), rule: rules::WECHSELTERMIN, explanation }
}

/// When the sender of a request or message escalates a missing receipt: 2 hours after
/// sending ([`rules::TEQ`]).
#[must_use]
pub fn teq_escalation(sent_at: Timestamp, sender: Role) -> Deadline {
    Deadline {
        kind: DeadlineKind::TeqEscalation,
        party: sender.other(),
        due: carrier_core::count::shift(sent_at, SignedDuration::from_hours(2)),
        rule: rules::TEQ,
        explanation: Explanation {
            start: "sent",
            from: sent_at,
            counted: vec![],
            skipped: vec![],
            method: "2 hours of elapsed time",
        },
    }
}

/// The EKPabg's answer to a pre-agreement: 3 AT after the positive TEQ ([`rules::VA_ANSWER`]).
#[must_use]
pub fn va_answer(teq_at: Timestamp, cal: &Calendar) -> Deadline {
    let (day, explanation) = forward(cal, "positive TEQ", teq_at, 3);
    Deadline {
        kind: DeadlineKind::VaAnswer,
        party: Role::EkpAbg,
        due: next_midnight(day),
        rule: rules::VA_ANSWER,
        explanation,
    }
}

/// The answer to a cancellation or postponement: 24 hours on working days after the positive
/// TEQ ([`rules::ANSWER_24H`]). `answering` is the side that must answer.
#[must_use]
pub fn answer_24h(teq_at: Timestamp, answering: Role, cal: &Calendar) -> Deadline {
    let (due, explanation) = working_hours(cal, "positive TEQ", teq_at, 24, true);
    Deadline { kind: DeadlineKind::Answer24h, party: answering, due, rule: rules::ANSWER_24H, explanation }
}

/// The last moment for a cancellation (Aufhebung) by `sender`: before 0:00 of 3 AT before the
/// switch date ([`rules::STR_AUF_LEAD`]).
#[must_use]
pub fn storno_aufhebung_latest(wechseltermin: Date, sender: Role, cal: &Calendar) -> Deadline {
    lead(DeadlineKind::StornoAufhebungLatest, sender, wechseltermin, 3, rules::STR_AUF_LEAD, cal)
}

/// The last moment for a cancellation for change by `sender`: 8 AT before the switch date
/// ([`rules::STR_AEN_LEAD`]).
#[must_use]
pub fn storno_aenderung_latest(wechseltermin: Date, sender: Role, cal: &Calendar) -> Deadline {
    lead(DeadlineKind::StornoAenderungLatest, sender, wechseltermin, 8, rules::STR_AEN_LEAD, cal)
}

/// The last moment for a postponement: 3 AT before the switch date, or 24 hours on working
/// days when `same_network_operator` (A1 §2.11 special case) ([`rules::TVS_LEAD`]).
#[must_use]
pub fn terminverschiebung_latest(wechseltermin: Date, same_network_operator: bool, cal: &Calendar) -> Deadline {
    if same_network_operator {
        let (due, explanation) = working_hours(cal, "agreed switch date", start_of(wechseltermin), 24, false);
        return Deadline {
            kind: DeadlineKind::TerminverschiebungLatest,
            party: Role::EkpAuf,
            due,
            rule: rules::TVS_LEAD,
            explanation,
        };
    }
    lead(DeadlineKind::TerminverschiebungLatest, Role::EkpAuf, wechseltermin, 3, rules::TVS_LEAD, cal)
}

/// From when the EKPabg reminds a missing AKM-TR: 10 AT before the switch date
/// ([`rules::AKM_TR_REMINDER`]).
#[must_use]
pub fn akm_tr_reminder(wechseltermin: Date, cal: &Calendar) -> Deadline {
    lead(DeadlineKind::AkmTrReminder, Role::EkpAuf, wechseltermin, 10, rules::AKM_TR_REMINDER, cal)
}

/// From when the EKPabg escalates a missing AKM-TR: 4 AT before the switch date
/// ([`rules::AKM_TR_ESCALATION`]).
#[must_use]
pub fn akm_tr_escalation(wechseltermin: Date, cal: &Calendar) -> Deadline {
    lead(DeadlineKind::AkmTrEscalation, Role::EkpAuf, wechseltermin, 4, rules::AKM_TR_ESCALATION, cal)
}

/// The new pre-agreement after an agreed change: 7 AT before the switch date
/// ([`rules::NEW_VA`]).
#[must_use]
pub fn new_vorabstimmung(wechseltermin: Date, cal: &Calendar) -> Deadline {
    lead(DeadlineKind::NewVorabstimmung, Role::EkpAuf, wechseltermin, 7, rules::NEW_VA, cal)
}

/// The last day a later porting of a number switched off on `abgeschaltet` is agreed:
/// 65 AT after ([`rules::LATER_PORTING`]).
#[must_use]
pub fn later_porting(abgeschaltet: Date, cal: &Calendar) -> Deadline {
    let (day, explanation) = forward(cal, "number switched off", start_of(abgeschaltet), 65);
    Deadline {
        kind: DeadlineKind::LaterPorting,
        party: Role::EkpAuf,
        due: next_midnight(day),
        rule: rules::LATER_PORTING,
        explanation,
    }
}

fn lead(kind: DeadlineKind, party: Role, x: Date, n: u32, rule: Rule, cal: &Calendar) -> Deadline {
    let (day, explanation) = backward(cal, "agreed switch date", x, n);
    Deadline { kind, party, due: start_of(day), rule, explanation }
}

#[cfg(test)]
mod tests {
    use super::*;

    // wbci-switch/SR-003: a timestamp from a peer near the end of time must not panic.
    #[test]
    fn extreme_timestamps_do_not_panic() {
        assert_eq!(teq_escalation(Timestamp::MAX, Role::EkpAuf).due, Timestamp::MAX);
        let cal = Calendar::wbci();
        let _ = va_answer(Timestamp::MAX, &cal);
        let _ = answer_24h(Timestamp::MAX, Role::EkpAbg, &cal);
        let _ = earliest_wechseltermin(Timestamp::MAX, Date::MAX, &cal);
        let _ = storno_aufhebung_latest(Date::MAX, Role::EkpAuf, &cal);
        let _ = later_porting(Date::MAX, &cal);
        let _ = earliest_kundenwunschtermin(Geschaeftsfall::VaKueMrn, Timestamp::MIN, &cal);
    }
}
