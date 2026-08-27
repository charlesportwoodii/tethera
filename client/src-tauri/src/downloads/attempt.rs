use crate::downloads::Downloads;
use crate::state::AppState;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};
use tethera_client_core::transfer::{Fetch, Next, Partial, Retry};
use tethera_common::protocol::transfer::{FetchHead, FetchSpec};
use tethera_common::structs::client::{DownloadProgress, DownloadState};
use tethera_common::structs::ids::{AssetId, ServerId};

/// One download, carried until it lands, is cancelled, or runs out of tries.
///
/// The retry loop is the feature, not a robustness flourish. Switching apps is
/// how a phone is used, and it suspends the connection under a transfer that is
/// working - so an interruption has to mean "pause and come back", not "start
/// again". `Partial` holds the bytes; this decides when to ask for the rest.
pub struct Attempt {
    app: AppHandle,
    dir: PathBuf,
    id: String,
    server: ServerId,
    asset: AssetId,
    name: String,
    destination: FilePath,
    cancelled: Arc<AtomicBool>,
}

impl Attempt {
    /// How often at most a running transfer reports itself.
    ///
    /// A 403 MiB file is six thousand chunks. Emitting one event per chunk
    /// gives the webview six thousand renders to do while it is also drawing a
    /// transcript, and a progress bar that costs more than the transfer is a
    /// worse answer than no bar.
    const REPORT_EVERY: Duration = Duration::from_millis(200);

    /// How much of the destination is written per pass at the end.
    const COPY_CHUNK: usize = 256 * 1024;

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        app: AppHandle,
        dir: PathBuf,
        id: String,
        server: ServerId,
        asset: AssetId,
        name: String,
        destination: FilePath,
        cancelled: Arc<AtomicBool>,
    ) -> Self {
        Self {
            app,
            dir,
            id,
            server,
            asset,
            name,
            destination,
            cancelled,
        }
    }

    /// Runs the download to a settled state, reporting as it goes.
    pub async fn carry(self) {
        Partial::sweep(&self.dir, Partial::STALE);

        let partial = Partial::new(&self.dir, &self.asset);
        let mut retry = Retry::new();
        // Whether this machine has ever answered with a head for this asset.
        //
        // A transfer that was serving and then stopped is an interruption, and
        // interruptions are worth asking about for minutes. One that never got
        // a head is a machine that is not there or an asset that is gone.
        let mut served = partial.bytes() > 0;

        loop {
            if self.stopped() {
                self.say(DownloadState::Cancelled, partial.bytes(), 0, None, None);

                break;
            }

            match self.once(&partial, &mut served).await {
                Landed::Saved(where_to) => {
                    log::info!(
                        "{} arrived whole and was saved to {}",
                        self.name,
                        where_to
                    );

                    self.say(
                        DownloadState::Done,
                        partial.bytes(),
                        partial.bytes(),
                        Some(where_to),
                        None,
                    );

                    partial.discard();

                    break;
                }
                Landed::Stopped => {
                    self.say(DownloadState::Cancelled, partial.bytes(), 0, None, None);

                    break;
                }
                // The file is whole and the place it was going is not usable.
                // Discarding it here would throw away a finished transfer over
                // a save location, which is the one mistake this whole feature
                // exists to stop.
                Landed::Unplaceable(reason) => {
                    self.say(
                        DownloadState::Failed,
                        partial.bytes(),
                        partial.bytes(),
                        None,
                        Some(reason),
                    );

                    break;
                }
                Landed::Damaged(reason) => {
                    // Removed rather than reported and left. A file that fails
                    // its digest is not a shorter version of what was asked
                    // for; it is bytes nobody should open, and leaving them
                    // means the next attempt resumes on top of them.
                    partial.discard();

                    self.say(DownloadState::Failed, 0, 0, None, Some(reason));

                    break;
                }
                Landed::Interrupted(reason) => {
                    // A dial refused because this app is locked is not the
                    // machine failing. It is the one failure certain to clear
                    // itself the moment somebody picks the phone up, so it
                    // waits without spending an attempt.
                    let locked = !self.unlocked();

                    let wait = match retry.after(served, locked) {
                        Next::Wait(wait) => wait,
                        Next::GiveUp => {
                            self.say(
                                DownloadState::Failed,
                                partial.bytes(),
                                0,
                                None,
                                Some(reason),
                            );

                            break;
                        }
                    };

                    log::info!(
                        "{} paused with {} bytes kept, asking again in {}s: {}",
                        self.name,
                        partial.bytes(),
                        wait.as_secs(),
                        reason
                    );

                    self.say(
                        DownloadState::Paused,
                        partial.bytes(),
                        0,
                        None,
                        Some(reason),
                    );

                    self.rest(wait).await;
                }
            }
        }

        self.app
            .state::<AppState>()
            .downloads()
            .settle(&self.id)
            .await;
    }

    /// One pass at the machine, from the fetch stream to the saved file.
    async fn once(&self, partial: &Partial, served: &mut bool) -> Landed {
        let (head, mut fetch) = match self.open(partial.bytes()).await {
            Ok(opened) => opened,
            Err(reason) => return Landed::Interrupted(reason),
        };

        *served = true;

        // The machine's offset, never the one that was asked for. It is free to
        // start earlier - it may hold a different file under that id now - and
        // keeping bytes it will not vouch for produces a file of the right
        // length with a wrong middle.
        let mut sink = match partial.resume(head.offset) {
            Ok(sink) => sink,
            Err(error) => return Landed::Damaged(error.to_string()),
        };

        self.say(DownloadState::Running, sink.bytes(), head.len, None, None);

        let mut last = Instant::now();

        loop {
            if self.stopped() {
                // Dropping the fetch resets the stream, which is how the machine
                // is told to stop reading a file nobody is waiting for. The
                // sink's bytes stay on disk.
                return Landed::Stopped;
            }

            let chunk = match fetch.next().await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(error) => return Landed::Interrupted(error.to_string()),
            };

            // Off the runtime's threads: this both writes to disk and hashes,
            // and a synchronous pass over 64 KiB on a runtime thread stalls
            // every other task sharing it.
            let written = tokio::task::spawn_blocking(move || {
                let outcome = sink.write(&chunk);

                (outcome, sink)
            })
            .await;

            let (outcome, back) = match written {
                Ok(pair) => pair,
                Err(error) => return Landed::Interrupted(error.to_string()),
            };

            sink = back;

            if let Err(error) = outcome {
                return Landed::Damaged(error.to_string());
            }

            if last.elapsed() >= Self::REPORT_EVERY {
                self.say(DownloadState::Running, sink.bytes(), head.len, None, None);

                last = Instant::now();
            }
        }

        let arrived = sink.bytes();

        let written = match sink.finish() {
            Ok(written) => written,
            Err(error) => return Landed::Damaged(error.to_string()),
        };

        // Checked over the file rather than over the stream, which is what makes
        // it checkable at all on a resumed transfer: a fetch that started part
        // way through never sees its own first half, and `FetchHead::sha256`
        // covers the whole asset.
        if written != head.sha256 {
            return Landed::Damaged(format!(
                "{} arrived damaged and was not kept: the machine said its contents hash to {} \
                 and what arrived hashes to {}",
                self.name,
                head.sha256.as_str(),
                written.as_str()
            ));
        }

        self.say(DownloadState::Running, arrived, head.len, None, None);

        match self.place(partial).await {
            Ok(where_to) => Landed::Saved(where_to),
            Err(reason) => Landed::Unplaceable(reason),
        }
    }

    /// Opens the fetch stream, re-dialling once before giving up.
    ///
    /// A phone that slept, or a machine that restarted, leaves a connection
    /// that looks alive until an idle timeout expires - so the first attempt
    /// after either spends the whole deadline failing. Re-dialling costs a
    /// handshake; not re-dialling costs the person twenty seconds and an error
    /// for a machine that is answering.
    async fn open(&self, offset: u64) -> Result<(FetchHead, Fetch), String> {
        let spec = FetchSpec {
            asset: self.asset.clone(),
            offset,
        };

        let connection = self.app.state::<AppState>().connect(&self.server).await?;

        match Fetch::open(&connection, spec.clone()).await {
            Ok(opened) => Ok(opened),
            Err(_) => {
                let connection = self.app.state::<AppState>().reconnect(&self.server).await?;

                Fetch::open(&connection, spec)
                    .await
                    .map_err(|error| error.to_string())
            }
        }
    }

    /// Copies the finished partial to where the person asked for it.
    ///
    /// A copy rather than a rename, because the two are not on the same
    /// filesystem and often not the same kind of thing: on Android the
    /// destination is a `content://` URI belonging to another app's provider,
    /// whose write grant is not persistable and whose length cannot be read
    /// back. Downloading straight into it would mean no resumable file at all,
    /// which is the whole feature.
    async fn place(&self, partial: &Partial) -> Result<String, String> {
        let source = partial.path().to_path_buf();
        let destination = self.destination.clone();
        let shown = destination.to_string();
        let app = self.app.clone();

        tokio::task::spawn_blocking(move || {
            let mut from = std::fs::File::open(&source)
                .map_err(|error| format!("could not read the finished download: {error}"))?;

            let mut into = app
                .fs()
                .open(
                    destination,
                    OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .to_owned(),
                )
                .map_err(|error| format!("could not open {shown}: {error}"))?;

            let mut chunk = vec![0u8; Self::COPY_CHUNK];

            loop {
                let read = from
                    .read(&mut chunk)
                    .map_err(|error| format!("could not read the finished download: {error}"))?;

                if read == 0 {
                    break;
                }

                into.write_all(&chunk[..read])
                    .map_err(|error| format!("could not write {shown}: {error}"))?;
            }

            into.flush()
                .map_err(|error| format!("could not write {shown}: {error}"))?;

            Ok(shown)
        })
        .await
        .map_err(|error| error.to_string())?
    }

    fn stopped(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Reports where this download has got to.
    fn say(
        &self,
        state: DownloadState,
        received: u64,
        total: u64,
        saved_to: Option<String>,
        failure: Option<String>,
    ) {
        Downloads::report(
            &self.app,
            &DownloadProgress {
                id: self.id.clone(),
                asset: self.asset.clone(),
                name: self.name.clone(),
                received,
                total,
                state,
                saved_to,
                failure,
            },
        );
    }

    /// Waits out the gap between attempts, or until the app is in front again.
    ///
    /// Coming back is the moment worth acting on. Sleeping the full gap after a
    /// person has returned to the app leaves them looking at a paused row for
    /// up to half a minute with a working radio in their hand.
    async fn rest(&self, wait: Duration) {
        let downloads = self.app.state::<AppState>();
        let woken = downloads.downloads().woken();

        tokio::select! {
            _ = tokio::time::sleep(wait) => {}
            _ = woken.notified() => {}
        }
    }

    /// This app's own launch lock, read the same way every dial reads it.
    fn unlocked(&self) -> bool {
        self.app.state::<AppState>().settings().unlocked()
    }
}

/// How one pass at a download ended.
///
/// The failures are separated because each wants a different thing done with
/// the bytes already on disk. A lost connection leaves a file that is correct
/// as far as it goes, so it is kept and asked about again. A digest that does
/// not match leaves bytes nobody should keep, and asking the same machine again
/// would resume on top of them. A destination that will not take a finished
/// file is not the file's fault at all.
enum Landed {
    Saved(String),
    Stopped,
    Interrupted(String),
    /// Whole, and the destination would not take it.
    Unplaceable(String),
    Damaged(String),
}
