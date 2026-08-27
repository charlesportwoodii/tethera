use crate::link::Measure;
use std::time::Duration;

/// How long a sweep may spend, at each of its three scales.
///
/// A plain config struct rather than three arguments, so a caller that wants one
/// value different does not have to restate the others. The chained `with_*`
/// methods return `Self`; there is no separate builder type and no `build()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SweepBudget {
    /// How long to let a path settle before believing it. Zero measures once and
    /// reports whatever is selected, which is what the "hold out for a direct
    /// path" setting turns off.
    pub settle: Duration,
    /// How long one machine may take to answer.
    pub dial: Duration,
    /// How long the whole pass may take, so one hanging machine cannot leave the
    /// list spinning.
    pub total: Duration,
}

impl SweepBudget {
    /// A relay-assisted connect from cold takes seconds. Shorter reports a
    /// working machine as dead on a slow mobile path.
    pub const DIAL: Duration = Duration::from_secs(10);

    pub const TOTAL: Duration = Duration::from_secs(20);

    pub fn new() -> Self {
        Self {
            settle: Measure::SETTLE,
            dial: Self::DIAL,
            total: Self::TOTAL,
        }
    }

    pub fn with_settle(mut self, settle: Duration) -> Self {
        self.settle = settle;
        self
    }

    pub fn with_dial(mut self, dial: Duration) -> Self {
        self.dial = dial;
        self
    }

    pub fn with_total(mut self, total: Duration) -> Self {
        self.total = total;
        self
    }
}

impl Default for SweepBudget {
    fn default() -> Self {
        Self::new()
    }
}
