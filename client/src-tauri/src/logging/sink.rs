use tauri::Wry;
use tauri_plugin_curia::curia::{Level, LogEvent, Sink};
use tauri_plugin_curia::{ConsoleSink, FileSink, WebviewSink};

/// The sinks this client writes to.
///
/// An enum rather than `Box<dyn Sink>` because `Dispatcher<S: Sink>` is generic
/// over exactly one sink type, so sinks with different jobs cannot share a
/// vector without one.
pub enum LogSink {
    Console(ConsoleSink),
    File(FileSink),
    /// Carries records into the webview, so the front end's own diagnostics and
    /// the Rust side's appear in one stream rather than two.
    Webview(WebviewSink<Wry>),
}

impl Sink for LogSink {
    fn level(&self) -> Level {
        match self {
            Self::Console(sink) => sink.level(),
            Self::File(sink) => sink.level(),
            Self::Webview(sink) => sink.level(),
        }
    }

    fn emit(&self, event: &LogEvent) {
        match self {
            Self::Console(sink) => sink.emit(event),
            Self::File(sink) => sink.emit(event),
            Self::Webview(sink) => sink.emit(event),
        }
    }
}
