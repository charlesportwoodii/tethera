use super::{code, qr};

#[derive(clap::Subcommand, Debug, Clone)]
pub enum PairSubCommand {
    /// Print the pairing offer as a scannable QR code
    Qr(qr::Config),
    /// Issue a short-lived pairing code
    Code(code::Config),
}
