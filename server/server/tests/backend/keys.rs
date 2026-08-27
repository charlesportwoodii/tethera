use tethera_common::protocol::terminal::{Key, Mods};
use tethera_server_lib::backend::herdr::HerdrKeys;

// Every name here was accepted by a real herdr, and the refused ones were
// refused by it. A table written from its help would be a guess: the help lists
// no vocabulary, and an unsupported name fails at the socket rather than at the
// parse — so a wrong name reaches a person as a keystroke that did nothing.
#[test]
fn every_key_a_conversation_needs_has_a_name() {
    for (key, expected) in [
        (Key::Escape, "esc"),
        (Key::Enter, "enter"),
        (Key::Tab, "tab"),
        (Key::Backspace, "backspace"),
        (Key::Up, "up"),
        (Key::Down, "down"),
        (Key::Left, "left"),
        (Key::Right, "right"),
        (Key::Char(' '), "space"),
        (Key::Char('c'), "c"),
        (Key::F(1), "f1"),
    ] {
        assert_eq!(
            HerdrKeys::name(key, Mods::NONE).expect("a name"),
            expected,
            "{key:?}"
        );
    }
}

// `+` and not `-`: measured, herdr refuses `ctrl-c` and accepts `ctrl+c`.
#[test]
fn a_modifier_is_spelled_the_way_the_backend_reads_it() {
    assert_eq!(
        HerdrKeys::name(Key::Char('c'), Mods::CTRL).expect("a name"),
        "ctrl+c"
    );
    assert_eq!(
        HerdrKeys::name(Key::Tab, Mods::SHIFT).expect("a name"),
        "shift+tab"
    );
    assert_eq!(
        HerdrKeys::name(Key::Enter, Mods::ALT).expect("a name"),
        "alt+enter"
    );
}

// The same modifier set has to produce the same string every time, or two
// callers asking for the same key press send two different names.
#[test]
fn several_modifiers_are_spelled_in_one_fixed_order() {
    let both = Mods(Mods::CTRL.0 | Mods::SHIFT.0);

    assert_eq!(
        HerdrKeys::name(Key::Char('c'), both).expect("a name"),
        "ctrl+shift+c"
    );
}

// This backend genuinely cannot express these, and a key silently dropped
// reaches the caller as one that was delivered and did nothing. It costs
// nothing to refuse: herdr publishes no per-pane byte stream, so it never
// advertises `terminal_input` and no full keyboard is ever driven through it.
#[test]
fn a_key_this_backend_cannot_express_is_refused_rather_than_dropped() {
    for key in [
        Key::Delete,
        Key::Insert,
        Key::Home,
        Key::End,
        Key::PageUp,
        Key::PageDown,
    ] {
        assert!(
            HerdrKeys::name(key, Mods::NONE).is_err(),
            "{key:?} must be refused, not sent as something else"
        );
    }
}
