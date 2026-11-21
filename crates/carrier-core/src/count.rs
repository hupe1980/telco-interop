//! Counting working days and working hours, with the arithmetic kept for the reader.
//!
//! Counting (WBCI Anl. 1 V5.4.1 §2.5): forwards, „X+n AT" ends at 23:59 of the n-th working
//! day after X; backwards, „X−n AT" means before 0:00 of the n-th working day before X. „24
//! Stunden (bezogen auf Arbeitstage)" is read as 24 hours of elapsed time counted on working
//! days only (an interpretation). All arithmetic happens in `Europe/Berlin`.

use core::fmt;

use jiff::civil::{Date, Weekday};
use jiff::{SignedDuration, Timestamp};

use crate::calendar::{Calendar, Closed, berlin};

/// How a deadline was computed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Explanation {
    /// The event that starts the clock, e.g. „positive TEQ".
    pub start: &'static str,
    /// The day or instant counting starts from.
    pub from: Timestamp,
    /// Working days counted, in counting order.
    pub counted: Vec<Date>,
    /// Days skipped and why.
    pub skipped: Vec<(Date, Closed)>,
    /// The counting rule applied.
    pub method: &'static str,
}

impl fmt::Display for Explanation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let z = self.from.to_zoned_datetime(berlin());
        if z.hour() == 0 && z.minute() == 0 && z.second() == 0 {
            write!(f, "{} {}; {}", self.start, Day(z.date()), self.method)?;
        } else {
            write!(f, "{} at {} {:02}:{:02}; {}", self.start, Day(z.date()), z.hour(), z.minute(), self.method)?;
        }
        if !self.counted.is_empty() {
            write!(f, "; counted ")?;
            for (i, d) in self.counted.iter().enumerate() {
                write!(f, "{}{}", if i == 0 { "" } else { ", " }, Day(*d))?;
            }
        }
        if !self.skipped.is_empty() {
            write!(f, "; skipped ")?;
            for (i, (d, why)) in self.skipped.iter().enumerate() {
                write!(f, "{}{} ({why})", if i == 0 { "" } else { ", " }, Day(*d))?;
            }
        }
        Ok(())
    }
}

/// Formats a date as `Do 26.11.2026`.
#[derive(Debug, Clone, Copy)]
pub struct Day(pub Date);

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let wd = match self.0.weekday() {
            Weekday::Monday => "Mo",
            Weekday::Tuesday => "Di",
            Weekday::Wednesday => "Mi",
            Weekday::Thursday => "Do",
            Weekday::Friday => "Fr",
            Weekday::Saturday => "Sa",
            Weekday::Sunday => "So",
        };
        write!(f, "{wd} {:02}.{:02}.{}", self.0.day(), self.0.month(), self.0.year())
    }
}

/// Formats an exclusive instant in German time: midnight reads as `23:59` of the day before.
#[derive(Debug, Clone, Copy)]
pub struct Berlin(pub Timestamp);

impl fmt::Display for Berlin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let z = self.0.to_zoned_datetime(berlin());
        let (day, hm) = if z.hour() == 0 && z.minute() == 0 && z.second() == 0 && z.subsec_nanosecond() == 0 {
            (z.date().yesterday().unwrap_or(z.date()), (23, 59))
        } else {
            (z.date(), (z.hour(), z.minute()))
        };
        write!(f, "{} {:02}:{:02}", Day(day), hm.0, hm.1)
    }
}

/// 0:00 of `day` in German time.
#[must_use]
pub fn start_of(day: Date) -> Timestamp {
    day.to_zoned_datetime(berlin()).map_or(Timestamp::MAX, |z| z.timestamp())
}

/// The German calendar day of `ts`.
#[must_use]
pub fn day_of(ts: Timestamp) -> Date {
    ts.to_zoned_datetime(berlin()).date()
}

/// 0:00 of the day after `day`: the exclusive end of `day`.
#[must_use]
pub fn next_midnight(day: Date) -> Timestamp {
    day.tomorrow().map_or(Timestamp::MAX, start_of)
}

const FORWARD: &str = "forwards: ends 23:59 of the last working day counted";
const BACKWARD: &str = "backwards: before 0:00 of the last working day counted";
const HOURS: &str = "24 hours counted on working days only";

/// „X+n AT" from the instant `at`: the n-th working day after the German day of `at`.
#[must_use]
pub fn forward(cal: &Calendar, start: &'static str, at: Timestamp, n: u32) -> (Date, Explanation) {
    let (day, counted, skipped) = cal.walk(day_of(at), n, 1);
    (day, Explanation { start, from: at, counted, skipped, method: FORWARD })
}

/// „X−n AT" from the day `x`: the n-th working day before it.
#[must_use]
pub fn backward(cal: &Calendar, start: &'static str, x: Date, n: u32) -> (Date, Explanation) {
    let (day, counted, skipped) = cal.walk(x, n, -1);
    (day, Explanation { start, from: start_of(x), counted, skipped, method: BACKWARD })
}

/// `hours` hours of elapsed time counted on working days only, from `at`, forwards or
/// backwards.
#[must_use]
pub fn working_hours(
    cal: &Calendar,
    start: &'static str,
    at: Timestamp,
    hours: i64,
    forward_dir: bool,
) -> (Timestamp, Explanation) {
    let mut remaining = SignedDuration::from_hours(hours);
    let mut cursor = at;
    let mut counted = Vec::new();
    let mut skipped = Vec::new();
    for _ in 0..400 {
        let day = if forward_dir { day_of(cursor) } else { day_of(shift(cursor, SignedDuration::from_nanos(-1))) };
        let (lo, hi) = (start_of(day), day.tomorrow().map_or(Timestamp::MAX, start_of));
        if let Some(why) = cal.closed(day) {
            skipped.push((day, why));
            cursor = if forward_dir { hi } else { lo };
            continue;
        }
        counted.push(day);
        let available = if forward_dir { hi.duration_since(cursor) } else { cursor.duration_since(lo) };
        if remaining <= available {
            let due = if forward_dir { shift(cursor, remaining) } else { shift(cursor, -remaining) };
            return (due, Explanation { start, from: at, counted, skipped, method: HOURS });
        }
        remaining -= available;
        cursor = if forward_dir { hi } else { lo };
    }
    (cursor, Explanation { start, from: at, counted, skipped, method: HOURS })
}

/// `at + by`, clamped to the representable range instead of panicking: timestamps can come
/// from another operator's XML, and deadline arithmetic must not crash on extreme values.
#[must_use]
pub fn shift(at: Timestamp, by: SignedDuration) -> Timestamp {
    at.checked_add(by).unwrap_or(if by.is_negative() { Timestamp::MIN } else { Timestamp::MAX })
}

#[cfg(test)]
mod tests {
    #[test]
    fn extreme_instants_saturate() {
        let cal = crate::Calendar::wbci();
        assert_eq!(super::shift(Timestamp::MAX, SignedDuration::from_hours(2)), Timestamp::MAX);
        assert_eq!(super::shift(Timestamp::MIN, SignedDuration::from_hours(-2)), Timestamp::MIN);
        let _ = super::working_hours(&cal, "x", Timestamp::MAX, 24, true);
        let _ = super::working_hours(&cal, "x", Timestamp::MIN, 24, false);
    }

    use super::*;
    use jiff::civil::date;

    #[test]
    fn forward_counts_and_explains() {
        let at: Timestamp = "2026-11-23T08:00:00Z".parse().unwrap();
        let (day, e) = forward(&Calendar::wbci(), "sent", at, 3);
        assert_eq!(day, date(2026, 11, 26));
        assert_eq!(
            e.to_string(),
            "sent at Mo 23.11.2026 09:00; forwards: ends 23:59 of the last working day counted; counted Di 24.11.2026, Mi 25.11.2026, Do 26.11.2026"
        );
        assert_eq!(Berlin(next_midnight(day)).to_string(), "Do 26.11.2026 23:59");
    }

    #[test]
    fn working_hours_skip_the_weekend() {
        let at: Timestamp = "2026-11-27T11:00:00Z".parse().unwrap(); // Friday 12:00 Berlin
        let (due, e) = working_hours(&Calendar::wbci(), "TEQ", at, 24, true);
        assert_eq!(due, "2026-11-30T11:00:00Z".parse::<Timestamp>().unwrap());
        assert_eq!(e.skipped.len(), 2);
    }
}
