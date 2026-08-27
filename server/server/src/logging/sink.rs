use curia::{ConsoleSink, FileSink, LogEvent, Level, Sink};

/// The sinks this machine writes to.
///
/// An enum rather than `Box<dyn Sink>` because `Dispatcher<S: Sink>` is generic
/// over exactly one sink type, so a console and a file cannot share a vector
/// without one. Adding a third sink then touches this file and nothing else.
pub enum LogSink {
    Console(ConsoleSink),
    File(FileSink),
}

impl Sink for LogSink {
    fn level(&self) -> Level {
        match self {
            Self::Console(sink) => sink.level(),
            Self::File(sink) => sink.level(),
        }
    }

    fn emit(&self, event: &LogEvent) {
        match self {
            Self::Console(sink) => sink.emit(event),
            Self::File(sink) => sink.emit(event),
        }
    }
}
