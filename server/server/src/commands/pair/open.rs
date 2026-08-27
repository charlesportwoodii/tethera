use crate::config::ApplicationConfig;
use crate::identity::Identity;
use crate::machine::{Machine, MachineAddress, Offer};
use crate::services::PairingService;
use crate::storage::Storage;
use std::sync::Arc;

/// Opens a pairing window and shows what a person has to see.
///
/// The address, as a QR the phone can scan, and the code, which is the consent.
/// The QR carries no secret: it is where this machine is, and it stays valid
/// indefinitely. What stops a stranger is this window, which a human opened by
/// running this command.
// Flattened into `pair::Config`, which is also named `Config`, and clap names an
// argument group after each `Args` struct. Two groups of one name in one command
// is a panic at startup, so this one declares none.
#[derive(clap::Args, Debug, Clone)]
#[group(skip)]
pub struct Config {
    /// How long the window stays open
    #[clap(long, default_value_t = PairingService::DEFAULT_TTL_SECONDS)]
    pub ttl_seconds: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ttl_seconds: PairingService::DEFAULT_TTL_SECONDS,
        }
    }
}

impl Config {
    pub async fn run(&self, config: Arc<ApplicationConfig>) -> anyhow::Result<()> {
        let endpoint_id = Identity::load_or_create(&config.identity_path())?
            .public()
            .to_string();

        let now = chrono::Utc::now().timestamp();
        let connection = Storage::connect(&config).await?;
        let pairing = PairingService::new(Arc::new(connection));
        let (plaintext, superseded) = pairing.open_window(self.ttl_seconds, now).await?;

        let offer = Offer::build(&config, &endpoint_id, now);
        let uri = offer.to_uri();

        println!("{}", Offer::qr(&uri)?);
        println!("{uri}");
        println!();
        println!("machine: {}", Machine::label(&config, &endpoint_id));

        // The only time this value is ever emitted. It is not logged, not
        // returned, and not recoverable from the row that was just written.
        println!("code:    {plaintext}");
        println!(
            "the window is open for {} seconds, and {} attempts",
            self.ttl_seconds,
            PairingService::DEFAULT_ATTEMPTS
        );

        Self::report_superseded(superseded);
        Self::warn_if_nothing_is_listening(&config, &endpoint_id, now);

        Ok(())
    }

    // Said out loud, because the code somebody is already looking at stopped
    // working the moment this ran.
    fn report_superseded(count: usize) {
        if count == 0 {
            return;
        }

        println!();
        println!(
            "{count} earlier pairing window(s) were closed; any code still on screen no longer works"
        );
    }

    // A window nothing is serving is a code a person types into a screen that
    // never answers. The record is rewritten by the running server on a timer,
    // so its absence is the closest thing to a liveness check this command has.
    fn warn_if_nothing_is_listening(
        config: &ApplicationConfig,
        endpoint_id: &str,
        now: i64,
    ) {
        let running = MachineAddress::read(config)
            .is_some_and(|record| record.is_fresh(now) && record.endpoint_id == endpoint_id);

        if running {
            return;
        }

        println!();
        println!(
            "no server is running on this machine, so nothing will answer the phone that scans this."
        );
        println!("start one with: tethera server start");
    }
}
