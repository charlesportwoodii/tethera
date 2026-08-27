use std::path::{Path, PathBuf};
use tethera_common::structs::agent::{
    Agent, AgentProfile, AgentSpawn, ClaudeAgent, TranscriptSource,
};
use tethera_common::traits::AgentTrait;

#[test]
fn claude_launch_command_is_bare_when_no_first_prompt_is_given() {
    let spawn = AgentSpawn::new(Agent::Claude, "/home/charl/projects".to_string(), None);

    assert_eq!(Agent::Claude.launch_command(&spawn), vec!["claude".to_string()]);
}

#[test]
fn claude_launch_command_appends_the_first_prompt_as_one_argument() {
    let spawn = AgentSpawn::new(
        Agent::Claude,
        "/home/charl/projects".to_string(),
        Some("investigate the flaky test".to_string()),
    );

    assert_eq!(
        Agent::Claude.launch_command(&spawn),
        vec![
            "claude".to_string(),
            "investigate the flaky test".to_string()
        ]
    );
}

#[test]
fn resume_command_carries_the_session_id() {
    assert_eq!(
        Agent::Claude.resume_command("abc123"),
        vec![
            "claude".to_string(),
            "--resume".to_string(),
            "abc123".to_string()
        ]
    );
}

#[test]
fn each_agent_dispatches_to_its_own_binary() {
    let spawn = AgentSpawn::new(Agent::Codex, "/tmp".to_string(), None);

    assert_eq!(Agent::Codex.launch_command(&spawn), vec!["codex".to_string()]);
}

#[test]
fn a_profile_round_trips_through_postcard() {
    let profile = Agent::Claude.profile();
    let bytes = postcard::to_stdvec(&profile).expect("encode");

    assert_eq!(
        postcard::from_bytes::<AgentProfile>(&bytes).expect("decode"),
        profile
    );
}

// A profile that promised a transcript it cannot produce would put an empty
// conversation in front of a person instead of the terminal that does work.
#[test]
fn a_profile_says_whether_its_records_can_be_read() {
    assert!(Agent::Claude.profile().provides_transcript);
    assert!(!Agent::Codex.profile().provides_transcript);
}

#[test]
fn the_catalog_is_every_agent_this_build_accepts() {
    let catalog: Vec<AgentProfile> = Agent::ALL.iter().map(Agent::profile).collect();

    assert_eq!(catalog.len(), 2);
    assert!(catalog.iter().any(|p| p.id.as_str() == "claude"));
    assert!(catalog.iter().any(|p| p.id.as_str() == "codex"));
}

// Two profiles sharing an id would make one of them unstartable: the client
// hands an id back and the server would resolve it to whichever came first.
#[test]
fn no_two_profiles_share_an_id() {
    let ids: std::collections::BTreeSet<String> = Agent::ALL
        .iter()
        .map(|agent| agent.profile().id.as_str().to_string())
        .collect();

    assert_eq!(ids.len(), Agent::ALL.len());
}

// Measured on a real machine: every character outside [A-Za-z0-9] becomes the
// same dash, which is why the mapping cannot be run backwards.
#[test]
fn the_project_directory_collapses_a_colon_a_separator_a_dot_and_a_space_to_one_dash() {
    assert_eq!(
        ClaudeAgent::project_directory(r"C:\Users\charl\projects\tethera"),
        "C--Users-charl-projects-tethera"
    );
    assert_eq!(
        ClaudeAgent::project_directory(r"C:\Users\charl\bin\Amulet-Map-Editor-0.10.48"),
        "C--Users-charl-bin-Amulet-Map-Editor-0-10-48"
    );
    assert_eq!(
        ClaudeAgent::project_directory(r"C:\Users\charl\notebook\Books\Terah Lai Shorehn"),
        "C--Users-charl-notebook-Books-Terah-Lai-Shorehn"
    );
}

#[test]
fn the_project_directory_of_a_unix_path_leads_with_a_dash() {
    assert_eq!(
        ClaudeAgent::project_directory("/home/charl/projects/bvc"),
        "-home-charl-projects-bvc"
    );
}

#[test]
fn a_transcript_source_names_the_session_file_under_the_flattened_project() {
    let source = Agent::Claude.transcript_source(
        Path::new("/home/charl"),
        "/home/charl/projects/tethera",
        "090ae836-f18e-43ae-8ec5-6874b7513357",
    );

    assert_eq!(
        source.path().map(PathBuf::from),
        Some(PathBuf::from(
            "/home/charl/.claude/projects/-home-charl-projects-tethera/090ae836-f18e-43ae-8ec5-6874b7513357.jsonl"
        ))
    );
}

// Absent is a real answer. A pane whose agent has no readable records has no
// conversation surface at all, and the client offers its terminal.
#[test]
fn an_agent_whose_records_nobody_has_measured_reports_no_source() {
    let source = Agent::Codex.transcript_source(Path::new("/home/charl"), "/tmp", "session");

    assert_eq!(source, TranscriptSource::Absent);
    assert!(!source.is_readable());
}

#[test]
fn text_wholly_wrapped_in_a_system_reminder_is_noise() {
    let filter = Agent::Claude.noise_filter();

    assert!(filter.is_noise("<system-reminder>the file changed on disk</system-reminder>"));
    assert!(filter.is_noise("\n  <task-notification>done</task-notification>  \n"));
}

// The difference between a filter and a censor. A person asking about an
// injected shape must keep their message.
#[test]
fn a_message_that_merely_mentions_a_system_reminder_survives() {
    let filter = Agent::Claude.noise_filter();

    assert!(!filter.is_noise(
        "what does <system-reminder> mean when it shows up in the middle of a turn?"
    ));
    assert!(!filter.is_noise("</system-reminder> is the closing half"));
}

#[test]
fn the_local_command_caveat_is_noise() {
    let filter = Agent::Claude.noise_filter();

    assert!(filter.is_noise(
        "Caveat: The messages below were generated by the user while running local commands. DO NOT respond to these."
    ));
    assert!(filter.is_noise("Another Claude session sent a message:\nStatus: DONE"));
}

// **A command a person ran is not noise.** It was, and the span dropped ran from
// `<command-name>` to `</command-args>` — so the command took its own arguments
// with it and `/goal ship it` arrived as `/goal`. The tags are read rather than
// dropped now, in the one place that knows how, so the filter must leave them
// alone for that reader to get them.
#[test]
fn a_slash_command_is_not_noise() {
    let filter = Agent::Claude.noise_filter();

    assert!(!filter.is_noise(
        "<command-name>/goal</command-name>\n<command-args>ship it</command-args>"
    ));
    assert!(!filter.is_noise("<local-command-stdout>ok</local-command-stdout>"));
}

// Nobody has measured Codex, so its filter drops nothing. An empty table is
// honest; a borrowed one would drop a person's words on a guess.
#[test]
fn an_unmeasured_agent_treats_nothing_as_noise() {
    assert!(!Agent::Codex.noise_filter().is_noise("<system-reminder>x</system-reminder>"));
}

// **The same rule for every table on the trait, and the reason they are tables.**
// A harness nobody has measured has no command grammar and no screen chrome, so
// its records are not read as commands and its screens are not driven. Borrowing
// the measured harness's answers would be worse than having none: reading a
// second harness's records through the first one's grammar mis-attributes
// whatever happens to match, and driving its picker on a guess answers the wrong
// option on somebody's behalf and reports success.
#[test]
fn an_unmeasured_agent_has_no_command_grammar_and_no_screen_chrome() {
    assert!(Agent::Codex.command_tags().is_none());
    assert!(Agent::Codex.screen_chrome().is_none());
}

// And the measured one carries both, so nothing reaches for a table that is not
// there on the harness this build actually reads.
#[test]
fn a_measured_agent_carries_the_tables_that_drive_it() {
    let tags = Agent::Claude.command_tags().expect("command tags");
    let chrome = Agent::Claude.screen_chrome().expect("screen chrome");

    assert_eq!(tags.record_kind, "system");
    assert_eq!(chrome.cursor, '❯');
}

// A `File` part is a deliberate act. An agent edits constantly, and a card per
// edit buries the conversation in offers nobody asked for.
#[test]
fn only_a_file_push_tool_hands_a_file_to_the_person() {
    assert!(Agent::Claude.file_push_tools().contains(&"SendUserFile"));
    assert!(!Agent::Claude.file_push_tools().contains(&"Edit"));
    assert!(!Agent::Claude.file_push_tools().contains(&"Write"));
    assert!(Agent::Claude.diff_tools().contains(&"Edit"));
    assert!(Agent::Claude.question_tools().contains(&"AskUserQuestion"));
}

// Measured shapes that reach the filter, from 224 354 records. Each is content
// the harness wrote under the person's role.
#[test]
fn the_shapes_a_harness_writes_under_the_persons_role_are_noise() {
    let filter = Agent::Claude.noise_filter();

    assert!(filter.is_noise("<bash-input>ls -la</bash-input>"));
    assert!(filter.is_noise("<bash-stdout>total 4</bash-stdout>"));
    assert!(filter.is_noise("[Request interrupted by user]"));
}

// The interrupt is dropped here and re-emitted as a status part by the reader,
// so a person who typed the sentence themselves still loses nothing: their turn
// renders as the interruption it describes.
#[test]
fn a_person_writing_about_an_interrupt_is_not_matched_by_the_interrupt_row() {
    let filter = Agent::Claude.noise_filter();

    assert!(!filter.is_noise(
        "why does [Request interrupted by user] show up twice in the log?"
    ));
}
