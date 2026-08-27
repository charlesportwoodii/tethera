use tethera_common::structs::agent::CommandTags;

/// A slash command a person ran, read back off the record.
///
/// The harness does not record `/goal ship it` as those words. It records the
/// name, the message and the arguments as separate tags, and its output
/// separately again — so a reader that dropped the whole span took the
/// arguments with the name, and `/goal ship it` arrived as `/goal`.
///
/// **Which tags those are is the harness's, not this reader's.** They arrive as
/// a `CommandTags` off `AgentTrait`, beside the noise filter, so a second
/// harness is a table rather than a branch here — and one nobody has measured
/// has no table at all rather than a borrowed one.
pub struct SlashCommand;

impl SlashCommand {
    /// The command as the person typed it, arguments included.
    pub fn spoken(tags: &CommandTags, text: &str) -> Option<String> {
        let name = CommandTags::between(text, tags.name)?.trim().to_string();

        if name.is_empty() {
            return None;
        }

        let args = CommandTags::between(text, tags.args).unwrap_or("").trim();

        if args.is_empty() {
            return Some(name);
        }

        Some(format!("{name} {args}"))
    }

    /// What the command printed.
    ///
    /// Empty output is `None` rather than an empty fold: a command that printed
    /// nothing has nothing to open, and a fold that opens on nothing advertises
    /// detail it does not have.
    pub fn output(tags: &CommandTags, text: &str) -> Option<String> {
        let printed = CommandTags::between(text, tags.stdout)?.trim();

        if printed.is_empty() {
            return None;
        }

        Some(printed.to_string())
    }

    /// Whether a record is one of these at all.
    ///
    /// Checked before the noise filter, because the filter's job is to drop
    /// what the harness wrote under the person's role and this is the one shape
    /// there that a person actually did.
    pub fn is_command(tags: &CommandTags, text: &str) -> bool {
        CommandTags::between(text, tags.name).is_some()
            || CommandTags::between(text, tags.stdout).is_some()
    }
}
