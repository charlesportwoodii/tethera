use crate::protocol::handshake::ClientHello;
use crate::protocol::request::Request;
use crate::protocol::terminal::AttachSpec;
use crate::protocol::transfer::{FetchSpec, PutSpec};
use crate::protocol::watch::WatchSpec;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The first frame on any client-opened stream.
///
/// QUIC is the multiplexer: each operation gets a stream, this frame says which
/// operation, and after it the stream is typed. That is what removes the need
/// for a request id, a correlation table, or a multiplexing layer of our own.
///
/// Cancellation is resetting the stream, which both ends observe. The
/// predecessor dropped an HTTP handler future on client disconnect, taking any
/// in-flight backend call with it, and logged nothing at either end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub enum StreamOpen {
    /// Mandatory first stream on a connection.
    Hello(ClientHello),
    /// One request, one terminal response, both sides FIN.
    Rpc(Request),
    /// Snapshot then events, long-lived.
    Watch(WatchSpec),
    /// Terminal frames down, input up, long-lived.
    Attach(AttachSpec),
    /// A head frame, then raw bytes to FIN.
    Fetch(FetchSpec),
    /// A ready frame, then raw bytes to FIN, then a result frame.
    Put(PutSpec),
}

impl StreamOpen {
    /// Whether this stream may be opened before the handshake has completed.
    ///
    /// Only `Hello` may. Stated here rather than inside the dispatcher so the
    /// rule is testable without a connection, and so a new stream kind cannot be
    /// added without deciding the question.
    pub fn is_permitted_before_handshake(&self) -> bool {
        matches!(self, Self::Hello(_))
    }
}
