use tethera_server_lib::terminal::{Downlink, Uplink};

// The length is validated before the body is read, as `FrameCodec` does: the far
// end is another process, and a confused header must not make this one allocate
// for it.
#[test]
fn an_oversize_payload_length_is_refused() {
    let mut header = [0u8; Downlink::HEADER_BYTES];
    header[0] = Downlink::INPUT;
    header[1..].copy_from_slice(&(Downlink::MAX_PAYLOAD as u32 + 1).to_be_bytes());

    assert_eq!(Downlink::payload_length(header), None);

    let mut header = [0u8; Uplink::HEADER_BYTES];
    header[0] = Uplink::OUTPUT;
    header[1..].copy_from_slice(&(Uplink::MAX_PAYLOAD as u32 + 1).to_be_bytes());

    assert_eq!(Uplink::payload_length(header), None);
}

// A tag from a newer build is skipped, not fatal. A shim that outlived an
// upgrade should ignore what it does not know rather than tear down a pane
// somebody is working in.
#[test]
fn an_unknown_tag_decodes_to_nothing_rather_than_failing() {
    assert_eq!(Downlink::decode(200, b"anything"), None);
    assert_eq!(Uplink::decode(200, b"anything"), None);
}

// A resize is four bytes of geometry and must survive the round trip exactly:
// the pane's whole layout is computed from it.
#[test]
fn a_resize_survives_its_own_encoding_in_both_directions() {
    let down = Downlink::Resize { cols: 58, rows: 30 };
    let encoded = down.encode();

    assert_eq!(
        Downlink::decode(encoded[0], &encoded[Downlink::HEADER_BYTES..]),
        Some(down)
    );

    let up = Uplink::Resized {
        cols: 194,
        rows: 46,
    };
    let encoded = up.encode();

    assert_eq!(
        Uplink::decode(encoded[0], &encoded[Uplink::HEADER_BYTES..]),
        Some(up)
    );
}

// The header names the body's length, so a reader knows how much to take before
// it takes any of it.
#[test]
fn a_header_names_the_length_of_the_body_that_follows() {
    let message = Uplink::Output(b"twelve bytes".to_vec());
    let encoded = message.encode();

    let mut header = [0u8; Uplink::HEADER_BYTES];
    header.copy_from_slice(&encoded[..Uplink::HEADER_BYTES]);

    assert_eq!(Uplink::payload_length(header), Some(12));
    assert_eq!(encoded.len(), Uplink::HEADER_BYTES + 12);
}
