use tethera_common::protocol::request::Request;
use tethera_common::protocol::stream::StreamOpen;
use tethera_transport::error::TransportError;
use tethera_transport::frame::FrameCodec;

#[test]
fn the_codec_rejects_an_oversize_frame_rather_than_truncating_it() {
    let codec = FrameCodec::new(8);
    let oversize = vec![0u8; 64];

    match codec.encode(&oversize) {
        Err(TransportError::FrameTooLarge { limit, .. }) => assert_eq!(limit, 8),
        other => panic!("expected FrameTooLarge, got {other:?}"),
    }
}

#[test]
fn the_codec_rejects_an_oversize_length_header_before_allocating_a_body() {
    let codec = FrameCodec::new(8);

    assert!(codec.decode_length(9u32.to_be_bytes()).is_err());
}

// There is no empty control frame, so zero means a confused or hostile sender
// and is refused alongside an oversized one.
#[test]
fn a_zero_length_header_is_refused() {
    assert!(FrameCodec::default().decode_length([0, 0, 0, 0]).is_err());
}

// The control cap is far above any control frame and far below what is worth
// allocating for a peer that has not yet been authorised. Bulk transfer does not
// use this codec at all.
#[test]
fn the_control_cap_is_sixty_four_kibibytes() {
    assert_eq!(FrameCodec::CONTROL_MAX_FRAME_BYTES, 64 * 1024);
    assert_eq!(
        FrameCodec::DEFAULT_MAX_FRAME_BYTES,
        FrameCodec::CONTROL_MAX_FRAME_BYTES
    );
}

#[test]
fn a_protocol_frame_round_trips_through_its_own_length_header() {
    let codec = FrameCodec::default();
    let open = StreamOpen::Rpc(Request::Describe);

    let wire = codec.encode(&open).expect("encode");
    let header: [u8; 4] = wire[..4].try_into().expect("header");
    let body_len = codec.decode_length(header).expect("length");

    assert_eq!(body_len, wire.len() - 4);
    assert_eq!(
        codec
            .decode_body::<StreamOpen>(&wire[4..])
            .expect("decode"),
        open
    );
}

#[test]
fn a_truncated_body_fails_to_decode_rather_than_yielding_a_partial_frame() {
    let codec = FrameCodec::default();
    let wire = codec.encode(&vec![9u8; 32]).expect("encode");

    assert!(codec
        .decode_body::<Vec<u8>>(&wire[4..wire.len() - 8])
        .is_err());
}

// The header is big-endian, which is the one thing a peer reads before it knows
// anything about the sender. A byte-order change here is a silent wire break.
#[test]
fn the_length_header_is_four_bytes_big_endian() {
    let codec = FrameCodec::default();
    let wire = codec.encode(&vec![7u8; 300]).expect("encode");
    let body_len = wire.len() - 4;

    assert_eq!(wire[..4], (body_len as u32).to_be_bytes());
    assert!(body_len > 255, "a one-byte length would not prove the order");
}
