use tethera_transport::error::TransportError;
use tethera_transport::frame::{Frame, FrameCodec};

#[test]
fn the_codec_rejects_an_oversize_frame_rather_than_truncating_it() {
    let codec = FrameCodec::new(8);
    let frame = Frame::Placeholder {
        payload: vec![0u8; 64],
    };

    match codec.encode(&frame) {
        Err(TransportError::FrameTooLarge { limit, .. }) => assert_eq!(limit, 8),
        other => panic!("expected FrameTooLarge, got {other:?}"),
    }
}

#[test]
fn the_codec_rejects_an_oversize_length_header_before_allocating_a_body() {
    let codec = FrameCodec::new(8);
    let header = 9u32.to_be_bytes();

    assert!(codec.decode_length(header).is_err());
}

#[test]
fn a_frame_within_the_limit_round_trips_through_its_own_length_header() {
    let codec = FrameCodec::new(FrameCodec::DEFAULT_MAX_FRAME_BYTES);
    let frame = Frame::Placeholder {
        payload: vec![1, 2, 3, 4],
    };

    let wire = codec.encode(&frame).expect("encode");
    let header: [u8; 4] = wire[..4].try_into().expect("header");
    let body_len = codec.decode_length(header).expect("length");

    assert_eq!(body_len, wire.len() - 4);

    let decoded = codec.decode_body(&wire[4..]).expect("decode");
    let Frame::Placeholder { payload } = decoded;

    assert_eq!(payload, vec![1, 2, 3, 4]);
}

#[test]
fn a_truncated_body_fails_to_decode_rather_than_yielding_a_partial_frame() {
    let codec = FrameCodec::new(FrameCodec::DEFAULT_MAX_FRAME_BYTES);
    let wire = codec
        .encode(&Frame::Placeholder {
            payload: vec![9; 32],
        })
        .expect("encode");

    assert!(codec.decode_body(&wire[4..wire.len() - 8]).is_err());
}
