/// The last part of a path, read the way the machine that wrote it spelled it.
///
/// `Path::file_name` and `Path::file_stem` split on the *host's* separator. A
/// `\`-separated path read on Linux therefore has no components at all and comes
/// back whole, so a card meant to show `notes.md` shows the entire
/// `C:\Users\...\notes.md` instead, and a test written on Windows passes on
/// Windows only.
///
/// Every string these take is a path some *other* process wrote down: an agent's
/// own session records, herdr's report of a pane's directory, a peer's upload
/// spec. The separator in it is a property of that writer and never of the
/// reader, so nothing here may consult the host.
///
/// `Catalog::session_of` deliberately does not use this. It is handed paths from
/// the server's own directory walk, which are host-native by construction.
pub struct PathName;

impl PathName {
    /// The final component, ignoring any trailing separators.
    pub fn basename(path: &str) -> &str {
        let trimmed = path.trim_end_matches(['/', '\\']);

        match trimmed.rfind(['/', '\\']) {
            Some(cut) => &trimmed[cut + 1..],
            None => trimmed,
        }
    }

    /// The final component with its last extension removed.
    ///
    /// A leading dot is kept whole, so `.bashrc` stems to `.bashrc` rather than
    /// to nothing. That is what `Path::file_stem` does, and a name that vanished
    /// would be reported as an absent value by every caller here.
    pub fn stem(path: &str) -> &str {
        let name = Self::basename(path);

        match name.rfind('.') {
            Some(cut) if cut > 0 => &name[..cut],
            _ => name,
        }
    }
}
