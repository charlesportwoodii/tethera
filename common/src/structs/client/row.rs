use crate::protocol::handshake::RefuseReason;
use crate::structs::client::ServerEntry;
use crate::structs::link::Link;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One row of a client's server list.
///
/// The entry is remembered; the link is measured on the current sweep and never
/// persisted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct ServerRow {
    pub entry: ServerEntry,
    pub link: Link,
    /// Set when the machine answered and turned this client away. A refusal is a
    /// different sentence from "no route": the machine is up, so the network is
    /// not the thing to go and debug.
    pub refusal: Option<RefuseReason>,
}
