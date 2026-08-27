use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum AgentStatus {
    Working,
    Idle,
    Done,
    Blocked,
    /// A call is in flight and nothing has moved for a while.
    ///
    /// Distinct from `Idle`, which means the agent finished and is waiting for a
    /// person. A stalled agent is not finished — it is stuck, and reporting it as
    /// idle reads as "all done, nothing wrong", which is the one thing it is not.
    ///
    /// Last, and it has to stay last. postcard encodes a variant by its index,
    /// so putting this anywhere else renumbers every variant after it and a
    /// client already shipped decodes `Idle` as `Done`.
    Stalled,
}
