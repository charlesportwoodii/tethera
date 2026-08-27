use crate::config::ApplicationConfig;
use std::sync::Arc;

/// Approval moved to the wire, and this says so rather than pretending.
///
/// A device is approved by redeeming a code over its own connection, inside the
/// transaction that consumes that code — which is what makes the code single
/// use. There is no second path that could approve a device from here without
/// reopening the hole that transaction closes.
#[derive(clap::Args, Debug, Clone)]
pub struct Config {
    /// The device this would have approved
    pub id: String,

    /// Retained so an existing script fails with an explanation rather than an
    /// unrecognised-argument error
    #[clap(long)]
    pub code: Option<String>,
}

impl Config {
    pub async fn run(&self, _config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        anyhow::bail!(
            "a device approves itself by typing the pairing code over its own connection.\n\
             run `tethera pair` on this machine and enter the six digits on the device"
        )
    }
}
