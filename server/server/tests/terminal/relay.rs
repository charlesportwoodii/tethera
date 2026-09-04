use tethera_server_lib::terminal::{Half, ShimRelay};
use tokio::io::BufReader;

async fn hello(line: &str) -> anyhow::Result<(String, u16, u16, Half)> {
    let mut reader = BufReader::new(line.as_bytes());
    let (pane, size, half) = ShimRelay::hello(&mut reader).await?;

    Ok((pane.as_str().to_string(), size.cols, size.rows, half))
}

// The greeting is minted, not parsed. A shim announces herdr's own `w85:p3`, and
// tethera's `PaneId` is that with a prefix - parsing the native form rejects
// every real pane, which reads like a malformed peer rather than a mapping bug.
#[tokio::test]
async fn a_greeting_names_a_pane_a_size_and_a_direction() {
    let (pane, cols, rows, half) = hello("w85:p3 194 46 up\n").await.expect("a greeting");

    assert!(pane.ends_with("w85:p3"), "got {pane}");
    assert_eq!((cols, rows), (194, 46));
    assert_eq!(half, Half::Up);
}

// The direction decides which half of the pane's stream this channel is, and a
// shim opens two. Defaulting to `Up` rather than refusing keeps an older shim
// readable, which is the half that matters.
#[tokio::test]
async fn a_greeting_names_its_direction() {
    let (_, _, _, down) = hello("w85:p3 80 24 down\n").await.expect("a greeting");
    let (_, _, _, up) = hello("w85:p3 80 24\n").await.expect("a greeting");

    assert_eq!(down, Half::Down);
    assert_eq!(up, Half::Up);
}

// A shim that could not read its own terminal still has a usable stream, so a
// missing size is defaulted rather than refused. The real geometry arrives with
// the first resize.
#[tokio::test]
async fn a_greeting_without_a_size_is_defaulted_rather_than_refused() {
    let (_, cols, rows, _) = hello("w85:p3\n").await.expect("a greeting");

    assert_eq!((cols, rows), (80, 24));
}

// An unbounded read on a peer that never sends a newline grows until the process
// dies, and the far end of this channel is another process. `HELLO_LIMIT` caps
// the read, so a flood is truncated at the cap rather than consumed.
#[tokio::test]
async fn a_greeting_with_no_newline_is_bounded_at_the_limit() {
    let flood = "w".repeat(4096);
    let (pane, _, _, _) = hello(&flood).await.expect("a bounded greeting");

    assert!(
        pane.len() < 600,
        "the greeting read past its limit: {} bytes",
        pane.len()
    );
}

// An empty channel is a shim that opened one and died, not a shim with an
// unusual greeting. Refusing by name is what keeps the accept loop's log
// readable.
#[tokio::test]
async fn a_channel_that_says_nothing_is_refused() {
    assert!(hello("").await.is_err());
}
