use tethera_common::protocol::terminal::{Key, Mods, TerminalInput};

/// `TerminalInput` to pty bytes.
///
/// xterm's table rather than the pane's terminfo, and not as a guess: the pty
/// backend sets `TERM=xterm-256color` on the pane itself, so xterm is what the
/// program in there was told to expect. A terminfo lookup would add a dependency
/// and a per-pane read to rediscover a value this server chose.
///
/// The one mode that genuinely changes the encoding is DECCKM, and applications
/// set it on the stream the emulator is already parsing, so it is read from there
/// rather than from a database.
pub struct KeyEncoder;

impl KeyEncoder {
    pub fn encode(
        input: &TerminalInput,
        application_cursor_keys: bool,
        bracketed_paste: bool,
    ) -> Vec<u8> {
        match input {
            TerminalInput::Text(text) => Self::text(text, bracketed_paste),
            TerminalInput::Key { key, mods } => Self::key(*key, *mods, application_cursor_keys),
        }
    }

    pub fn key(key: Key, mods: Mods, application_cursor_keys: bool) -> Vec<u8> {
        let mut out = match key {
            Key::Char(ch) => Self::character(ch, mods),
            Key::Enter => vec![0x0d],
            Key::Escape => vec![0x1b],
            Key::Tab if mods.contains(Mods::SHIFT) => b"\x1b[Z".to_vec(),
            Key::Tab => vec![0x09],
            Key::Backspace if mods.contains(Mods::CTRL) => vec![0x08],
            Key::Backspace => vec![0x7f],
            Key::Insert => Self::tilde(2, mods),
            Key::Delete => Self::tilde(3, mods),
            Key::PageUp => Self::tilde(5, mods),
            Key::PageDown => Self::tilde(6, mods),
            Key::Home => Self::cursor('H', mods, application_cursor_keys),
            Key::End => Self::cursor('F', mods, application_cursor_keys),
            Key::Up => Self::cursor('A', mods, application_cursor_keys),
            Key::Down => Self::cursor('B', mods, application_cursor_keys),
            Key::Right => Self::cursor('C', mods, application_cursor_keys),
            Key::Left => Self::cursor('D', mods, application_cursor_keys),
            Key::F(number) => Self::function(number, mods),
        };

        // Only the keys with no parameterised form take the escape prefix. The
        // cursor, tilde and function forms already carry ALT inside their
        // modifier parameter, and a leading escape on top of that reaches the
        // program as a *separate* Escape key press - which opens vi-mode in a
        // shell and aborts a readline prefix. A character carries its own prefix,
        // because there the escape has to precede a control byte.
        let prefixes_escape = matches!(
            key,
            Key::Enter | Key::Escape | Key::Tab | Key::Backspace
        );

        if mods.contains(Mods::ALT) && prefixes_escape && !out.is_empty() {
            out.insert(0, 0x1b);
        }

        out
    }

    pub fn text(text: &str, bracketed_paste: bool) -> Vec<u8> {
        let multi_line = text.contains('\n') || text.contains('\r');
        let mut body = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }

                    body.push('\r');
                }
                '\n' => body.push('\r'),
                '\t' => body.push('\t'),
                // Every other control character is dropped. `TerminalInput` has
                // no raw-bytes variant precisely so a client cannot inject an
                // escape sequence into a pane, and text carrying one would
                // reopen exactly that: the bytes reach the pty and the program
                // interprets them.
                ch if ch.is_control() => {}
                ch => body.push(ch),
            }
        }

        // Multi-line text is a paste by construction, and bracketing it is what
        // stops a pasted script executing line by line. Single-line text is
        // indistinguishable from typing, so bracketing it would change what a
        // shell does with it.
        if bracketed_paste && multi_line {
            return format!("\x1b[200~{body}\x1b[201~").into_bytes();
        }

        body.into_bytes()
    }

    fn character(ch: char, mods: Mods) -> Vec<u8> {
        let mut out = if mods.contains(Mods::CTRL) {
            match ch {
                'a'..='z' => vec![(ch as u8) - b'a' + 1],
                'A'..='Z' => vec![(ch as u8) - b'A' + 1],
                '@' | ' ' => vec![0x00],
                '[' => vec![0x1b],
                '\\' => vec![0x1c],
                ']' => vec![0x1d],
                '^' => vec![0x1e],
                '_' => vec![0x1f],
                '?' => vec![0x7f],
                other => other.to_string().into_bytes(),
            }
        } else {
            ch.to_string().into_bytes()
        };

        if mods.contains(Mods::ALT) {
            out.insert(0, 0x1b);
        }

        out
    }

    /// xterm's modifier parameter: `1 + shift + 2*alt + 4*ctrl`.
    ///
    /// One means no modifier, which is why the parameterised forms below are used
    /// only above one.
    fn modifier(mods: Mods) -> u8 {
        let mut value = 1;

        if mods.contains(Mods::SHIFT) {
            value += 1;
        }

        if mods.contains(Mods::ALT) {
            value += 2;
        }

        if mods.contains(Mods::CTRL) {
            value += 4;
        }

        value
    }

    fn cursor(final_byte: char, mods: Mods, application: bool) -> Vec<u8> {
        let modifier = Self::modifier(mods);

        // A modified cursor key never takes the SS3 form, whatever DECCKM says.
        if modifier > 1 {
            return format!("\x1b[1;{modifier}{final_byte}").into_bytes();
        }

        if application {
            return format!("\x1bO{final_byte}").into_bytes();
        }

        format!("\x1b[{final_byte}").into_bytes()
    }

    fn tilde(number: u8, mods: Mods) -> Vec<u8> {
        let modifier = Self::modifier(mods);

        if modifier > 1 {
            return format!("\x1b[{number};{modifier}~").into_bytes();
        }

        format!("\x1b[{number}~").into_bytes()
    }

    fn function(number: u8, mods: Mods) -> Vec<u8> {
        let modifier = Self::modifier(mods);

        // F1 to F4 are SS3 keys unmodified and parameterised CSI keys modified,
        // which is why they are not in the tilde table.
        if (1..=4).contains(&number) {
            let final_byte = [b'P', b'Q', b'R', b'S'][usize::from(number) - 1] as char;

            if modifier > 1 {
                return format!("\x1b[1;{modifier}{final_byte}").into_bytes();
            }

            return format!("\x1bO{final_byte}").into_bytes();
        }

        let parameter = match number {
            5 => 15,
            6 => 17,
            7 => 18,
            8 => 19,
            9 => 20,
            10 => 21,
            11 => 23,
            12 => 24,
            13 => 25,
            14 => 26,
            15 => 28,
            16 => 29,
            17 => 31,
            18 => 32,
            19 => 33,
            20 => 34,
            // An unknown function key encodes to nothing. A guess would send a
            // real sequence for a different key, which is worse than sending
            // none.
            _ => return Vec::new(),
        };

        Self::tilde(parameter, mods)
    }
}
