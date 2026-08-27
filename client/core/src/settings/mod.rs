use crate::error::ClientError;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tethera_common::structs::client::Preferences;

/// This device's own preferences, and whether it is currently unlocked.
///
/// The two live together because they are read together on every call that
/// reaches a machine, and splitting them would let a caller read one without the
/// other - which is how a lock ends up being consulted on some paths and not on
/// others.
///
/// JSON beside the server book, for the same reason: a handful of values that
/// never grow.
pub struct SettingsStore {
    path: PathBuf,
    preferences: Mutex<Preferences>,
    /// Runtime only, never persisted. A file that recorded "unlocked" would
    /// survive a restart and defeat the whole feature on the one occasion it
    /// matters - a phone that was picked up and rebooted.
    unlocked: AtomicBool,
    /// Whether this app put a system screen in front of itself.
    ///
    /// A file picker and a save dialog are separate activities, so opening one
    /// backgrounds this app and the webview reports itself hidden - which is
    /// indistinguishable, from the webview, from somebody putting the phone
    /// down. Locking on that means picking a file asks for a fingerprint on the
    /// way out and again on the way back, for a screen the person never left.
    holding: AtomicBool,
}

impl SettingsStore {
    pub const FILE_NAME: &'static str = "settings.json";

    /// An absent file is the ordinary first launch.
    ///
    /// An unreadable one falls back to the defaults rather than failing, which
    /// is the opposite of `ServerBook`'s choice and deliberately so. The book
    /// holds machines that cannot be reconstructed, so losing it must be loud.
    /// This holds one boolean that a person can set again in a second, and
    /// refusing to start over it would strand somebody behind a corrupt file
    /// with no way in.
    pub fn open(path: PathBuf) -> Self {
        let preferences = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Preferences>(&bytes).ok())
            .unwrap_or_default();

        // Nothing to unlock when no lock was asked for. Starting locked and
        // relying on a screen to unlock would mean a build that failed to draw
        // that screen could reach no machine at all.
        let unlocked = AtomicBool::new(!preferences.biometric_lock);

        Self {
            path,
            preferences: Mutex::new(preferences),
            unlocked,
            holding: AtomicBool::new(false),
        }
    }

    pub fn preferences(&self) -> Preferences {
        *self.preferences.lock().expect("settings")
    }

    /// Whether anything may reach a machine right now.
    ///
    /// `SeqCst` because another task's correctness depends on observing it: the
    /// whole point is that a request raced against a lock does not slip through.
    pub fn unlocked(&self) -> bool {
        self.unlocked.load(Ordering::SeqCst)
    }

    pub fn unlock(&self) {
        self.unlocked.store(true, Ordering::SeqCst);
    }

    /// Closes the door again, which is what returning from the background does.
    ///
    /// A no-op when no lock is set, so a resume cannot strand a person who never
    /// asked to be locked out.
    pub fn lock(&self) {
        // A system screen this app opened is not somebody leaving. The picker
        // is still this app's own task, the person is still holding the phone,
        // and they are two taps into something they started.
        if self.holding.load(Ordering::SeqCst) {
            return;
        }

        if self.preferences().biometric_lock {
            self.unlocked.store(false, Ordering::SeqCst);
        }
    }

    /// Marks a system screen as open, and answers a guard that clears it.
    ///
    /// A guard rather than a pair of calls, because the flag has to clear on
    /// every path out of the dialog - dismissed, failed, or completed - and a
    /// flag left set would disable the lock for the life of the process.
    pub fn holding(&self) -> Holding<'_> {
        self.holding.store(true, Ordering::SeqCst);

        Holding { store: self }
    }

    /// Turns the lock on or off.
    ///
    /// Turning it *on* leaves this session unlocked. The person setting it has
    /// just proved who they are by using the app, and locking them out of the
    /// screen they are standing on teaches them the toggle is dangerous. It
    /// takes effect on the next resume, which is when it matters.
    pub fn set_biometric_lock(&self, on: bool) -> Result<Preferences, ClientError> {
        let updated = {
            let mut held = self.preferences.lock().expect("settings");
            held.biometric_lock = on;

            *held
        };

        if !on {
            self.unlocked.store(true, Ordering::SeqCst);
        }

        self.write(&updated)?;

        Ok(updated)
    }

    fn write(&self, preferences: &Preferences) -> Result<(), ClientError> {
        let bytes = serde_json::to_vec_pretty(preferences).map_err(|error| ClientError::Book {
            path: self.path.display().to_string(),
            reason: error.to_string(),
        })?;

        std::fs::write(&self.path, bytes).map_err(|error| ClientError::Book {
            path: self.path.display().to_string(),
            reason: error.to_string(),
        })
    }
}

/// Held while a system screen this app opened is in front of it.
///
/// Clears on drop, including on an early return or a panic, which is the whole
/// reason it is a guard rather than two calls.
pub struct Holding<'a> {
    store: &'a SettingsStore,
}

impl Drop for Holding<'_> {
    fn drop(&mut self) {
        self.store.holding.store(false, Ordering::SeqCst);
    }
}
