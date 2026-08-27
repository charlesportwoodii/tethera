use tethera_common::structs::agent::{Agent, CommandTags};
use tethera_common::traits::AgentTrait;
use tethera_server_lib::transcript::SlashCommand;

/// The tags come off the harness, never out of this reader.
fn tags() -> &'static CommandTags {
    Agent::Claude
        .command_tags()
        .expect("this harness has been measured")
}

/// The shape the harness actually writes, taken off a real session file.
fn recorded(name: &str, args: &str) -> String {
    format!(
        "<command-name>{name}</command-name>\n            <command-message>{}</command-message>\n            <command-args>{args}</command-args>",
        name.trim_start_matches('/')
    )
}

// **The whole point.** The arguments are what says what the command is to do,
// and they were dropped: the span treated as noise ran from `<command-name>` to
// `</command-args>`, so `/goal ship it` reached a phone as `/goal`.
#[test]
fn a_command_keeps_the_arguments_it_was_given() {
    let spoken = SlashCommand::spoken(tags(), &recorded("/goal", "ship it")).expect("a command");

    assert_eq!(spoken, "/goal ship it");
}

#[test]
fn a_command_with_no_arguments_is_just_itself() {
    let spoken = SlashCommand::spoken(tags(), &recorded("/compact", "")).expect("a command");

    assert_eq!(spoken, "/compact");
}

#[test]
fn ordinary_words_are_not_a_command() {
    assert!(SlashCommand::spoken(tags(), "just something somebody typed").is_none());
    assert!(!SlashCommand::is_command(tags(), "just something somebody typed"));
}

// A person's own words that happen to mention the tag are not a command. The
// closing half has to be there, in order, or there is nothing to read between.
#[test]
fn an_unclosed_tag_is_not_a_command() {
    assert!(SlashCommand::spoken(tags(), "I typed <command-name>/goal and it broke").is_none());
}

// Reading between them backwards would slice from a high index to a low one,
// which panics rather than returning nothing.
#[test]
fn tags_in_the_wrong_order_are_not_a_command() {
    let text = "the format is </command-name> after <command-name>, obviously";

    assert!(SlashCommand::spoken(tags(), text).is_none());
}

#[test]
fn command_output_is_read_out_of_its_wrapper() {
    let text = "<local-command-stdout>Compacted 4 turns</local-command-stdout>";

    assert_eq!(
        SlashCommand::output(tags(), text).expect("output"),
        "Compacted 4 turns"
    );
    assert!(SlashCommand::is_command(tags(), text));
}

// A command that printed nothing has nothing to open, and a fold that opens on
// nothing advertises detail it does not have.
#[test]
fn empty_output_is_absent_rather_than_an_empty_fold() {
    assert!(SlashCommand::output(tags(), "<local-command-stdout>   </local-command-stdout>").is_none());
}
