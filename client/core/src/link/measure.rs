use iroh::endpoint::Connection;
use std::time::Duration;
use tethera_common::structs::link::{Link, LinkKind};

/// How this device reached a machine.
///
/// iroh 1.1 has no `ConnectionType` - that is a 0.x API, and every example
/// online still shows it. Live path state is `Connection::paths()`.
pub struct Measure;

impl Measure {
    /// How long to let a path settle before believing it.
    ///
    /// Iroh races a relay against a direct address and upgrades when a hole is
    /// punched, so measuring the instant a handshake completes reports `Relayed`
    /// for a machine that is about to be direct. This is the value behind the
    /// "hold out for a direct path" setting; zero turns the wait off.
    pub const SETTLE: Duration = Duration::from_millis(1500);

    /// How often to look while waiting for a path to settle.
    const POLL: Duration = Duration::from_millis(50);

    /// One snapshot, taken now.
    ///
    /// `Path<'a>` borrows the connection and cannot cross a task boundary, so
    /// the owned `Link` is produced here rather than the path being handed out.
    pub fn of(connection: &Connection) -> Link {
        let paths = connection.paths();

        let Some(path) = paths.iter().find(|path| path.is_selected()) else {
            return Link {
                kind: LinkKind::Unknown,
                rtt_ms: None,
            };
        };

        let kind = if path.is_ip() {
            LinkKind::Direct
        } else if path.is_relay() {
            LinkKind::Relayed
        } else {
            LinkKind::Unknown
        };

        Link {
            kind,
            rtt_ms: Some(path.rtt().as_millis().min(u128::from(u32::MAX)) as u32),
        }
    }

    /// Waits for the selected path to stop improving, up to `settle`.
    ///
    /// Returns as soon as a direct path is selected, because nothing better
    /// follows it.
    pub async fn settled(connection: &Connection, settle: Duration) -> Link {
        let deadline = tokio::time::Instant::now() + settle;
        let mut best = Self::of(connection);

        while best.kind != LinkKind::Direct && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Self::POLL).await;

            let now = Self::of(connection);

            // An Unknown reading is a moment with no selected path, not a
            // downgrade. Keeping the better answer stops a transient gap from
            // erasing a measurement already taken.
            if now.kind != LinkKind::Unknown {
                best = now;
            }
        }

        best
    }
}
