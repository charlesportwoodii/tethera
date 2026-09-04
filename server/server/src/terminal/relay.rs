use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tethera_common::protocol::terminal::CloseReason;
use tethera_common::structs::ids::PaneId;
use tethera_common::structs::terminal::Size;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::backend::herdr::HerdrIds;
use crate::terminal::event::PaneEvent;
use crate::terminal::io::PaneIo;
use crate::terminal::link::{Downlink, Uplink};
use crate::terminal::registry::PaneRegistry;
use crate::terminal::source::PaneSource;

/// Accepts shims and adopts the panes they announce.
///
/// One connection is one pane. The shim opens the channel, states which pane it
/// is running in and how large that pane is, and from then on the channel is the
/// pane's byte stream in both directions — the shim's pty output inbound, input
/// for the shell outbound.
///
/// The pane is adopted as `PaneSource::Relayed` rather than `Streamed`. The bytes
/// are equally real; what differs is that the shim is the terminal answering the
/// pty's device queries, so this side must read the stream without replying to
/// it.
pub struct ShimRelay {
    registry: Arc<PaneRegistry>,
    /// Uplinks whose downlink has not arrived yet.
    ///
    /// A shim opens two channels, not one, because on Windows a blocking read
    /// and a blocking write on the same synchronous pipe handle serialize:
    /// measured, a pending `ReadFile` blocked the paired `WriteFile`, the shim's
    /// output thread stalled behind it, ConPTY stopped being drained and the
    /// shell never started at all. Two channels give each direction its own
    /// handle, and cost this pairing.
    waiting: Mutex<HashMap<PaneId, (mpsc::Receiver<Vec<u8>>, mpsc::Sender<PaneEvent>)>>,
    /// How a client tells a relayed pane what size to hold.
    ///
    /// Kept per pane and for the pane's life, because a claim arrives long after
    /// the channel was opened — when somebody picks up their phone.
    claims: Mutex<HashMap<PaneId, (mpsc::Sender<Size>, mpsc::Sender<PaneEvent>)>>,
}

/// Which half of a pane's stream a channel carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Half {
    /// The shim's pty output, inbound to the emulator.
    Up,
    /// Input for the shell, outbound to the shim.
    Down,
}

impl ShimRelay {
    pub fn new(registry: Arc<PaneRegistry>) -> Self {
        Self {
            registry,
            waiting: Mutex::new(HashMap::new()),
            claims: Mutex::new(HashMap::new()),
        }
    }

    pub fn new_shared(registry: Arc<PaneRegistry>) -> Arc<Self> {
        Arc::new(Self::new(registry))
    }

    fn waiting(
        &self,
    ) -> std::sync::MutexGuard<'_, HashMap<PaneId, (mpsc::Receiver<Vec<u8>>, mpsc::Sender<PaneEvent>)>>
    {
        self.waiting
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn claims(
        &self,
    ) -> std::sync::MutexGuard<'_, HashMap<PaneId, (mpsc::Sender<Size>, mpsc::Sender<PaneEvent>)>>
    {
        self.claims
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Hands a pane's geometry to a viewer, until somebody at the desk types.
    ///
    /// A handoff rather than a negotiation. One shell is one pty and one pty is
    /// one size, so there is no arrangement in which a phone and a desk both get
    /// the width they want — and the honest resolution of that is to say who
    /// currently owns the session rather than to average two answers into one
    /// nobody asked for.
    ///
    /// Sticky on purpose: the claim outlives the attach. A phone that locked its
    /// screen mid-command has not stopped caring what width that command was
    /// laid out for, and a size that sprang back the moment a screen dimmed
    /// would reflow the pane under somebody every time they looked away.
    pub fn claim(&self, pane: &PaneId, size: Size) -> bool {
        let Some((claim, events)) = self.claims().get(pane).cloned() else {
            return false;
        };

        // The emulator is told the same thing as the shim, because the server is
        // what decided it. Waiting for the shim to report the size back would
        // leave every frame between the claim and the report laid out for a
        // geometry the pane no longer has.
        let _ = events.try_send(PaneEvent::Resized(size));

        claim.try_send(size).is_ok()
    }

    /// How long a downlink waits for its uplink to be adopted.
    ///
    /// The shim dials up before down, but the server accepts both and spawns a
    /// task each, so which greeting is read first is a scheduling detail rather
    /// than an ordering guarantee. Measured: the downlink won the race and was
    /// refused. Waiting is correct; requiring an order the wire cannot enforce
    /// is not.
    const PAIR_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
    const PAIR_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

    /// The longest opening line a shim may send.
    ///
    /// A pane id and a size, so this is generous. Bounded because the line is
    /// read from a channel this process does not control the far end of, and an
    /// unbounded `read_line` on a peer that never sends a newline grows until
    /// the process dies.
    const HELLO_LIMIT: u64 = 512;

    /// `<pane id> <cols> <rows>` on one line, then bytes.
    ///
    /// Text rather than postcard, and no version. A shim and the server it dials
    /// are the same binary on the same machine — there is no skew to negotiate,
    /// and a readable first line is worth more here than a compact one, because
    /// this is the boundary somebody will debug with a pipe client.
    pub async fn hello<R>(reader: &mut BufReader<R>) -> anyhow::Result<(PaneId, Size, Half)>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        let mut line = String::new();
        let read = reader
            .take(Self::HELLO_LIMIT)
            .read_line(&mut line)
            .await
            .map_err(|error| anyhow::anyhow!("a shim opened a channel and said nothing: {error}"))?;

        if read == 0 {
            anyhow::bail!("a shim closed its channel before announcing a pane");
        }

        let mut parts = line.trim().split_whitespace();

        let pane = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("a shim announced no pane"))?;
        // Minted, not parsed. The shim announces what herdr told it —
        // `HERDR_PANE_ID`, which is herdr's own `w85:p3` — and tethera's own
        // `PaneId` is that with a prefix. Parsing the native form rejects every
        // real pane, which is a refusal that reads like a malformed peer.
        let pane = HerdrIds::pane(pane);

        // A shim that cannot read its own terminal size still has a usable
        // stream, so a missing or unparseable size is defaulted rather than
        // refused. The pane's real geometry arrives with the first resize.
        let cols = parts.next().and_then(|v| v.parse().ok()).unwrap_or(80);
        let rows = parts.next().and_then(|v| v.parse().ok()).unwrap_or(24);

        let half = match parts.next() {
            Some("down") => Half::Down,
            _ => Half::Up,
        };

        Ok((pane, Size { cols, rows }, half))
    }

    /// Adopts one dialled-in pane and moves bytes until it ends.
    ///
    /// Splits the channel rather than sharing it: the two directions are
    /// independent, and a shim blocked writing output must not stop input
    /// reaching the shell.
    pub async fn serve<S>(self: Arc<Self>, stream: S) -> anyhow::Result<Option<PaneId>>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + 'static,
    {
        let (read, write) = tokio::io::split(stream);
        let mut reader = BufReader::new(read);

        let (pane, size, half) = Self::hello(&mut reader).await?;

        match half {
            Half::Up => {
                // A pane already held is a shim that reconnected, or a second
                // shim in the same pane. Either way the emulator's state
                // describes a stream that has ended, and keeping it would apply
                // new output to an old screen.
                if self.registry.holds(&pane) {
                    self.registry.forget(&pane);
                }

                let (io, events, input) = PaneIo::channel(size);
                self.registry.adopt(pane.clone(), io, PaneSource::Relayed);

                tracing::info!(
                    pane = pane.as_str(),
                    cols = size.cols,
                    rows = size.rows,
                    "a shim relayed a pane"
                );

                Self::spawn_inbound(reader, events.clone());
                self.waiting().insert(pane.clone(), (input, events));

                Ok(Some(pane))
            }
            Half::Down => {
                let held = self.pair(&pane).await;

                match held {
                    Some((input, events)) => {
                        tracing::debug!(pane = pane.as_str(), "a shim opened its input channel");

                        // One outstanding claim is enough. A second arriving
                        // before the first is written describes the same
                        // viewer's newer size, so dropping it costs a resize
                        // that is about to be superseded.
                        let (claims, receiver) = mpsc::channel(1);
                        self.claims().insert(pane.clone(), (claims, events));

                        Self::spawn_outbound(write, input, receiver);
                    }
                    // The uplink is what adopts the pane, so a downlink without
                    // one has nothing to deliver to. Dropped rather than held:
                    // holding it would keep a channel open against a pane that
                    // may never appear.
                    None => anyhow::bail!(
                        "a shim opened an input channel for {} and no output channel followed",
                        pane.as_str()
                    ),
                }

                Ok(None)
            }
        }
    }

    /// Waits for this pane's uplink to hand over its input receiver.
    async fn pair(
        &self,
        pane: &PaneId,
    ) -> Option<(mpsc::Receiver<Vec<u8>>, mpsc::Sender<PaneEvent>)> {
        let deadline = tokio::time::Instant::now() + Self::PAIR_TIMEOUT;

        loop {
            if let Some(input) = self.waiting().remove(pane) {
                return Some(input);
            }

            if tokio::time::Instant::now() >= deadline {
                return None;
            }

            tokio::time::sleep(Self::PAIR_INTERVAL).await;
        }
    }

    /// The shim's pty output and its resizes, into the emulator.
    ///
    /// Header then body, never by chunk: the pipe chooses where an arrival ends,
    /// so treating one as a message would split a resize the first time it was
    /// delivered in two pieces.
    fn spawn_inbound<R>(mut reader: BufReader<R>, events: mpsc::Sender<PaneEvent>)
    where
        R: tokio::io::AsyncRead + Unpin + Send + 'static,
    {
        tokio::spawn(async move {
            let mut header = [0u8; Uplink::HEADER_BYTES];

            loop {
                if reader.read_exact(&mut header).await.is_err() {
                    break;
                }

                let Some(length) = Uplink::payload_length(header) else {
                    tracing::warn!("a shim named an oversize payload");

                    break;
                };

                let mut payload = vec![0u8; length];

                if reader.read_exact(&mut payload).await.is_err() {
                    break;
                }

                // Awaited, unlike the reply path. This is output backpressure,
                // and the right place for it to land is the shim's own write —
                // which backs up into its pty and slows the program down,
                // exactly as a slow terminal would.
                let event = match Uplink::decode(header[0], &payload) {
                    Some(Uplink::Output(bytes)) => PaneEvent::Output(bytes),
                    Some(Uplink::Resized { cols, rows }) => {
                        PaneEvent::Resized(Size { cols, rows })
                    }
                    // Skipped rather than fatal, so a shim from a newer build
                    // does not tear down a pane somebody is working in.
                    None => continue,
                };

                if events.send(event).await.is_err() {
                    return;
                }
            }

            // The pane did not necessarily die — the shim did, or the channel
            // did. `Disconnected` says that rather than claiming the shell
            // exited, because herdr still owns the pane and it is very likely
            // still there.
            let _ = events.send(PaneEvent::Closed(CloseReason::Disconnected)).await;
        });
    }

    /// Input from an attached client, and the geometry it claims, out to the
    /// shim.
    ///
    /// Both go down one channel and both are framed, so a resize can never be
    /// mistaken for something somebody typed. The pty's input has no escape that
    /// would be safe to reserve for a control message, which is the reason this
    /// direction is framed at all.
    fn spawn_outbound<W>(
        mut write: W,
        mut input: mpsc::Receiver<Vec<u8>>,
        mut claims: mpsc::Receiver<Size>,
    ) where
        W: tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        tokio::spawn(async move {
            loop {
                let message = tokio::select! {
                    bytes = input.recv() => match bytes {
                        Some(bytes) => Downlink::Input(bytes),
                        None => return,
                    },
                    claimed = claims.recv() => match claimed {
                        Some(size) => Downlink::Resize {
                            cols: size.cols,
                            rows: size.rows,
                        },
                        // The claim channel closing is not the pane ending. Input
                        // is what this loop exists for.
                        None => continue,
                    },
                };

                let encoded = message.encode();

                if write.write_all(&encoded).await.is_err() || write.flush().await.is_err() {
                    return;
                }
            }
        });
    }
}
