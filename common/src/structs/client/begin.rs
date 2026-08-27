use crate::structs::client::{FoundServer, ServerEntry};
use crate::structs::ids::ServerId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Every way opening a pairing attempt can end.
///
/// A variant rather than an error string. Each of these needs a different next
/// action on screen, and branching on prose would make a reworded message a
/// behaviour change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum BeginOutcome {
    /// The only outcome that leaves an enrolment stream open for a code.
    Found(FoundServer),
    /// This machine already knows this client. Re-scanning is not an error.
    AlreadyPaired(ServerEntry),
    /// The offer named one machine and another answered.
    IdMismatch { scanned: String, answered: ServerId },
    WindowClosed,
    /// Also arrives when the machine's device table is unreadable, because its
    /// enrolment lookup fails closed. Never render this as an accusation.
    Revoked,
    NoCommonVersion,
    /// The machine is already serving as many connections as it will.
    AtCapacity,
    /// A deliberate close carrying a code this build does not know, which most
    /// likely means the machine is newer than the client. Never folded into
    /// `AtCapacity`: "try again shortly" is the wrong instruction for a refusal
    /// that means something else.
    ClosedByMachine {
        #[ts(type = "number")]
        code: u32,
    },
    /// Nothing answered.
    Unreachable,
}
