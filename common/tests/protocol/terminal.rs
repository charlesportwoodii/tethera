use tethera_common::protocol::terminal::{
    attrs, Color, CursorShape, CursorState, Key, Mods, RowUpdate, Span, Style, TerminalFrame,
    TerminalInput,
};

#[test]
fn a_snapshot_round_trips_through_postcard() {
    let frame = TerminalFrame::Snapshot {
        cols: 200,
        rows: 50,
        styles: vec![Style {
            fg: Color::Default,
            bg: Color::Default,
            attrs: attrs::NONE,
        }],
        rows_data: vec![RowUpdate {
            y: 0,
            from_x: 0,
            spans: vec![Span {
                style: 0,
                text: "hello".into(),
            }],
        }],
        cursor: Some(CursorState {
            x: 5,
            y: 0,
            visible: true,
            shape: CursorShape::Block,
        }),
        alt_screen: false,
        scrollback_len: Some(1200),
    };

    let bytes = postcard::to_stdvec(&frame).expect("encode");

    assert_eq!(
        postcard::from_bytes::<TerminalFrame>(&bytes).expect("decode"),
        frame
    );
}

// A pane owning the alternate screen genuinely has no scrollback. Reporting zero
// there is the "absent is not zero" failure the predecessor made.
#[test]
fn an_alternate_screen_pane_reports_no_scrollback_rather_than_zero() {
    let frame = TerminalFrame::Snapshot {
        cols: 80,
        rows: 24,
        styles: Vec::new(),
        rows_data: Vec::new(),
        cursor: None,
        alt_screen: true,
        scrollback_len: None,
    };

    match frame {
        TerminalFrame::Snapshot {
            alt_screen,
            scrollback_len,
            ..
        } => {
            assert!(alt_screen);
            assert!(scrollback_len.is_none());
        }
        _ => panic!("expected a snapshot"),
    }
}

// Input is intent, not bytes. The server encodes, so a CTRL+C overlay bar on a
// phone never needs to know a terminal encoding. There is deliberately no
// raw-bytes variant: it buys nothing and opens an injection surface.
#[test]
fn control_c_is_expressed_as_a_key_and_a_modifier() {
    let input = TerminalInput::Key {
        key: Key::Char('c'),
        mods: Mods::CTRL,
    };

    let bytes = postcard::to_stdvec(&input).expect("encode");

    assert_eq!(
        postcard::from_bytes::<TerminalInput>(&bytes).expect("decode"),
        input
    );
}

#[test]
fn a_modifier_set_reports_the_modifiers_it_holds() {
    let both = Mods(Mods::CTRL.0 | Mods::SHIFT.0);

    assert!(both.contains(Mods::CTRL));
    assert!(both.contains(Mods::SHIFT));
    assert!(!both.contains(Mods::ALT));
}

// Every modifier occupies its own bit, or two different chords would encode the
// same and the server would send the wrong keystroke.
#[test]
fn no_two_modifiers_share_a_bit() {
    assert_eq!(Mods::CTRL.0 & Mods::ALT.0, 0);
    assert_eq!(Mods::CTRL.0 & Mods::SHIFT.0, 0);
    assert_eq!(Mods::ALT.0 & Mods::SHIFT.0, 0);
}

// Same for style attributes: two attributes sharing a bit would make bold and
// dim indistinguishable on the wire.
#[test]
fn no_two_style_attributes_share_a_bit() {
    let all = [
        attrs::BOLD,
        attrs::DIM,
        attrs::ITALIC,
        attrs::UNDERLINE,
        attrs::REVERSE,
        attrs::STRIKE,
        attrs::BLINK,
    ];

    let combined = all.iter().fold(0u8, |acc, bit| acc | bit);

    assert_eq!(combined.count_ones() as usize, all.len());
}

#[test]
fn every_colour_shape_round_trips() {
    for colour in [Color::Default, Color::Indexed(9), Color::Rgb(1, 2, 3)] {
        let bytes = postcard::to_stdvec(&colour).expect("encode");

        assert_eq!(
            postcard::from_bytes::<Color>(&bytes).expect("decode"),
            colour
        );
    }
}

// Text and a key chord are different variants rather than one string, so a
// pasted literal "\x03" can never be mistaken for CTRL+C.
#[test]
fn typed_text_and_a_key_chord_are_different_frames() {
    assert_ne!(
        postcard::to_stdvec(&TerminalInput::Text("c".into())).expect("encode"),
        postcard::to_stdvec(&TerminalInput::Key {
            key: Key::Char('c'),
            mods: Mods::NONE,
        })
        .expect("encode")
    );
}

/// The bit values are a contract with a table the client hand-writes.
///
/// `Mods` crosses to TypeScript as a bare number, so ts-rs carries the type and
/// nothing carries the meaning of the bits. The other half of the contract is
/// `MOD` in `client/src/console/components/KeyBar/KeyBar.types.ts`, and the two
/// disagreeing is silent: a `^C` cap sent as 4 arrives as a modifier the server
/// recognises, applies, and encodes — so the pane receives a real keypress that
/// is the wrong one. That is why this pins the numbers rather than the names.
///
/// The values are xterm's, which `PtyKeys::modifier` already encodes as
/// `1 + shift + 2*alt + 4*ctrl`.
#[test]
fn modifier_bits_match_the_client_table() {
    assert_eq!(Mods::NONE.0, 0);
    assert_eq!(Mods::SHIFT.0, 1);
    assert_eq!(Mods::ALT.0, 2);
    assert_eq!(Mods::CTRL.0, 4);
    assert_eq!(Mods::META.0, 8);
}
