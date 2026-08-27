mod attempt;

pub use attempt::Attempt;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tauri_plugin_fs::FilePath;
use tethera_client_core::transfer::Partial;
use tethera_common::structs::client::{DownloadProgress, DownloadState};
use tethera_common::structs::ids::{AssetId, ServerId};
use tokio::sync::{Mutex, Notify};

/// Every download this app is carrying, and the flags that stop them.
///
/// A transfer runs in a task of its own rather than inside the command that
/// asked for it. That is the difference between a download and a screen: the
/// command answers an id the moment the file has somewhere to go, and the bytes
/// keep moving whether or not anybody is looking at the row. A transfer awaited
/// inside a command is a transfer that ends when the screen does.
pub struct Downloads {
    /// Where part-finished files wait between attempts.
    dir: PathBuf,
    /// One flag per live download, keyed by its id. Cleared when it settles.
    live: Mutex<HashMap<String, Arc<AtomicBool>>>,
    minted: AtomicU64,
    /// Signalled when this app comes back to the front.
    ///
    /// A paused download is asleep on a timer, and the moment worth acting on
    /// is a person returning to the app with a working radio in their hand -
    /// not the moment the timer happens to expire.
    wake: Arc<Notify>,
}

impl Downloads {
    /// The event channel every download reports on.
    pub const CHANNEL: &'static str = "download-progress";

    /// The directory partials live in, under the app's own data directory.
    pub const FOLDER: &'static str = "downloads";

    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir: dir.join(Self::FOLDER),
            live: Mutex::new(HashMap::new()),
            minted: AtomicU64::new(0),
            wake: Arc::new(Notify::new()),
        }
    }

    /// Starts a download and answers the id that names it.
    ///
    /// Returns as soon as the task is spawned. Everything a screen needs after
    /// this arrives on `CHANNEL`, starting with an `Opening` row emitted before
    /// this function returns - so the row is drawn before the machine has even
    /// been asked. That stretch is not short: the machine hashes the whole asset
    /// before it writes the head, which on a four hundred megabyte file is most
    /// of a second of apparent silence, and silence is what a person reads as an
    /// app that ignored the tap.
    pub async fn start(
        &self,
        app: AppHandle,
        server: ServerId,
        asset: AssetId,
        name: String,
        destination: FilePath,
    ) -> String {
        let id = format!("dl_{}", self.minted.fetch_add(1, Ordering::Relaxed));
        let cancelled = Arc::new(AtomicBool::new(false));

        self.live
            .lock()
            .await
            .insert(id.clone(), Arc::clone(&cancelled));

        Self::report(
            &app,
            &DownloadProgress {
                id: id.clone(),
                asset: asset.clone(),
                name: name.clone(),
                // Whatever a previous attempt left. A row that opens at zero
                // and jumps to a hundred and thirty megabytes a moment later
                // has told the person their download restarted.
                received: Partial::new(&self.dir, &asset).bytes(),
                total: 0,
                state: DownloadState::Opening,
                saved_to: None,
                failure: None,
            },
        );

        let attempt = Attempt::new(
            app,
            self.dir.clone(),
            id.clone(),
            server,
            asset,
            name,
            destination,
            cancelled,
        );

        tauri::async_runtime::spawn(attempt.carry());

        id
    }

    /// Stops a download, leaving what already arrived where it is.
    ///
    /// Kept rather than deleted, and deliberately: somebody who stops a transfer
    /// on a train is not saying "throw away a hundred and thirty megabytes",
    /// they are saying "not now". Asking for the same file again resumes from
    /// here, and `Partial::sweep` bounds how long an abandoned one lingers.
    ///
    /// Silent when the id names nothing. A download that already settled is not
    /// an error to cancel; it is a person tapping a button on a stale row.
    pub async fn cancel(&self, id: &str) {
        if let Some(flag) = self.live.lock().await.get(id) {
            flag.store(true, Ordering::SeqCst);
        }
    }

    /// Tells every paused download to try again now.
    ///
    /// Called when this app returns to the foreground. Waking a download that
    /// is already running does nothing: only a task sitting out a gap between
    /// attempts is listening.
    pub fn wake(&self) {
        self.wake.notify_waiters();
    }

    pub(crate) fn woken(&self) -> Arc<Notify> {
        Arc::clone(&self.wake)
    }

    /// Forgets a download that has settled.
    pub(crate) async fn settle(&self, id: &str) {
        self.live.lock().await.remove(id);
    }

    /// Emits a progress row, ignoring a webview that is not listening.
    ///
    /// A screen that has moved on is not a reason to stop a transfer that is
    /// working, which is the whole point of the transfer not living on that
    /// screen.
    pub(crate) fn report(app: &AppHandle, progress: &DownloadProgress) {
        let _ = app.emit(Self::CHANNEL, progress);
    }
}
