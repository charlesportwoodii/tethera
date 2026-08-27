mod state;

pub use state::DeviceState;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Device {
    pub id: String,
    pub name: String,
    pub endpoint_id: String,
    pub state: DeviceState,
    // ts-rs maps i64 to bigint, and these reach the client as JSON where
    // `JSON.parse` never produces one. Without the override the binding
    // describes a runtime value that cannot occur.
    #[ts(type = "number | null")]
    pub paired_at: Option<i64>,
    #[ts(type = "number | null")]
    pub last_seen_at: Option<i64>,
}

impl Device {
    pub fn new(id: String, name: String, endpoint_id: String) -> Self {
        Self {
            id,
            name,
            endpoint_id,
            state: DeviceState::Pending,
            paired_at: None,
            last_seen_at: None,
        }
    }
}
