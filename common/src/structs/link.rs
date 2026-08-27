use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How the client reached a machine.
///
/// Measured by the client from its own Iroh endpoint and never sent by the
/// server: the server can describe the connection *it* sees, which is a
/// different connection from the one the phone holds when a relay sits between
/// them. It is typed here so both halves share one vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum LinkKind {
    Direct,
    Relayed,
    /// No path has settled yet. Not the same as offline, and not a guess.
    Unknown,
    /// Nothing is answering.
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Link {
    pub kind: LinkKind,
    /// `None` until a path has settled. Absent is not zero.
    #[ts(type = "number | null")]
    pub rtt_ms: Option<u32>,
}
