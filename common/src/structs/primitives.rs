use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Milliseconds since the Unix epoch, UTC.
///
/// The TypeScript override is not cosmetic: ts-rs maps i64 to bigint, and these
/// values reach the client as JSON, where `JSON.parse` never produces a bigint.
/// The binding would describe a runtime value that cannot occur. Epoch millis
/// fit exactly in a double until the year 287396.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct Timestamp(#[ts(type = "number")] pub i64);

macro_rules! opaque_string {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
        #[ts(export, export_to = "./../../client/src/js/bindings/")]
        pub struct $name(pub String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
    };
}

opaque_string!(
    Cursor,
    "A position in a transcript. Opaque to the client, which stores and returns it and never parses it, so the server can change what it encodes without a wire change."
);
opaque_string!(
    Fingerprint,
    "A hash of a question's current state. Echoed back when answering so the server can refuse rather than answer a different question blind."
);
opaque_string!(Sha256, "A SHA-256 digest as lowercase hex.");
