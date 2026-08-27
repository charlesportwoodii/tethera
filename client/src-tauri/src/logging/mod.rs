mod sink;

pub use sink::LogSink;

use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_curia::curia::{Dispatcher, Level, Logger, TracingBridge};
use tauri_plugin_curia::{ConsoleSink, FileSink, WebviewSink};
use tethera_common::logging::{HumanFormatter, JsonFormatter};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Installs this client's logging.
///
/// Three sinks. The console is what `adb logcat` and a desktop terminal show.
/// The file is JSON and rotates, which is what a person can attach to a report
/// from a phone they are holding in a car park. The webview sink puts the Rust
/// side's records into the same stream the front end writes to, so a pairing
/// failure reads as one sequence rather than two halves that have to be
/// interleaved by hand.
pub struct Logging;

impl Logging {
    pub const FILE_NAME: &'static str = "tethera";

    /// The level for anything this workspace did not write.
    ///
    /// iroh's relay and path actors are conversational at debug, and on a phone
    /// that is the difference between a log somebody can read and a log that
    /// rotates away the moment something goes wrong.
    pub const DEPENDENCY_LEVEL: Level = Level::Warn;

    pub fn install(app: &AppHandle<Wry>) -> Result<(), String> {
        let console = ConsoleSink::new(Self::level(), HumanFormatter::formatter());
        let mut sinks = vec![LogSink::Console(console)];

        // Logging must never take the app down with it. An unwritable log
        // directory degrades to console only and says so once, rather than
        // failing a launch over diagnostics.
        match Self::file(app) {
            Ok(file) => sinks.push(LogSink::File(file)),
            Err(error) => eprintln!("log file unavailable, continuing without it: {error}"),
        }

        sinks.push(LogSink::Webview(WebviewSink::new(
            app.clone(),
            Self::level(),
            HumanFormatter::formatter(),
        )));

        Logger::install(Box::new(Dispatcher::new(sinks))).map_err(|error| error.to_string())?;

        // This app logs through the `log` crate and iroh logs through `tracing`,
        // so both have to arrive. The bridge is a tracing layer, and registering
        // it is what completes the chain: without a subscriber, `log -> tracing`
        // has nothing on the far side and every record is dropped.
        let bridge = TracingBridge::to_global()
            .with_max_level(Self::level())
            .with_target_level("iroh", Self::DEPENDENCY_LEVEL)
            .with_target_level("iroh_relay", Self::DEPENDENCY_LEVEL)
            .with_target_level("hickory", Self::DEPENDENCY_LEVEL)
            .with_target_level("netwatch", Self::DEPENDENCY_LEVEL)
            .with_target_level("portmapper", Self::DEPENDENCY_LEVEL);

        tracing_subscriber::registry().with(bridge).init();

        // Discarded: `init` above already claims the same slot when
        // tracing-subscriber's `tracing-log` feature is on, and "somebody
        // already bridged `log`" is the outcome this wanted.
        let _ = TracingBridge::install_log_capture();

        Ok(())
    }

    fn file(app: &AppHandle<Wry>) -> Result<FileSink, String> {
        let dir = app.path().app_log_dir().map_err(|error| error.to_string())?;

        FileSink::new(
            dir,
            Self::FILE_NAME.to_string(),
            Level::Debug,
            JsonFormatter::formatter(),
        )
        .map_err(|error| error.to_string())
    }

    fn level() -> Level {
        match std::env::var("TETHERA_LOG").ok().as_deref() {
            Some("error") => Level::Error,
            Some("warn") => Level::Warn,
            Some("debug") => Level::Debug,
            Some("trace") => Level::Trace,
            _ => Level::Info,
        }
    }
}
