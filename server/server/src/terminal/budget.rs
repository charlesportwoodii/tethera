use std::time::Duration;

use tokio::time::Instant;

/// The rate at which one attach is allowed to emit frames.
///
/// Thirty a second is faster than a person distinguishes on a phone at arm's
/// length, and slow enough that a build writing thousands of lines a second
/// collapses into thirty frames rather than thousands. The number matters far
/// less than that the bound exists: exceeding it merges into the next frame
/// rather than accumulating, because the emulator's own state is the buffer and
/// there is no frame queue at all.
pub struct FrameBudget {
    last: Option<Instant>,
}

impl FrameBudget {
    pub const MAX_FRAMES_PER_SECOND: u32 = 30;
    pub const MIN_FRAME_INTERVAL: Duration =
        Duration::from_nanos(1_000_000_000 / Self::MAX_FRAMES_PER_SECOND as u64);

    pub fn new() -> Self {
        Self { last: None }
    }

    /// Returns as soon as another frame is allowed.
    ///
    /// An idle pane's first output is never delayed, because the previous
    /// emission is already older than the interval.
    pub async fn ready(&mut self) {
        if let Some(last) = self.last {
            let next = last + Self::MIN_FRAME_INTERVAL;

            if next > Instant::now() {
                tokio::time::sleep_until(next).await;
            }
        }
    }

    pub fn spent(&mut self) {
        self.last = Some(Instant::now());
    }
}

impl Default for FrameBudget {
    fn default() -> Self {
        Self::new()
    }
}
