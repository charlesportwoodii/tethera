use tethera_common::structs::transcript::{Part, Turn};
use tethera_transport::frame::FrameCodec;

/// How much of a page a control frame will actually carry.
///
/// A page is asked for as a count of turns and delivered as one frame of bytes,
/// and the two are independent: one real turn carries tool inputs, tool results
/// and unified diffs, so a handful of them clear the frame cap while a hundred
/// short ones do not come close. Without a byte bound the count is the only
/// bound, and a page that happens to be large is simply not sendable — which
/// reaches the client as a stream that closed without answering.
///
/// The count stays a ceiling. This is the bound that decides.
pub struct PageBudget;

impl PageBudget {
    /// Room left for everything around the turns: the `Response` and `Payload`
    /// wrappers, `next_before`, `has_earlier`, and the length prefixes postcard
    /// writes for each of them. Generous, because being wrong in this direction
    /// costs one extra page and being wrong in the other costs the whole answer.
    const ENVELOPE: usize = 4 * 1024;

    /// Room kept back for the notice that replaces a part too large to send, so
    /// adding the notice cannot itself push the turn back over.
    const NOTICE: usize = 2 * 1024;

    /// How much of a dropped part's own text the notice carries.
    const KEPT_TEXT: usize = 512;

    /// The most bytes one page's turns may encode to.
    pub const MAX_PAGE_BYTES: usize = FrameCodec::CONTROL_MAX_FRAME_BYTES - Self::ENVELOPE;

    /// What one value costs on the wire, measured with the encoder that will
    /// send it rather than estimated from its text.
    pub fn size_of<T: serde::Serialize>(value: &T) -> usize {
        postcard::to_stdvec(value).map(|bytes| bytes.len()).unwrap_or(0)
    }

    /// The same turn with its largest parts replaced by a notice.
    ///
    /// Dropped rather than truncated in place, because every variant carries its
    /// content differently and a half-decoded diff or tool result is worse than
    /// an honest absence. What is kept is each dropped part's own
    /// `fallback_text`, cut short — that is the text the source wrote, and it is
    /// what a client renders when it cannot render the part.
    pub fn shrink(mut turn: Turn) -> Turn {
        let room = Self::MAX_PAGE_BYTES.saturating_sub(Self::NOTICE);
        let mut dropped: Vec<Part> = Vec::new();

        while Self::size_of(&turn) > room && !turn.parts.is_empty() {
            let largest = turn
                .parts
                .iter()
                .enumerate()
                .max_by_key(|(_, part)| Self::size_of(part))
                .map(|(index, _)| index)
                .unwrap_or(0);

            dropped.push(turn.parts.remove(largest));
        }

        if dropped.is_empty() {
            return turn;
        }

        tracing::warn!(
            turn = turn.id.as_str(),
            parts = dropped.len(),
            limit = Self::MAX_PAGE_BYTES,
            "a turn was too large for one frame; parts of it were replaced by a notice"
        );

        turn.parts.push(Self::notice(&dropped));

        turn
    }

    fn notice(dropped: &[Part]) -> Part {
        let kinds = dropped
            .iter()
            .map(|part| part.kind())
            .collect::<Vec<_>>()
            .join(", ");

        Part::Status {
            label: format!(
                "{} too large to send over this link",
                if dropped.len() == 1 { "a part" } else { "parts" }
            ),
            detail: Some(format!("{kinds}; open the pane at the machine to read them")),
            fallback_text: dropped
                .iter()
                .map(|part| Self::clipped(part.fallback_text()))
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    /// The first `KEPT_TEXT` bytes of `text`, cut at a character boundary.
    ///
    /// Slicing a `String` by a byte count panics in the middle of a multi-byte
    /// character, and a transcript is full of them.
    fn clipped(text: &str) -> String {
        if text.len() <= Self::KEPT_TEXT {
            return text.to_owned();
        }

        let end = text
            .char_indices()
            .map(|(at, _)| at)
            .take_while(|at| *at <= Self::KEPT_TEXT)
            .last()
            .unwrap_or(0);

        format!("{}…", &text[..end])
    }
}
