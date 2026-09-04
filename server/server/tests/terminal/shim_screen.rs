use tethera_common::structs::terminal::Size;
use tethera_server_lib::terminal::ShimScreen;

fn screen() -> ShimScreen {
    ShimScreen::new(Size { cols: 80, rows: 24 })
}

// A ConPTY will not start its child until something says where the cursor is,
// and no reply comes back through two nested ConPTYs. The shim owns this pty, so
// the shim answers - and it answers with the position it actually tracks rather
// than a constant, because a query later in a session would otherwise be told
// the cursor is at the origin while a program is drawing at row twenty.
#[test]
fn a_cursor_query_is_answered_from_the_tracked_position() {
    let mut screen = screen();

    assert_eq!(screen.observe(b"hello\r\nworld"), Vec::<u8>::new());

    let reply = screen.observe(b"\x1b[6n");

    // Row 2, column 6: one-based, after "world".
    assert_eq!(reply, b"\x1b[2;6R".to_vec());
}

#[test]
fn a_primary_device_attribute_query_is_answered() {
    let mut screen = screen();
    let reply = screen.observe(b"\x1b[c");

    assert!(
        reply.starts_with(b"\x1b[?"),
        "expected a DA1 response, got {reply:?}"
    );
}

// Output that asks nothing produces nothing. A shim that replied to ordinary
// text would type into the shell.
#[test]
fn ordinary_output_produces_no_reply() {
    let mut screen = screen();

    assert_eq!(screen.observe(b"PS C:\\> ls\r\n"), Vec::<u8>::new());
}

// XTWINOPS resize requests belong to the pty the shim owns. Forwarded, they
// reach the console the shim is a guest in and resize it - measured: a claim of
// 58x30 left the pane's console at 58x30 after the shim exited, while herdr went
// on reporting the pane as 66x46.
#[test]
fn a_window_resize_request_is_not_forwarded_to_the_pane() {
    let mut screen = screen();
    let forwarded = screen.forward(b"before\x1b[8;30;58tafter");

    assert_eq!(forwarded, b"beforeafter".to_vec());
}

// Everything else passes through byte for byte. The desk is a display somebody
// may be looking at, and a filter that dropped a colour or a cursor move would
// corrupt it.
#[test]
fn ordinary_output_passes_through_untouched() {
    let mut screen = screen();
    let chunk = b"\x1b[32mgreen\x1b[m\r\n\x1b[2;6H\x1b[?25l";

    assert_eq!(screen.forward(chunk), chunk.to_vec());
}

// A sequence split across two reads must still be caught. The pipe decides
// where a chunk ends, so a filter that only matched within one chunk would leak
// whenever the split landed mid-sequence.
#[test]
fn a_resize_request_split_across_chunks_is_still_removed() {
    let mut screen = screen();

    let first = screen.forward(b"aa\x1b[8;30");
    let second = screen.forward(b";58tbb");

    let mut joined = first;
    joined.extend_from_slice(&second);

    assert_eq!(joined, b"aabb".to_vec());
}
