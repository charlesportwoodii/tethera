use tethera_common::protocol::request::Request;
use tethera_common::protocol::response::{Payload, Progress, ProgressStage, Response};
use tethera_common::protocol::stream::StreamOpen;
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::testing::Loopback;
use tethera_transport::stream::FrameIo;

// Two endpoints on loopback, one bi stream, one frame each way. This is the
// smallest thing that proves the codec and the stream helpers agree: everything
// above it in the protocol is more frames through the same path.
#[tokio::test]
async fn a_frame_written_to_a_stream_reads_back_on_the_other_side() {
    let pair = Loopback::connect().await.expect("pair");
    let codec = FrameCodec::default();
    let sent = StreamOpen::Rpc(Request::Describe);

    let (mut send, _recv) = pair.client.open_bi().await.expect("open");
    FrameIo::write(&mut send, &codec, &sent)
        .await
        .expect("write");
    send.finish().expect("finish");

    let (_send, mut recv) = pair.server.accept_bi().await.expect("accept");
    let received: StreamOpen = FrameIo::read(&mut recv, &codec)
        .await
        .expect("read")
        .expect("a frame");

    assert_eq!(received, sent);
}

// A stream that ends cleanly before a header is not an error: it is a peer that
// finished. Treating it as one makes every orderly close log a failure.
#[tokio::test]
async fn a_stream_that_ends_cleanly_reads_as_no_frame() {
    let pair = Loopback::connect().await.expect("pair");

    let (mut send, _recv) = pair.client.open_bi().await.expect("open");
    send.finish().expect("finish");

    let (_send, mut recv) = pair.server.accept_bi().await.expect("accept");
    let received = FrameIo::read::<StreamOpen>(&mut recv, &FrameCodec::default())
        .await
        .expect("read");

    assert!(received.is_none());
}

// A peer that began a frame and vanished is corruption, not a goodbye. Reporting
// it is what keeps a truncated header from hiding behind the same silence as a
// clean close.
#[tokio::test]
async fn a_partial_header_is_an_error_rather_than_a_clean_close() {
    let pair = Loopback::connect().await.expect("pair");

    let (mut send, _recv) = pair.client.open_bi().await.expect("open");
    send.write_all(&[0x00, 0x00]).await.expect("write");
    send.finish().expect("finish");

    let (_send, mut recv) = pair.server.accept_bi().await.expect("accept");
    let received = FrameIo::read::<StreamOpen>(&mut recv, &FrameCodec::default()).await;

    assert!(received.is_err());
}

// Several frames on one stream, read back in order. The RPC lifecycle is zero or
// more Progress frames then exactly one terminal frame, so the stream has to
// carry a sequence rather than a single message.
#[tokio::test]
async fn many_frames_on_one_stream_arrive_in_order() {
    let pair = Loopback::connect().await.expect("pair");
    let codec = FrameCodec::default();

    let (mut send, _recv) = pair.server.open_bi().await.expect("open");
    for stage in [ProgressStage::Accepted, ProgressStage::StartingAgent] {
        let frame = Response::Progress(Progress {
            stage,
            detail: None,
        });
        FrameIo::write(&mut send, &codec, &frame).await.expect("write");
    }
    FrameIo::write(&mut send, &codec, &Response::Ok(Payload::Ack))
        .await
        .expect("write");
    send.finish().expect("finish");

    let (_send, mut recv) = pair.client.accept_bi().await.expect("accept");
    let mut seen = Vec::new();
    while let Some(frame) = FrameIo::read::<Response>(&mut recv, &codec)
        .await
        .expect("read")
    {
        let terminal = frame.is_terminal();
        seen.push(frame);

        if terminal {
            break;
        }
    }

    assert_eq!(seen.len(), 3);
    assert!(!seen[0].is_terminal());
    assert!(!seen[1].is_terminal());
    assert!(seen[2].is_terminal());
}

// The body of a download is raw bytes to FIN, so nothing in the framing bounds
// it and nothing in the framing notices when it stops. Two attempts at the same
// 403 MiB asset from a phone both ended at byte 136,708,096 - the same byte
// twice, which is a limit somewhere rather than a network event.
//
// Not in the gate: it moves a fifth of a gigabyte, which is worth a minute when
// chasing a truncation and worth nothing on every commit.
#[tokio::test]
#[ignore = "moves 192 MiB; run with --ignored when chasing a truncated transfer"]
async fn a_body_far_larger_than_a_flow_control_window_arrives_whole() {
    const TOTAL: usize = 192 * 1024 * 1024;
    const CHUNK: usize = 64 * 1024;

    let pair = Loopback::connect().await.expect("pair");
    let (mut send, _recv) = pair.server.open_bi().await.expect("open");

    let writer = tokio::spawn(async move {
        let chunk = vec![0xABu8; CHUNK];
        let mut written = 0usize;

        while written < TOTAL {
            send.write_all(&chunk).await.expect("write");
            written += CHUNK;
        }

        send.finish().ok();

        written
    });

    let (_send, mut recv) = pair.client.accept_bi().await.expect("accept");
    let mut chunk = vec![0u8; CHUNK];
    let mut read = 0usize;

    while read < TOTAL {
        recv.read_exact(&mut chunk)
            .await
            .unwrap_or_else(|error| panic!("the stream stopped {read} bytes in: {error}"));

        read += CHUNK;
    }

    assert_eq!(read, writer.await.expect("writer"));
}

// A hostile length header must be refused before a body is read, and on a real
// stream that means the reader errors rather than waiting for bytes that will
// never come.
#[tokio::test]
async fn an_oversize_length_header_is_refused_on_a_live_stream() {
    let pair = Loopback::connect().await.expect("pair");

    let (mut send, _recv) = pair.client.open_bi().await.expect("open");
    send.write_all(&u32::MAX.to_be_bytes()).await.expect("write");
    send.finish().expect("finish");

    let (_send, mut recv) = pair.server.accept_bi().await.expect("accept");
    let received = FrameIo::read::<StreamOpen>(&mut recv, &FrameCodec::default()).await;

    assert!(received.is_err());
}
