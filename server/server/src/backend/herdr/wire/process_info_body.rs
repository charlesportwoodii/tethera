use super::ProcessInfo;
use serde::Deserialize;

/// The `pane_process_info` result, which nests under `process_info`.
#[derive(Debug, Clone, Deserialize)]
pub struct ProcessInfoBody {
    pub process_info: ProcessInfo,
}
