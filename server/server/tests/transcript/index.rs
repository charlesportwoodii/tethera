use super::Fixture;
use tethera_common::structs::transcript::{Part, Role};

// Every shape in the measured table, in one file. What survives is the point:
// the filter drops what the harness wrote under the person's role and nothing
// else.
#[test]
fn every_shape_the_harness_writes_under_the_persons_role_is_dropped() {
    let turns = Fixture::turns("noise.jsonl");
    let surviving: Vec<&str> = turns.iter().map(|turn| turn.id.as_str()).collect();

    for dropped in [
        "n1", "n2", "n3", "n4", "n5", "n6", "n7", "n8", "n9", "n10", "n11",
    ] {
        assert!(
            !surviving.contains(&dropped),
            "{dropped} reached the conversation"
        );
    }
}

// The difference between a filter and a censor, and the reason wrappers are
// matched whole rather than anywhere.
#[test]
fn a_person_asking_about_an_injected_shape_keeps_their_message() {
    let turns = Fixture::turns("noise.jsonl");
    let kept = turns
        .iter()
        .find(|turn| turn.id.as_str() == "n12")
        .expect("the person's own question survives");

    assert_eq!(kept.role, Role::Operator);
    assert!(Fixture::texts(kept)[0].contains("<system-reminder>"));
}

// The filter is scoped to the person's role. The harness never injects under its
// own, so filtering agent text would drop words the agent actually said.
#[test]
fn an_agent_explaining_an_injected_shape_keeps_its_words() {
    let turns = Fixture::turns("noise.jsonl");
    let kept = turns
        .iter()
        .find(|turn| turn.id.as_str() == "n13")
        .expect("the agent's own sentence survives");

    assert_eq!(kept.role, Role::Agent);
    assert!(Fixture::texts(kept)[0].contains("<system-reminder>"));
}

// A subagent's conversation is its own. Threading it in would interleave two
// dialogues that never addressed each other.
#[test]
fn a_sidechain_record_does_not_reach_the_main_conversation() {
    let turns = Fixture::turns("noise.jsonl");

    assert!(turns.iter().all(|turn| turn.id.as_str() != "n11"));
}

// Measured: 179 interrupts carry a field naming the message they stopped, and 46
// carry only the sentence. Detecting one spelling finds four fifths of them.
#[test]
fn an_interrupt_is_a_status_part_whichever_way_the_harness_recorded_it() {
    let turns = Fixture::turns("noise.jsonl");

    for id in ["n14", "n15"] {
        let interrupt = turns
            .iter()
            .find(|turn| turn.id.as_str() == id)
            .unwrap_or_else(|| panic!("{id} should be a turn"));

        assert_eq!(interrupt.role, Role::Operator);
        assert!(matches!(
            interrupt.parts.as_slice(),
            [Part::Status { label, .. }] if label == "Interrupted"
        ));
    }
}

// The one place the history before a point is discontinuous with the history
// after it. Rendering the seam is cheaper than leaving a person to infer it.
#[test]
fn a_compaction_is_a_visible_seam_and_not_the_persons_words() {
    let turns = Fixture::turns("noise.jsonl");
    let seam = turns
        .iter()
        .find(|turn| turn.id.as_str() == "n16")
        .expect("the compaction seam is a turn");

    assert_eq!(seam.role, Role::Agent);
    assert!(matches!(
        seam.parts.as_slice(),
        [Part::Status { label, .. }] if label == "Context compacted"
    ));
}

// A file being appended to while it is read is the ordinary steady state, not a
// corruption. One measured line in 224 354 was mid-write.
#[test]
fn a_half_written_final_line_does_not_fail_the_page() {
    let turns = Fixture::turns("truncated.jsonl");

    assert_eq!(turns.len(), 2);
    assert_eq!(turns[0].id.as_str(), "x1");
    assert_eq!(turns[1].id.as_str(), "x2");
}

#[test]
fn a_line_that_is_not_json_is_skipped_rather_than_fatal() {
    let mut reader = Fixture::reader("truncated.jsonl");
    let page = reader.page(None, 100).expect("a page despite the bad line");

    assert_eq!(page.items.len(), 2);
    assert_eq!(reader.skipped(), 1);
}

// A message typed while the agent was still working reaches the file as an
// attachment, not a user record. Measured across 224 files: 393 of them, and 392
// appear nowhere else - so dropping the attachment type wholesale loses the
// person's own words.
#[test]
fn a_message_typed_while_the_agent_worked_is_still_the_person() {
    let turns = Fixture::turns("queued.jsonl");
    let spoken: Vec<String> = turns
        .iter()
        .flat_map(Fixture::texts)
        .collect();

    assert!(spoken.contains(&"stop and show me the diff first".to_string()));
    assert!(spoken.contains(&"a queued prompt written as blocks".to_string()));
    assert!(turns.iter().all(|turn| turn.role == Role::Operator));
}

// The person is not the only thing that fills the queue. An absent origin is not
// a claim of humanity either: older records carry none, and reading them as the
// person would put a peer agent's message under the operator's name.
#[test]
fn a_queued_message_nobody_typed_is_not_the_person() {
    let turns = Fixture::turns("queued.jsonl");
    let ids: Vec<&str> = turns.iter().map(|turn| turn.id.as_str()).collect();

    for machine in ["q2", "q3", "q4", "q7"] {
        assert!(!ids.contains(&machine), "{machine} was read as the person");
    }
}

#[test]
fn a_queued_message_still_passes_through_the_noise_filter() {
    let ids: Vec<String> = Fixture::turns("queued.jsonl")
        .iter()
        .map(|turn| turn.id.as_str().to_string())
        .collect();

    assert!(!ids.contains(&"q5".to_string()));
}
