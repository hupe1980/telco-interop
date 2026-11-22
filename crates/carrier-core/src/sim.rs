//! What the simulated peers of both interfaces share: how long they take to answer.
//!
//! The peers (`wbci::sim`, `spri::sim`) read no clock. A peer answers an inbound call with its
//! technical receipt after [`Latency::receipt`] and sends a business answer [`Latency::answer`]
//! after the event it answers. The documents set the deadlines, not the moment inside them;
//! the defaults are well inside every deadline: a receipt after 2 seconds (WBCI escalates after
//! 2 hours, Anl. 1 V5.4.1 §2.6.1; S/PRI expects it within 30 seconds, S/PRI 4.4 §11.2.3), an
//! answer after 1 hour (the shortest business deadline is 24 hours counted on working days,
//! Anl. 1 V5.4.1 §2.9–§2.11).

use jiff::SignedDuration;

/// How long a simulated peer takes: to send a receipt, and to answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Latency {
    /// From an inbound call to its technical receipt (TEQ).
    pub receipt: SignedDuration,
    /// From the event a business message answers to the message.
    pub answer: SignedDuration,
}

impl Latency {
    /// 2 seconds to the receipt, 1 hour to an answer.
    pub const DEFAULT: Self = Self { receipt: SignedDuration::from_secs(2), answer: SignedDuration::from_hours(1) };

    /// The receipt delay, at least zero and at most `max` (the receipt's deadline), so a
    /// receipt never leaves its deadline.
    #[must_use]
    pub fn receipt_within(&self, max: SignedDuration) -> SignedDuration {
        self.receipt.clamp(SignedDuration::ZERO, max.max(SignedDuration::ZERO))
    }

    /// The answer delay, at least zero.
    #[must_use]
    pub fn answer(&self) -> SignedDuration {
        self.answer.max(SignedDuration::ZERO)
    }
}

impl Default for Latency {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_clamping() {
        let l = Latency::default();
        assert_eq!(l.receipt, SignedDuration::from_secs(2));
        assert_eq!(l.answer(), SignedDuration::from_hours(1));
        let slow = Latency { receipt: SignedDuration::from_mins(5), answer: SignedDuration::from_secs(-1) };
        assert_eq!(slow.receipt_within(SignedDuration::from_secs(30)), SignedDuration::from_secs(30));
        assert_eq!(slow.answer(), SignedDuration::ZERO);
        let negative = Latency { receipt: SignedDuration::from_secs(-3), ..l };
        assert_eq!(negative.receipt_within(SignedDuration::from_secs(30)), SignedDuration::ZERO);
    }
}
