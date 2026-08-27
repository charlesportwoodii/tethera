/// What one harness draws on screen, and which keys drive it.
///
/// A permission prompt is never written to the records — the tool call is, but
/// the request to allow it is drawn, answered and gone. So the only way to know
/// a person is being asked is to read what the agent is showing them, and every
/// value that reading depends on is one harness's chrome rather than a general
/// fact about agents.
///
/// **An agent with no table here cannot be driven, and says so.** That is the
/// honest outcome rather than a limitation to work around: guessing that a
/// second harness marks its current row the same way would answer the wrong
/// option on somebody's behalf and report success. A refusal leaves a person
/// answering at the machine, which is where they already were.
pub struct ScreenChrome {
    /// The glyph the harness marks its current row with.
    ///
    /// It prefixes an echoed message and the input line too, which is why
    /// finding it *below* a list is evidence the list is scrollback rather than
    /// a live picker.
    pub cursor: char,
    /// A line made only of this is the harness's own rule, never content.
    pub rule: char,
    /// Printed by the picker that draws a preview beside its options, and by no
    /// other screen.
    ///
    /// **The two layouts take different keys**, and nothing else on screen
    /// separates them: the numbering, the rule and the row shapes are identical.
    pub preview_hint: &'static str,
    /// Shown when a set of answers is complete and waiting to be sent.
    pub review_marker: &'static str,
    /// The key that sends that review.
    pub submit: char,
}
