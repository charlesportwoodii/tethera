use crate::structs::ids::ProfileId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One row in a machine's agent catalog.
///
/// The only way an agent's identity crosses the wire, and it crosses as an
/// opaque id plus display text. The client lists profiles and hands an id back;
/// it never branches on which agent it is, so adding an agent is a trait
/// implementation and a catalog row rather than a client release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct AgentProfile {
    pub id: ProfileId,
    pub label: String,
    pub description: Option<String>,
    /// What the machine will actually run.
    pub version: Option<String>,
    pub supports_resume: bool,
    /// Whether a conversation started with this profile will have a readable
    /// transcript at all. The pre-start counterpart of
    /// `Conversation.has_transcript`: a profile answering `false` gives a
    /// terminal and no conversation surface, and the client should say so before
    /// the person commits rather than after.
    pub provides_transcript: bool,
}
