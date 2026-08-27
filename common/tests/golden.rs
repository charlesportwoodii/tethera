use std::path::PathBuf;

use tethera_common::protocol::WireVersion;

/// Four bytes big-endian, then the postcard body.
///
/// Spelled out rather than imported: `tethera-common` must not depend on
/// `tethera-transport`, because the layering runs the other way. It is the same
/// shape `FrameCodec` writes, and `transport`'s own tests pin that agreement.
fn frame<T: serde::Serialize>(value: &T) -> Vec<u8> {
    let body = postcard::to_stdvec(value).expect("encode");
    let mut out = Vec::with_capacity(4 + body.len());

    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);

    out
}

fn fixture_path(version: WireVersion, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(format!("v{}", version.0))
        .join(format!("{name}.bin"))
}

/// Assert a value still encodes to the bytes it encoded to when the fixture was
/// blessed.
///
/// Under a positional encoding a reordered field is a silent wire break that
/// surfaces as garbage on a phone you cannot easily attach a debugger to. Set
/// `TETHERA_BLESS=1` to rewrite the fixtures, and only when the wire change is
/// deliberate and the version was bumped for it: a fixture that changes without
/// a version bump is exactly the failure this exists to catch.
pub fn assert_golden<T: serde::Serialize>(version: WireVersion, name: &str, value: &T) {
    let encoded = frame(value);
    let path = fixture_path(version, name);

    if std::env::var("TETHERA_BLESS").as_deref() == Ok("1") {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("create fixture dir");
        std::fs::write(&path, &encoded).expect("write fixture");

        return;
    }

    let expected = std::fs::read(&path).unwrap_or_else(|_| {
        panic!(
            "no fixture at {}. Run `mise run bless` to create it, then read the diff before keeping it.",
            path.display()
        )
    });

    assert_eq!(
        encoded, expected,
        "the encoding of `{name}` changed without a wire version bump. Under postcard \
         that is a silent break for every client already shipped."
    );
}
