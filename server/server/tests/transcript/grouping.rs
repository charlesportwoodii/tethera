use super::Fixture;
use tethera_common::structs::transcript::Role;

// The harness writes each content block of a model response as its own record -
// measured, 7413 of 7415 assistant records carry exactly one. Rendering them
// separately would show a sentence and its tool call as two turns.
#[test]
fn one_model_response_written_across_records_is_one_turn() {
    let turns = Fixture::turns("grouped.jsonl");
    let first = &turns[0];

    assert_eq!(first.role, Role::Agent);
    assert_eq!(first.id.as_str(), "g1");
    assert_eq!(Fixture::kinds(first), vec!["text", "tool_use"]);
}

// Reasoning has no variant in the closed part set, so it contributes nothing -
// but it must not break the group it sits inside.
#[test]
fn a_reasoning_block_inside_a_group_contributes_nothing_and_breaks_nothing() {
    let first = &Fixture::turns("grouped.jsonl")[0];

    assert!(!Fixture::kinds(first).contains(&"unknown"));
    assert_eq!(Fixture::texts(first), vec!["First I will look."]);
}

// Measured: 107 cases on one machine where a request id appears, is followed by
// other ids, and appears again. Grouping by the id alone would merge two
// responses that are several tool calls apart.
#[test]
fn a_request_id_that_recurs_after_other_records_starts_a_new_turn() {
    let turns = Fixture::turns("grouped.jsonl");
    let recurred = turns
        .iter()
        .find(|turn| turn.id.as_str() == "g5")
        .expect("the recurring request id opens its own turn");

    assert_eq!(
        Fixture::texts(recurred),
        vec!["Same id, later, and a separate response."]
    );
}

#[test]
fn a_record_with_no_request_id_is_a_turn_of_its_own() {
    let turns = Fixture::turns("grouped.jsonl");
    let alone = turns
        .iter()
        .find(|turn| turn.id.as_str() == "g6")
        .expect("a record with no request id");

    assert_eq!(Fixture::texts(alone), vec!["No request id at all."]);
}

// A tool result is not a turn, and it closes the group that asked for it.
#[test]
fn a_tool_result_record_is_not_a_turn() {
    let turns = Fixture::turns("grouped.jsonl");

    assert!(turns.iter().all(|turn| turn.id.as_str() != "g4"));
    assert_eq!(turns.len(), 3);
}
