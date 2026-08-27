mod sink;

pub use sink::LogSink;

use crate::config::ApplicationConfig;
use tethera_common::logging::{HumanFormatter, JsonFormatter};
use curia::{ConsoleSink, Dispatcher, FileSink, Level, Logger, RotationStrategy, TracingBridge};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Installs this machine's logging.
///
/// Two sinks with different jobs. The console carries what an operator watching
/// a terminal needs, in a line they can read. The file carries everything as
/// JSON, because the question asked of a log after the fact is a query, and
/// grepping prose for a field that was never a field is how a debugging session
/// starts badly.
pub struct Logging;

impl Logging {
    pub const FILE_NAME: &'static str = "tethera";

    /// The level for anything this workspace did not write.
    ///
    /// Left at `Warn` because iroh, hickory and rustls at debug will fill a file
    /// with traffic that has nothing to do with a question anybody is asking.
    /// `TETHERA_LOG` raises it when the question *is* about them.
    pub const DEPENDENCY_LEVEL: Level = Level::Warn;

    /// Bytes of one log file, and how many are kept beside it.
    ///
    /// **curia's defaults are 40 KB and `KeepOne`**, which on this server is
    /// between two and four minutes of history. That is shorter than the gap
    /// between something going wrong on an operator's phone and anybody looking
    /// at the log: block transitions read at one moment were gone by the next
    /// command, and a question about a flicker on screen could not be answered
    /// at all. The defaults suit a library writing occasionally; this process
    /// logs a line per listing while a phone is polling it.
    const MAX_LOG_BYTES: u64 = 25 * 1024 * 1024;

    const KEPT_LOG_FILES: usize = 5;

    pub fn install(config: &ApplicationConfig) -> anyhow::Result<()> {
        let console = ConsoleSink::new(Self::console_level(), HumanFormatter::formatter());
        let mut sinks = vec![LogSink::Console(console)];

        // Logging must never take the server down with it. An unwritable
        // directory degrades to console only and says so once, rather than
        // refusing to start.
        match Self::file(config) {
            Ok(file) => sinks.push(LogSink::File(file)),
            Err(error) => eprintln!("log file unavailable, continuing without it: {error}"),
        }

        Logger::install(Box::new(Dispatcher::new(sinks)))?;

        // Every existing `tracing::info!` in this crate keeps working. The
        // bridge is a tracing layer, which is what makes adopting curia a change
        // to one function rather than to every call site.
        let bridge = TracingBridge::to_global()
            .with_max_level(Self::console_level())
            .with_target_level("iroh", Self::DEPENDENCY_LEVEL)
            .with_target_level("iroh_relay", Self::DEPENDENCY_LEVEL)
            .with_target_level("hickory", Self::DEPENDENCY_LEVEL)
            .with_target_level("netwatch", Self::DEPENDENCY_LEVEL)
            .with_target_level("portmapper", Self::DEPENDENCY_LEVEL)
            .with_target_level("rustls", Self::DEPENDENCY_LEVEL)
            .with_target_level("sqlx", Self::DEPENDENCY_LEVEL)
            .with_target_level("sea_orm", Self::DEPENDENCY_LEVEL);

        tracing_subscriber::registry().with(bridge).init();

        // Dependencies that log through the `log` crate rather than `tracing`
        // are otherwise silent. The result is discarded because
        // `tracing_subscriber`'s `init` above already claims the same slot when
        // its `tracing-log` feature is on, and "somebody already bridged `log`"
        // is the outcome this call wanted rather than a failure to report.
        let _ = TracingBridge::install_log_capture();

        Ok(())
    }

    fn file(config: &ApplicationConfig) -> anyhow::Result<FileSink> {
        config.ensure_data_dir()?;

        Ok(FileSink::with_rotation(
            config.log_dir(),
            Self::FILE_NAME.to_string(),
            Level::Debug,
            JsonFormatter::formatter(),
            Self::MAX_LOG_BYTES,
            RotationStrategy::KeepSome(Self::KEPT_LOG_FILES),
            curia::DEFAULT_TIMEZONE_STRATEGY,
            curia::DEFAULT_FILE_OPEN_STRATEGY,
        )?)
    }

    /// `TETHERA_LOG` keeps the name the previous subscriber read, so an operator
    /// with it already set does not have to learn a new one.
    fn console_level() -> Level {
        match std::env::var("TETHERA_LOG").ok().as_deref() {
            Some("error") => Level::Error,
            Some("warn") => Level::Warn,
            Some("debug") => Level::Debug,
            Some("trace") => Level::Trace,
            _ => Level::Info,
        }
    }
}
