use super::{code, qr};

// The two halves of `tethera pair`, for a caller that wants one of them alone.
#[derive(clap::Subcommand, Debug, Clone)]
pub enum PairSubCommand {
    /// Print the pairing offer as a scannable QR code, without opening a window
    Qr(qr::Config),
    /// Open a pairing window and print only its code
    Code(code::Config),
}
