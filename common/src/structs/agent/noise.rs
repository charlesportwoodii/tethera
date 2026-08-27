/// What is not a person speaking.
///
/// An agent that records its own injected content under the person's role -
/// skill bodies, task notifications, system reminders - has that content dropped
/// before a turn is built, because rendering machine chatter attributes it to
/// the person.
///
/// A table rather than a branch, so a second agent's equivalent is rows.
pub struct NoiseFilter {
    /// Open and close markers. Text is noise only when it is *wholly* enclosed
    /// by a pair.
    pub wrappers: &'static [(&'static str, &'static str)],
    /// Text starting with one of these is noise.
    pub prefixes: &'static [&'static str],
}

impl NoiseFilter {
    pub const EMPTY: NoiseFilter = NoiseFilter {
        wrappers: &[],
        prefixes: &[],
    };

    pub fn is_noise(&self, text: &str) -> bool {
        let trimmed = text.trim();

        if trimmed.is_empty() {
            return true;
        }

        // Wholly enclosed, never merely containing. A person asking what
        // `<system-reminder>` means must keep their message; a message that *is*
        // one must not survive. That difference is what separates a filter from
        // a censor.
        let wrapped = self.wrappers.iter().any(|(open, close)| {
            trimmed.len() >= open.len() + close.len()
                && trimmed.starts_with(open)
                && trimmed.ends_with(close)
        });

        wrapped
            || self
                .prefixes
                .iter()
                .any(|prefix| trimmed.starts_with(prefix))
    }
}
