use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The frame set's version.
///
/// Separate from the ALPN. `tethera/1` gates the framing and handshake
/// contract, which changes almost never; this negotiates which frames may be
/// sent, which changes with every feature.
///
/// postcard is positional, so an unknown enum discriminant is a decode error
/// with no length to skip past. This number is what makes that survivable: the
/// server serialises to the negotiated version and never emits a variant the
/// client cannot decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct WireVersion(pub u16);

impl WireVersion {
    /// Every version this build speaks, ascending.
    ///
    /// One entry, and not a list ending in the newest. Version one is gone
    /// rather than kept for compatibility because the change that ended it was
    /// two struct fields, and postcard encodes struct fields positionally with
    /// no way to omit one. A frame set can drop an enum variant it must not
    /// send; it cannot un-send a field. Offering version one would mean
    /// promising an encoding this build is incapable of producing.
    ///
    /// Version two ended the same way: `WatchOpen::Machine` gained a `layouts`
    /// field, and a struct variant's fields are as positional as a struct's.
    ///
    /// So an older client is refused by name at the handshake, which is the
    /// whole reason this number exists: the alternative is a phone decoding a
    /// pane into the wrong fields and drawing it.
    pub const SUPPORTED: &'static [WireVersion] = &[WireVersion(3)];

    /// The highest version both sides speak, or `None` when they share none.
    ///
    /// `None` is a refusal, not a fallback.
    pub fn negotiate(local: &[WireVersion], remote: &[WireVersion]) -> Option<WireVersion> {
        local.iter().filter(|v| remote.contains(v)).max().copied()
    }
}
