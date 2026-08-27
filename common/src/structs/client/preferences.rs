use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What this device has been told to do, held on this device only.
///
/// Nothing here reaches a machine. A preference that a server needed to know
/// about would belong on the device record it already keeps, not in a file this
/// phone writes for itself.
///
/// Every field is `serde(default)` and every default is the *permissive* value,
/// because this file is read before any screen exists. A field added in a later
/// build must not make an older file unreadable, and a read that failed would
/// otherwise have to choose between locking somebody out of their own machines
/// and ignoring a lock they asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Preferences {
    /// Whether opening the app has to be authenticated.
    ///
    /// Off by default, and deliberately: a lock that switched itself on during
    /// an update would strand anybody whose sensor does not work, and the thing
    /// they are locked out of is the machine they would fix it from.
    #[serde(default)]
    pub biometric_lock: bool,
}

impl Preferences {
    pub fn new() -> Self {
        Self {
            biometric_lock: false,
        }
    }
}

impl Default for Preferences {
    fn default() -> Self {
        Self::new()
    }
}
