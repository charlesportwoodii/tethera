use super::Fixture;
use tethera_common::structs::transcript::{Answer, Part, Role, ToolStatus};

#[test]
fn a_typed_prompt_and_its_answer_are_one_turn_each() {
    let turns = Fixture::turns("plain.jsonl");

    assert_eq!(turns.len(), 2);
    assert_eq!(turns[0].role, Role::Operator);
    assert_eq!(Fixture::texts(&turns[0]), vec!["rename the widget"]);
    assert_eq!(turns[1].role, Role::Agent);
    assert_eq!(Fixture::texts(&turns[1]), vec!["Renaming it now."]);
}

// Stable across reads is what lets a client dedupe a re-broadcast turn against
// the one it already drew.
#[test]
fn a_turn_takes_its_id_from_the_record_that_opened_it() {
    let turns = Fixture::turns("plain.jsonl");

    assert_eq!(turns[0].id.as_str(), "u1");
    assert_eq!(turns[1].id.as_str(), "a1");
}

#[test]
fn a_tool_call_carries_its_name_input_and_result() {
    let turns = Fixture::turns("tools.jsonl");
    let call = turns
        .iter()
        .flat_map(|turn| &turn.parts)
        .find_map(|part| match part {
            Part::ToolUse {
                name,
                input,
                result,
                status,
                ..
            } if status == &ToolStatus::Ok => Some((name.clone(), input.clone(), result.clone())),
            _ => None,
        })
        .expect("a completed tool call");

    assert_eq!(call.0, "Bash");
    assert!(call.1.contains("echo hi"));
    assert_eq!(call.2.as_deref(), Some("hi"));
}

// Running is a real answer. The last call in a live transcript genuinely has no
// result yet, and drawing it as failed or complete would both be lies.
#[test]
fn a_tool_call_whose_result_has_not_arrived_is_running() {
    let statuses: Vec<ToolStatus> = Fixture::parts("tools.jsonl")
        .iter()
        .filter_map(|part| match part {
            Part::ToolUse { status, .. } => Some(*status),
            _ => None,
        })
        .collect();

    assert_eq!(
        statuses,
        vec![ToolStatus::Ok, ToolStatus::Failed, ToolStatus::Running]
    );
}

#[test]
fn a_file_push_tool_becomes_a_file_part_carrying_its_size_and_type() {
    let file = Fixture::parts("structured.jsonl")
        .into_iter()
        .find_map(|part| match part {
            Part::File {
                name, mime, size, ..
            } => Some((name, mime, size)),
            _ => None,
        })
        .expect("a file part");

    assert_eq!(file.0, "report.md");
    assert_eq!(file.1.as_deref(), Some("text/markdown"));
    assert_eq!(file.2, Some(2048));
}

// The distinction the part set exists to preserve: an agent edits constantly,
// and a card per edit buries the conversation in offers nobody asked for.
#[test]
fn an_edit_becomes_a_diff_and_never_a_file() {
    let parts = Fixture::parts("structured.jsonl");

    let diff = parts
        .iter()
        .find_map(|part| match part {
            Part::Diff {
                path,
                unified,
                added,
                removed,
                ..
            } => Some((path.clone(), unified.clone(), *added, *removed)),
            _ => None,
        })
        .expect("a diff part");

    assert!(diff.0.ends_with("main.rs"));
    assert!(diff.1.contains("@@ -1,2 +1,2 @@"));
    assert!(diff.1.contains("-alpha"));
    assert!(diff.1.contains("+gamma"));
    assert_eq!(diff.2, Some(1));
    assert_eq!(diff.3, Some(1));

    let files: Vec<&Part> = parts
        .iter()
        .filter(|part| matches!(part, Part::File { .. }))
        .collect();

    assert_eq!(files.len(), 1, "only the file-push tool may produce a file part");
}

// A created file has no hunks, so there is nothing to draw as a diff and the
// call renders as the call it was.
#[test]
fn a_write_that_creates_a_file_falls_back_to_a_tool_part() {
    let named: Vec<String> = Fixture::parts("structured.jsonl")
        .iter()
        .filter_map(|part| match part {
            Part::ToolUse { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();

    assert!(named.contains(&"Write".to_string()));
}

#[test]
fn a_question_carries_its_options_and_their_descriptions() {
    let question = Fixture::parts("structured.jsonl")
        .into_iter()
        .find_map(|part| match part {
            Part::Question { question, .. } => Some(question),
            _ => None,
        })
        .expect("a question part");

    let ask = &question.asks[0];

    assert_eq!(ask.prompt, "Which store should the server use?");
    assert_eq!(ask.header.as_deref(), Some("Store"));
    assert_eq!(ask.options.len(), 2);
    assert_eq!(ask.options[0].label, "SQLite");
    assert_eq!(
        ask.options[0].description.as_deref(),
        Some("Relational, migrations, grows well.")
    );
    assert!(ask.allows_free_text);
}

// Both ends compute the fingerprint from identical inputs, which is what makes
// a stale answer detectable rather than answered blind.
#[test]
fn a_questions_fingerprint_is_the_one_common_computes() {
    let question = Fixture::parts("structured.jsonl")
        .into_iter()
        .find_map(|part| match part {
            Part::Question { question, .. } => Some(question),
            _ => None,
        })
        .expect("a question part");

    let expected =
        tethera_common::structs::transcript::Question::fingerprint_of(&question.asks);

    assert_eq!(question.fingerprint, expected);
}

#[test]
fn an_answered_question_records_which_option_was_chosen() {
    let answered: Vec<Answer> = Fixture::parts("structured.jsonl")
        .into_iter()
        .filter_map(|part| match part {
            Part::Question { answered, .. } => answered,
            _ => None,
        })
        .flat_map(|record| record.answers.into_iter().flatten())
        .collect();

    assert_eq!(answered[0], Answer::Choice(0));
    assert_eq!(answered[1], Answer::Multi(vec![0, 1]));
}

// The harness records what was chosen by its label, so a label that is not among
// the options is an "Other" answer rather than a lost one.
#[test]
fn an_answer_that_matches_no_option_is_kept_as_free_text() {
    let answered: Vec<Answer> = Fixture::parts("structured.jsonl")
        .into_iter()
        .filter_map(|part| match part {
            Part::Question { answered, .. } => answered,
            _ => None,
        })
        .flat_map(|record| record.answers.into_iter().flatten())
        .collect();

    assert_eq!(
        answered[2],
        Answer::Text("something else entirely".to_string())
    );
}

// The part set has no image, and a megabyte of base64 on a control frame would
// be refused by the frame cap anyway.
#[test]
fn an_image_becomes_an_unknown_part_without_its_body() {
    let image = Fixture::parts("structured.jsonl")
        .into_iter()
        .find_map(|part| match part {
            Part::Unknown {
                kind,
                fallback_text,
            } if kind == "image" => Some(fallback_text),
            _ => None,
        })
        .expect("an image part");

    assert_eq!(image, "[image/png]");
    assert!(!image.contains("iVBOR"));
}
