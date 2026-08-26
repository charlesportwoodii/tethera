use super::{approve, ban, list, revoke, unban};

#[derive(clap::Subcommand, Debug, Clone)]
pub enum DeviceSubCommand {
    /// List every known device and its state
    List(list::Config),
    /// Approve a pending device with its pairing code
    Approve(approve::Config),
    /// Revoke an approved device
    Revoke(revoke::Config),
    /// Ban a device
    Ban(ban::Config),
    /// Lift a ban, returning the device to pending so it must pair again
    Unban(unban::Config),
}
