use crate::structs::client::ServerEntry;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Every way submitting a typed pairing code can end.
///
/// There is no `WindowClosed` here, and that is deliberate. Once the machine has
/// written `EnrollPending` it answers only with `EnrollResult`, whose refusal
/// reason is always `NotEnrolled`, so `attempts_left` is the entire signal. A
/// `WindowClosed` variant would be unreachable, and an unreachable variant is a
/// lie in the type: a screen would carry a branch and a sentence for a state
/// that can never arrive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum PairOutcome {
    Paired(ServerEntry),
    /// The enrolment stream stays usable. Somebody mistyping six digits is the
    /// common case, not the exceptional one.
    WrongCode { attempts_left: u8 },
    /// `attempts_left` came back zero and the machine ended the stream.
    ///
    /// Deliberately not named after guesses: zero means the attempts are spent
    /// *or* that no window was open when the code arrived, and a client cannot
    /// tell which. Both are fixed by opening a new window, so the name points at
    /// the window rather than claiming a number of wrong guesses.
    WindowSpent,
    /// The connection died mid-exchange. Not a refusal, and the person has done
    /// nothing wrong.
    LinkLost,
}
