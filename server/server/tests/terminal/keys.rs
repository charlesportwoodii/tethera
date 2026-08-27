use tethera_common::protocol::terminal::{Key, Mods, TerminalInput};
use tethera_server_lib::terminal::KeyEncoder;

fn key(key: Key, mods: Mods) -> Vec<u8> {
    KeyEncoder::key(key, mods, false)
}

// The overlay bar on a phone sends this and never needs to know a terminal
// encoding. It is the single most important byte this table produces.
#[test]
fn control_c_encodes_to_the_interrupt_byte() {
    assert_eq!(key(Key::Char('c'), Mods::CTRL), vec![0x03]);
}

#[test]
fn control_letters_map_onto_the_low_control_range() {
    assert_eq!(key(Key::Char('a'), Mods::CTRL), vec![0x01]);
    assert_eq!(key(Key::Char('d'), Mods::CTRL), vec![0x04]);
    assert_eq!(key(Key::Char('z'), Mods::CTRL), vec![0x1a]);
    assert_eq!(key(Key::Char('C'), Mods::CTRL), vec![0x03]);
    assert_eq!(key(Key::Char('['), Mods::CTRL), vec![0x1b]);
    assert_eq!(key(Key::Char('\\'), Mods::CTRL), vec![0x1c]);
    assert_eq!(key(Key::Char('?'), Mods::CTRL), vec![0x7f]);
    assert_eq!(key(Key::Char(' '), Mods::CTRL), vec![0x00]);
}

#[test]
fn a_plain_character_encodes_as_its_own_utf8() {
    assert_eq!(key(Key::Char('q'), Mods::NONE), b"q".to_vec());
    assert_eq!(
        key(Key::Char('\u{e9}'), Mods::NONE),
        "\u{e9}".as_bytes().to_vec()
    );
}

#[test]
fn alt_prefixes_exactly_one_escape() {
    assert_eq!(key(Key::Char('b'), Mods::ALT), vec![0x1b, b'b']);
    assert_eq!(key(Key::Escape, Mods::ALT), vec![0x1b, 0x1b]);
    assert_eq!(key(Key::Enter, Mods::ALT), vec![0x1b, 0x0d]);
}

// A key with a parameterised form already carries ALT in its modifier parameter.
// A leading escape on top of that reaches the program as a separate Escape press,
// which opens vi-mode in a shell and aborts a readline prefix.
#[test]
fn alt_does_not_also_prefix_a_key_that_carries_it_in_a_parameter() {
    assert_eq!(key(Key::Up, Mods::ALT), b"\x1b[1;3A".to_vec());
    assert_eq!(key(Key::Delete, Mods::ALT), b"\x1b[3;3~".to_vec());
    assert_eq!(key(Key::F(1), Mods::ALT), b"\x1b[1;3P".to_vec());
    assert_eq!(key(Key::F(5), Mods::ALT), b"\x1b[15;3~".to_vec());
    assert_eq!(
        key(Key::Home, Mods(Mods::ALT.0 | Mods::CTRL.0)),
        b"\x1b[1;7H".to_vec()
    );
}

#[test]
fn the_named_keys_encode_to_their_xterm_sequences() {
    assert_eq!(key(Key::Enter, Mods::NONE), vec![0x0d]);
    assert_eq!(key(Key::Escape, Mods::NONE), vec![0x1b]);
    assert_eq!(key(Key::Tab, Mods::NONE), vec![0x09]);
    assert_eq!(key(Key::Tab, Mods::SHIFT), b"\x1b[Z".to_vec());
    assert_eq!(key(Key::Backspace, Mods::NONE), vec![0x7f]);
    assert_eq!(key(Key::Backspace, Mods::CTRL), vec![0x08]);
    assert_eq!(key(Key::Delete, Mods::NONE), b"\x1b[3~".to_vec());
    assert_eq!(key(Key::Insert, Mods::NONE), b"\x1b[2~".to_vec());
    assert_eq!(key(Key::PageUp, Mods::NONE), b"\x1b[5~".to_vec());
    assert_eq!(key(Key::PageDown, Mods::NONE), b"\x1b[6~".to_vec());
    assert_eq!(key(Key::Home, Mods::NONE), b"\x1b[H".to_vec());
    assert_eq!(key(Key::End, Mods::NONE), b"\x1b[F".to_vec());
    assert_eq!(key(Key::Up, Mods::NONE), b"\x1b[A".to_vec());
    assert_eq!(key(Key::Down, Mods::NONE), b"\x1b[B".to_vec());
    assert_eq!(key(Key::Right, Mods::NONE), b"\x1b[C".to_vec());
    assert_eq!(key(Key::Left, Mods::NONE), b"\x1b[D".to_vec());
    assert_eq!(key(Key::F(1), Mods::NONE), b"\x1bOP".to_vec());
    assert_eq!(key(Key::F(4), Mods::NONE), b"\x1bOS".to_vec());
    assert_eq!(key(Key::F(5), Mods::NONE), b"\x1b[15~".to_vec());
    assert_eq!(key(Key::F(12), Mods::NONE), b"\x1b[24~".to_vec());
}

// An application that sets DECCKM expects SS3 for the cursor keys, and
// full-screen programs set it themselves on the stream the emulator parses.
#[test]
fn application_cursor_key_mode_switches_the_arrows_to_ss3() {
    assert_eq!(KeyEncoder::key(Key::Up, Mods::NONE, true), b"\x1bOA".to_vec());
    assert_eq!(
        KeyEncoder::key(Key::Home, Mods::NONE, true),
        b"\x1bOH".to_vec()
    );
}

#[test]
fn a_modified_arrow_takes_the_parameterised_form() {
    assert_eq!(key(Key::Up, Mods::CTRL), b"\x1b[1;5A".to_vec());
    assert_eq!(key(Key::Right, Mods::SHIFT), b"\x1b[1;2C".to_vec());
    assert_eq!(key(Key::Delete, Mods::CTRL), b"\x1b[3;5~".to_vec());
    assert_eq!(key(Key::F(1), Mods::CTRL), b"\x1b[1;5P".to_vec());
}

// A modified cursor key never takes the SS3 form, whatever DECCKM says.
#[test]
fn a_modified_arrow_ignores_application_cursor_key_mode() {
    assert_eq!(
        KeyEncoder::key(Key::Up, Mods::CTRL, true),
        b"\x1b[1;5A".to_vec()
    );
}

// A guess would send a real sequence for a different key, which is worse than
// sending none.
#[test]
fn an_unknown_function_key_encodes_to_nothing() {
    assert!(key(Key::F(0), Mods::NONE).is_empty());
    assert!(key(Key::F(64), Mods::NONE).is_empty());
    assert!(key(Key::F(0), Mods::ALT).is_empty());
}

// `TerminalInput` has no raw-bytes variant because it opens an injection
// surface. Text carrying an escape sequence reopens exactly that: the bytes
// reach the pty and the program interprets them.
#[test]
fn text_cannot_carry_an_escape_sequence_to_the_pty() {
    // The introducer and the terminator are gone, so what reaches the pty is
    // inert text rather than a sequence. What remains of the body does not
    // matter: without an ESC nothing can parse it as anything but characters.
    let sanitised = KeyEncoder::text("\x1b]2;evil\x07ok", false);

    assert!(!sanitised.contains(&0x1b), "an escape reached the pty");
    assert!(!sanitised.contains(&0x07), "a bell reached the pty");
    assert!(sanitised.ends_with(b"ok"));

    assert_eq!(KeyEncoder::text("a\x00b\x1bc", false), b"abc".to_vec());
    assert_eq!(KeyEncoder::text("\x03", false), Vec::<u8>::new());
}

#[test]
fn text_keeps_tabs_and_normalises_newlines_to_carriage_returns() {
    assert_eq!(
        KeyEncoder::text("a\tb\nc\r\nd", false),
        b"a\tb\rc\rd".to_vec()
    );
}

// Multi-line text is a paste by construction, and bracketing it is what stops a
// pasted script executing line by line. Single-line text is indistinguishable
// from typing, so bracketing it would change what a shell does with it.
#[test]
fn multi_line_text_is_bracketed_when_the_pane_asked_for_it() {
    assert_eq!(
        KeyEncoder::text("one\ntwo", true),
        b"\x1b[200~one\rtwo\x1b[201~".to_vec()
    );
    assert_eq!(KeyEncoder::text("one", true), b"one".to_vec());
    assert_eq!(KeyEncoder::text("one\ntwo", false), b"one\rtwo".to_vec());
}

#[test]
fn encode_dispatches_both_input_shapes() {
    let text = TerminalInput::Text("hi".into());
    let control = TerminalInput::Key {
        key: Key::Char('c'),
        mods: Mods::CTRL,
    };

    assert_eq!(KeyEncoder::encode(&text, false, false), b"hi".to_vec());
    assert_eq!(KeyEncoder::encode(&control, false, false), vec![0x03]);
}
