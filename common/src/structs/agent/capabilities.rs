use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct AgentCapabilities {
    pub resume: bool,
    pub interrupt: bool,
    pub file_upload: bool,
    pub questions: bool,
}
