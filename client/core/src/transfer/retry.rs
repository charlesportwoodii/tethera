use std::time::Duration;

/// When an interrupted download asks again, and when it stops asking.
///
/// A decision rather than a loop: it takes what it needs as arguments and holds
/// only its own counters, so every rule below is checkable without a phone, a
/// machine or a connection to lose.
///
/// The rules exist because a phone is not a desktop. Switching apps suspends
/// the connection under a transfer that is working, so an interruption has to
/// mean "ask again shortly", and a person coming back has to find a download
/// that continued rather than one that reported failure and discarded its
/// bytes.
pub struct Retry {
    attempts: u32,
    /// How long this download has spent waiting on a locked phone.
    locked_for: Duration,
}

/// What to do after an interruption.
pub enum Next {
    Wait(Duration),
    GiveUp,
}

impl Retry {
    /// How many interruptions a download absorbs before it gives up.
    ///
    /// With the wait below this is about four minutes of asking, which covers a
    /// phone carried between two rooms. Past that the person is somewhere else,
    /// and a row that says so beats a task that never stops.
    pub const ATTEMPTS: u32 = 12;

    /// The longest gap between attempts.
    pub const LONGEST: Duration = Duration::from_secs(30);

    /// How often to look again while this app is locked.
    pub const WHILE_LOCKED: Duration = Duration::from_secs(15);

    /// How long a download waits on a locked phone before giving up.
    ///
    /// Generous, because a locked phone is the ordinary case rather than a
    /// fault: somebody put it in a pocket, and the bytes on disk are safe until
    /// they take it out again. Bounded all the same, so a phone nobody comes
    /// back to does not hold a task open for the life of the process.
    pub const LOCKED_PATIENCE: Duration = Duration::from_secs(30 * 60);

    pub fn new() -> Self {
        Self {
            attempts: 0,
            locked_for: Duration::ZERO,
        }
    }

    /// Decides what happens after one interrupted pass.
    ///
    /// `served` is whether this machine has ever answered with a head for this
    /// asset, or whether bytes from an earlier attempt are already on disk.
    /// `locked` is this app's own launch lock, which refuses every dial.
    pub fn after(&mut self, served: bool, locked: bool) -> Next {
        // Never served and nothing on disk. That is a machine which is not
        // there or an asset that is gone, and neither answer changes because it
        // was asked twelve times from a phone in somebody's pocket.
        if !served {
            return Next::GiveUp;
        }

        // A dial refused because this app is locked is not the machine failing,
        // and it is the one failure certain to clear itself the moment somebody
        // picks the phone up. Counting these would spend the whole budget in
        // seconds and report a download as failed for the best possible reason.
        if locked {
            if self.locked_for >= Self::LOCKED_PATIENCE {
                return Next::GiveUp;
            }

            self.locked_for += Self::WHILE_LOCKED;

            return Next::Wait(Self::WHILE_LOCKED);
        }

        self.attempts += 1;

        if self.attempts >= Self::ATTEMPTS {
            return Next::GiveUp;
        }

        Next::Wait(Self::backoff(self.attempts))
    }

    /// How far along the budget this download is, for a log line.
    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    /// How long to wait before attempt `attempt`.
    ///
    /// Doubling and then flat. The early waits are for a radio that came back
    /// in a second; the flat tail is for a phone in a pocket, where asking
    /// faster changes nothing and costs battery.
    fn backoff(attempt: u32) -> Duration {
        Duration::from_secs(1u64 << attempt.min(5)).min(Self::LONGEST)
    }
}

impl Default for Retry {
    fn default() -> Self {
        Self::new()
    }
}
