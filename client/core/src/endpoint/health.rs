use iroh::endpoint::RelayStatus;
use std::fmt;
use std::net::SocketAddr;

/// What this device's endpoint looks like from the inside, right now.
///
/// One line for a log, and the only way to tell a resume that recovered from one
/// that did not. A phone that comes back from suspension either rebound its
/// sockets and reconnected its relay or it did not, and the difference is
/// invisible from the screen: both look like a machine that will not answer.
pub struct EndpointHealth {
    sockets: Vec<SocketAddr>,
    /// Formatted here rather than held as `RelayStatus`, so nothing downstream
    /// has to know how a relay reports itself in order to write it down.
    relays: Vec<String>,
}

impl EndpointHealth {
    pub fn new(sockets: Vec<SocketAddr>, relays: Vec<RelayStatus>) -> Self {
        let relays = relays
            .iter()
            .map(|relay| match (relay.is_connected(), relay.last_error()) {
                (true, _) => format!("{}=connected", relay.url()),
                (false, Some(error)) => format!("{}=disconnected({error})", relay.url()),
                (false, None) => format!("{}=disconnected", relay.url()),
            })
            .collect();

        Self { sockets, relays }
    }
}

impl fmt::Display for EndpointHealth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sockets = self
            .sockets
            .iter()
            .map(|socket| socket.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        // Named rather than left empty, because an endpoint with no relay and an
        // endpoint whose relay has not been selected yet read the same otherwise,
        // and on a phone the first is a machine that can never be reached cold.
        let relays = if self.relays.is_empty() {
            "none".to_string()
        } else {
            self.relays.join(", ")
        };

        write!(formatter, "sockets=[{sockets}] relays=[{relays}]")
    }
}
