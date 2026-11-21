//! Working days (Arbeitstage, AT).
//!
//! A1 V5.4.1 §2.5 counts deadlines in „Arbeitstage (Mo – Fr)"; WBCI 2.0 §12.6 keeps the
//! interface available „an bundeseinheitlichen Arbeitstagen". [`Calendar::wbci`] reads both
//! together: Monday to Friday without the nine nationwide public holidays. That is an
//! interpretation until the certifier confirms it; [`Calendar::mon_fri`] is the literal
//! reading, and [`Calendar::with_closed`] adds regional or company closing days.
//!
//! S/PRI also counts in Werktage (Monday to Saturday) where the Arbeitshandbuch says so, e.g.
//! the 10 Werktage the service provider has to answer a TAM (AH4S/PRI 2.9.5 §3.4):
//! [`Calendar::werktage`] is Monday to Saturday without the nationwide public holidays.

use std::collections::BTreeSet;

use jiff::civil::{Date, Weekday};
use jiff::tz::TimeZone;

static BERLIN: TimeZone = jiff::tz::get!("Europe/Berlin");

/// The `Europe/Berlin` time zone, compiled in (no system database is read).
#[must_use]
pub fn berlin() -> TimeZone {
    BERLIN.clone()
}

/// Why a day is not a working day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Closed {
    /// Saturday or Sunday.
    Weekend,
    /// A nationwide public holiday, by name.
    Holiday(&'static str),
    /// A day the caller closed.
    Extra,
}

impl core::fmt::Display for Closed {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Weekend => f.write_str("weekend"),
            Self::Holiday(name) => f.write_str(name),
            Self::Extra => f.write_str("closed day"),
        }
    }
}

/// Which days count as working days.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Calendar {
    literal: bool,
    saturday: bool,
    extra: BTreeSet<Date>,
}

impl Calendar {
    /// Monday to Friday without the nationwide public holidays — the default, and the reading
    /// of „bundeseinheitliche Arbeitstage" that WBCI 2.0 §12.6 and S/PRI 4.4 §11.3 share.
    #[must_use]
    pub fn wbci() -> Self {
        Self::default()
    }

    /// Monday to Friday, holidays included: the literal reading of A1 §2.5.
    #[must_use]
    pub fn mon_fri() -> Self {
        Self { literal: true, saturday: false, extra: BTreeSet::new() }
    }

    /// Werktage: Monday to Saturday without the nationwide public holidays (AH4S/PRI 2.9.5
    /// §3.4 „10 WT", S/PRI 4.4 §5.1.1 „nächster Werktag (Mo–Sa)").
    #[must_use]
    pub fn werktage() -> Self {
        Self { literal: false, saturday: true, extra: BTreeSet::new() }
    }

    /// Whether Saturday counts (the [`Calendar::werktage`] reading).
    #[must_use]
    pub const fn counts_saturday(&self) -> bool {
        self.saturday
    }

    /// Adds a closed day, e.g. a regional holiday that matters to a party.
    #[must_use]
    pub fn with_closed(mut self, day: Date) -> Self {
        self.extra.insert(day);
        self
    }

    /// Why `day` is not a working day, or `None` if it is one.
    #[must_use]
    pub fn closed(&self, day: Date) -> Option<Closed> {
        match day.weekday() {
            Weekday::Sunday => return Some(Closed::Weekend),
            Weekday::Saturday if !self.saturday => return Some(Closed::Weekend),
            _ => {}
        }
        if !self.literal
            && let Some(name) = holiday(day)
        {
            return Some(Closed::Holiday(name));
        }
        self.extra.contains(&day).then_some(Closed::Extra)
    }

    /// Whether `day` is a working day.
    #[must_use]
    pub fn is_working_day(&self, day: Date) -> bool {
        self.closed(day).is_none()
    }

    /// The `n`-th working day after `day` (A1 §2.5 „X+n AT"); `day` itself is not counted.
    #[must_use]
    pub fn add_working_days(&self, day: Date, n: u32) -> Date {
        self.walk(day, n, 1).0
    }

    /// The `n`-th working day before `day` (A1 §2.5 „X−n AT").
    #[must_use]
    pub fn sub_working_days(&self, day: Date, n: u32) -> Date {
        self.walk(day, n, -1).0
    }

    /// Walks `n` working days in `step` direction (positive: forwards), recording the days
    /// counted and skipped. Returns the last day reached.
    #[must_use]
    pub fn walk(&self, start: Date, n: u32, step: i8) -> (Date, Vec<Date>, Vec<(Date, Closed)>) {
        let mut day = start;
        let mut counted = Vec::new();
        let mut skipped = Vec::new();
        while counted.len() < n as usize {
            day = if step > 0 { day.tomorrow() } else { day.yesterday() }.unwrap_or(day);
            match self.closed(day) {
                None => counted.push(day),
                Some(reason) => skipped.push((day, reason)),
            }
            if day == Date::MAX || day == Date::MIN {
                break;
            }
        }
        (day, counted, skipped)
    }
}

/// The nationwide public holiday on `day`, if any.
///
/// Neujahr, Karfreitag, Ostermontag, Tag der Arbeit, Christi Himmelfahrt, Pfingstmontag,
/// Tag der Deutschen Einheit, 1. and 2. Weihnachtstag.
#[must_use]
pub fn holiday(day: Date) -> Option<&'static str> {
    let fixed = match (day.month(), day.day()) {
        (1, 1) => Some("Neujahr"),
        (5, 1) => Some("Tag der Arbeit"),
        (10, 3) => Some("Tag der Deutschen Einheit"),
        (12, 25) => Some("1. Weihnachtstag"),
        (12, 26) => Some("2. Weihnachtstag"),
        _ => None,
    };
    if fixed.is_some() {
        return fixed;
    }
    let easter = easter_sunday(day.year())?;
    let offset = (day - easter).get_days();
    match offset {
        -2 => Some("Karfreitag"),
        1 => Some("Ostermontag"),
        39 => Some("Christi Himmelfahrt"),
        50 => Some("Pfingstmontag"),
        _ => None,
    }
}

/// Easter Sunday in the Gregorian calendar (anonymous Gregorian algorithm).
#[must_use]
#[allow(clippy::many_single_char_names)] // the algorithm's own notation
pub fn easter_sunday(year: i16) -> Option<Date> {
    let y = i32::from(year);
    let a = y % 19;
    let b = y / 100;
    let c = y % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    Date::new(year, i8::try_from(month).ok()?, i8::try_from(day).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    #[test]
    fn easter_dates() {
        assert_eq!(easter_sunday(2024), Some(date(2024, 3, 31)));
        assert_eq!(easter_sunday(2026), Some(date(2026, 4, 5)));
        assert_eq!(easter_sunday(2027), Some(date(2027, 3, 28)));
        assert_eq!(easter_sunday(2038), Some(date(2038, 4, 25)));
    }

    #[test]
    fn holidays_2026() {
        assert_eq!(holiday(date(2026, 4, 3)), Some("Karfreitag"));
        assert_eq!(holiday(date(2026, 4, 6)), Some("Ostermontag"));
        assert_eq!(holiday(date(2026, 5, 14)), Some("Christi Himmelfahrt"));
        assert_eq!(holiday(date(2026, 5, 25)), Some("Pfingstmontag"));
        assert_eq!(holiday(date(2026, 10, 3)), Some("Tag der Deutschen Einheit"));
        assert_eq!(holiday(date(2026, 11, 20)), None);
    }

    #[test]
    fn counting_skips_weekends_and_holidays() {
        let cal = Calendar::wbci();
        // Wednesday 23.12.2026 + 2 AT: Thu 24 counts, 25/26 holiday/weekend, 27 Sunday, Mon 28.
        assert_eq!(cal.add_working_days(date(2026, 12, 23), 2), date(2026, 12, 28));
        assert_eq!(Calendar::mon_fri().add_working_days(date(2026, 12, 23), 2), date(2026, 12, 25));
        assert_eq!(cal.sub_working_days(date(2026, 4, 7), 1), date(2026, 4, 2));
        let closed = Calendar::wbci().with_closed(date(2026, 11, 23));
        assert_eq!(closed.add_working_days(date(2026, 11, 20), 1), date(2026, 11, 24));
    }

    #[test]
    fn werktage_count_saturdays_but_not_holidays() {
        let cal = Calendar::werktage();
        assert!(cal.counts_saturday());
        assert!(!Calendar::wbci().counts_saturday());
        assert!(cal.is_working_day(date(2026, 11, 21))); // Saturday
        assert_eq!(cal.closed(date(2026, 11, 22)), Some(Closed::Weekend)); // Sunday
        assert_eq!(cal.closed(date(2026, 12, 26)), Some(Closed::Holiday("2. Weihnachtstag"))); // a Saturday
        // Friday 20.11.2026 + 10 WT: Sa 21, Mo 23 … Sa 28, Mo 30, Di 01.12, Mi 02.12.
        assert_eq!(cal.add_working_days(date(2026, 11, 20), 10), date(2026, 12, 2));
        assert_eq!(Calendar::wbci().add_working_days(date(2026, 11, 20), 10), date(2026, 12, 4));
        // Wednesday 23.12.2026 + 2 WT: Thu 24, (25, 26 holidays, 27 Sunday), Mon 28.
        assert_eq!(cal.add_working_days(date(2026, 12, 23), 2), date(2026, 12, 28));
        assert_eq!(cal.sub_working_days(date(2026, 11, 23), 1), date(2026, 11, 21));
        let closed = Calendar::werktage().with_closed(date(2026, 11, 21));
        assert_eq!(closed.closed(date(2026, 11, 21)), Some(Closed::Extra));
        // The other calendars are unchanged.
        assert_eq!(Calendar::wbci().closed(date(2026, 11, 21)), Some(Closed::Weekend));
        assert_eq!(Calendar::mon_fri().closed(date(2026, 11, 21)), Some(Closed::Weekend));
    }

    #[test]
    fn berlin_is_compiled_in() {
        let ts = date(2026, 7, 1).at(12, 0, 0, 0).to_zoned_datetime(berlin()).unwrap().timestamp();
        assert_eq!(berlin().to_offset(ts).seconds(), 7200);
    }
}
