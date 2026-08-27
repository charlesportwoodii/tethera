use crate::structs::agent::{
    AgentCapabilities, AgentProfile, AgentSpawn, CommandTags, NoiseFilter, ScreenChrome,
    TranscriptSource,
};
use std::path::Path;

/// One agent harness, as behaviour.
///
/// Every difference between two agents is a table on this trait, never a branch
/// at a call site: adding an agent is an implementation and a catalog row.
pub trait AgentTrait {
    /// The command this agent is, as a bare name.
    ///
    /// Named on the trait rather than read out of a launch line, because the
    /// question a machine asks of it — is this harness installed here? — has
    /// nothing to do with what any one spawn would run.
    fn binary(&self) -> &'static str;

    fn launch_command(&self, spawn: &AgentSpawn) -> Vec<String>;

    fn resume_command(&self, session_id: &str) -> Vec<String>;

    fn capabilities(&self) -> AgentCapabilities;

    /// This agent as a catalog row. The wire never carries the enum.
    fn profile(&self) -> AgentProfile;

    /// Where this agent's own records for one session live.
    ///
    /// `home` is passed rather than resolved because this crate reads no
    /// filesystem and holds no environment.
    fn transcript_source(&self, home: &Path, cwd: &str, session: &str) -> TranscriptSource;

    /// What this agent records under the person's role that the person did not
    /// say.
    fn noise_filter(&self) -> &'static NoiseFilter;

    /// How this harness records a command a person ran, if anybody has measured
    /// it. `None` leaves such a record to the noise filter, which is what a
    /// reader that knows nothing about a harness's command grammar should do.
    fn command_tags(&self) -> Option<&'static CommandTags>;

    /// What this harness draws on screen, if anybody has measured it. `None`
    /// means its screens are not read and its pickers are not driven — see
    /// `ScreenChrome`.
    fn screen_chrome(&self) -> Option<&'static ScreenChrome>;

    /// Tool names that hand a file to the person deliberately.
    ///
    /// A `Part::File` comes from one of these and from nothing else. An agent
    /// edits constantly, and a card per edit buries the conversation in offers
    /// nobody asked for.
    fn file_push_tools(&self) -> &'static [&'static str];

    /// Tool names that put a choice to the person.
    fn question_tools(&self) -> &'static [&'static str];

    /// Tool names whose result carries a patch worth rendering as a diff.
    fn diff_tools(&self) -> &'static [&'static str];

    /// Tool names that carry a task list.
    fn todo_tools(&self) -> &'static [&'static str];
}
