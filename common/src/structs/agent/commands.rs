/// How one harness records a command a person ran.
///
/// A slash command is not written as the words the person typed. Claude Code
/// writes the name, the message and the arguments as three separate tags, on a
/// record of its own kind — and its output separately again. None of that is a
/// general fact about agents; it is one harness's vocabulary, so it lives on a
/// table beside `NoiseFilter` rather than in the reader.
///
/// An agent nobody has measured has no table and no guess is made for it. The
/// alternative is borrowing this one, which would read a second harness's
/// records through the first harness's grammar and quietly mis-attribute
/// whatever happened to match.
pub struct CommandTags {
    /// Wraps the command itself — `/goal`.
    pub name: (&'static str, &'static str),
    /// Wraps what followed it. **A separate tag, which is the whole reason this
    /// exists**: a reader that treated the span from the name to the end of the
    /// arguments as one thing to drop took the arguments with it, and
    /// `/goal ship it` arrived as `/goal`.
    pub args: (&'static str, &'static str),
    /// Wraps what the command printed.
    pub stdout: (&'static str, &'static str),
    /// The record `type` the invocation is written under, which is not the
    /// person's role even though a person is what ran it.
    pub record_kind: &'static str,
    /// The `subtype` that distinguishes it from every other record of that kind.
    pub record_subtype: &'static str,
}

impl CommandTags {
    /// The text between a matched pair, if both are present and in order.
    ///
    /// Ordered rather than merely present: a person writing `</command-args>`
    /// before `<command-args>` in a sentence about the format has not run a
    /// command, and reading between them backwards would panic on the slice.
    pub fn between<'a>(text: &'a str, (open, close): (&str, &str)) -> Option<&'a str> {
        let start = text.find(open)? + open.len();
        let end = text[start..].find(close)? + start;

        Some(&text[start..end])
    }
}
