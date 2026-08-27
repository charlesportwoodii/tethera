use crate::backend::error::BackendError;
use tethera_common::protocol::terminal::{Key, Mods};

/// One key press as herdr spells it.
///
/// herdr takes key *names* rather than bytes — `esc`, `enter`, `ctrl+c` — which
/// is the whole reason the two backends need a table each. The pty backend owns
/// the pty and encodes for the program on the far end; herdr owns neither and
/// takes the name.
///
/// Every name here was probed against a real herdr rather than read from its
/// help, because the help lists no vocabulary and an unsupported name is
/// refused at the socket rather than at the parse.
pub struct HerdrKeys;

impl HerdrKeys {
    /// What herdr accepts, measured. `delete`, `insert`, `home`, `end`,
    /// `pageup` and `pagedown` are each rejected as `unsupported key`.
    ///
    /// That gap is not a problem for what this backend is for. herdr publishes
    /// no per-pane byte stream, so it never advertises `terminal_input` and
    /// nothing drives a full keyboard through it; these keys are what a
    /// conversation needs — interrupt it, move a selection, confirm — and those
    /// are all here.
    pub fn name(key: Key, mods: Mods) -> Result<String, BackendError> {
        let named = match key {
            Key::Escape => "esc".to_string(),
            Key::Enter => "enter".to_string(),
            Key::Tab => "tab".to_string(),
            Key::Backspace => "backspace".to_string(),
            Key::Up => "up".to_string(),
            Key::Down => "down".to_string(),
            Key::Left => "left".to_string(),
            Key::Right => "right".to_string(),
            Key::F(number) => format!("f{number}"),
            Key::Char(' ') => "space".to_string(),
            Key::Char(character) => character.to_string(),
            Key::Delete
            | Key::Insert
            | Key::Home
            | Key::End
            | Key::PageUp
            | Key::PageDown => {
                return Err(BackendError::message(format!(
                    "the terminal backend has no name for {key:?}; it accepts escape, enter, \
                     tab, backspace, the arrows, the function keys and characters"
                )))
            }
        };

        Ok(Self::modified(&named, mods))
    }

    /// `ctrl+c`, `shift+tab`, `alt+enter`. Measured against a real herdr, which
    /// takes `+` and refuses `-`.
    ///
    /// Ordered rather than iterated, so the same modifier set always produces
    /// the same string.
    fn modified(named: &str, mods: Mods) -> String {
        let mut spelled = String::new();

        for (flag, prefix) in [
            (Mods::CTRL, "ctrl+"),
            (Mods::ALT, "alt+"),
            (Mods::SHIFT, "shift+"),
        ] {
            if mods.contains(flag) {
                spelled.push_str(prefix);
            }
        }

        spelled.push_str(named);

        spelled
    }
}
