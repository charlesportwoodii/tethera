use tethera_common::protocol::WireVersion;
use tethera_common::structs::ids::TurnId;
use tethera_common::structs::primitives::{Cursor, Timestamp};
use tethera_common::structs::transcript::{Part, Role, TodoItem, TodoStatus, ToolStatus, Turn};

fn a_turn(parts: Vec<Part>) -> Turn {
    Turn {
        cursor: Cursor("t1:8814".into()),
        id: TurnId("rec-9f21".into()),
        at: Timestamp(1_766_000_000_000),
        role: Role::Agent,
        parts,
    }
}

#[test]
fn a_turn_round_trips_through_postcard() {
    let turn = a_turn(vec![Part::Text {
        text: "hello".into(),
    }]);
    let bytes = postcard::to_stdvec(&turn).expect("encode");

    assert_eq!(postcard::from_bytes::<Turn>(&bytes).expect("decode"), turn);
}

// The client orders turns by their arrival order in a page, dedupes by TurnId,
// and resumes from a Cursor. It never needs a sequence number, so the protocol
// does not carry one.
#[test]
fn a_turn_carries_a_cursor_rather_than_a_sequence_number() {
    assert_eq!(a_turn(Vec::new()).cursor.as_str(), "t1:8814");
}

// The transcript is a timeline: every turn shows a time in the gutter and your
// own turns are marked differently from the agent's. Both were absent from the
// scaffolding's TranscriptEntry and the design could not draw without them.
#[test]
fn a_turn_states_who_spoke_and_when() {
    let turn = a_turn(Vec::new());

    assert_eq!(turn.role, Role::Agent);
    assert_eq!(turn.at, Timestamp(1_766_000_000_000));
}

#[test]
fn an_unknown_part_falls_back_to_its_source_rows_verbatim() {
    let part = Part::Unknown {
        kind: "some_future_type".into(),
        fallback_text: "  raw   source rows  ".into(),
    };

    assert_eq!(part.fallback_text(), "  raw   source rows  ");
}

#[test]
fn a_text_part_falls_back_to_its_own_text() {
    assert_eq!(
        Part::Text {
            text: "hello".into()
        }
        .fallback_text(),
        "hello"
    );
}

// A tool row renders as three chunks - name, invocation, result - which is what
// the design draws as `Bash / grep -rn tethera / 2 hits`.
#[test]
fn a_tool_use_part_carries_its_name_input_and_result() {
    let part = Part::ToolUse {
        name: "Bash".into(),
        input: "grep -rn tethera".into(),
        result: Some("2 hits".into()),
        status: ToolStatus::Ok,
        fallback_text: "ran Bash".into(),
    };

    assert_eq!(part.fallback_text(), "ran Bash");
}

#[test]
fn every_structured_part_carries_its_source_rows() {
    let parts = vec![
        Part::Diff {
            path: "src/lib.rs".into(),
            unified: "@@ -1 +1 @@".into(),
            added: Some(1),
            removed: Some(1),
            fallback_text: "DIFF src/lib.rs".into(),
        },
        Part::Todo {
            items: vec![TodoItem {
                text: "write the test".into(),
                status: TodoStatus::Done,
            }],
            fallback_text: "TODO write the test".into(),
        },
        Part::Table {
            columns: vec!["a".into()],
            rows: vec![vec!["1".into()]],
            fallback_text: "a 1".into(),
        },
        Part::Status {
            label: "compiling".into(),
            detail: None,
            fallback_text: "compiling".into(),
        },
    ];

    assert!(parts.iter().all(|part| !part.fallback_text().is_empty()));
}

// The mechanism that makes a positional encoding survive an app store. Zero is
// a version older than every part, so every part must degrade - and it degrades
// to Unknown, not Text, so the client can tell a degraded part from something
// the agent actually said.
#[test]
fn a_version_older_than_every_part_degrades_all_of_them_to_unknown() {
    let part = Part::Diff {
        path: "src/lib.rs".into(),
        unified: "@@ -1 +1 @@".into(),
        added: Some(1),
        removed: Some(1),
        fallback_text: "DIFF src/lib.rs".into(),
    };

    assert_eq!(
        part.for_version(WireVersion(0)),
        Part::Unknown {
            kind: "diff".into(),
            fallback_text: "DIFF src/lib.rs".into(),
        }
    );
}

#[test]
fn a_version_that_carries_a_part_leaves_it_alone() {
    let part = Part::Text {
        text: "hello".into(),
    };

    assert_eq!(part.for_version(WireVersion(1)), part);
}

// Downgrading a part that is already Unknown must not wrap it again, or a
// two-version gap produces `unknown` nested inside `unknown`.
#[test]
fn downgrading_an_unknown_part_leaves_it_unchanged() {
    let part = Part::Unknown {
        kind: "diff".into(),
        fallback_text: "DIFF src/lib.rs".into(),
    };

    assert_eq!(part.for_version(WireVersion(0)), part);
}

#[test]
fn every_part_in_this_build_was_introduced_at_version_one() {
    assert_eq!(
        Part::Status {
            label: "compiling".into(),
            detail: None,
            fallback_text: "compiling".into(),
        }
        .since(),
        WireVersion(1)
    );
}

// Every variant reports a distinct kind, or a downgrade would label two
// different parts the same and the client could not tell them apart.
#[test]
fn no_two_part_kinds_share_a_name() {
    let kinds = [
        Part::Text {
            text: String::new(),
        }
        .kind(),
        Part::ToolUse {
            name: String::new(),
            input: String::new(),
            result: None,
            status: ToolStatus::Ok,
            fallback_text: String::new(),
        }
        .kind(),
        Part::Diff {
            path: String::new(),
            unified: String::new(),
            added: None,
            removed: None,
            fallback_text: String::new(),
        }
        .kind(),
        Part::Todo {
            items: Vec::new(),
            fallback_text: String::new(),
        }
        .kind(),
        Part::Table {
            columns: Vec::new(),
            rows: Vec::new(),
            fallback_text: String::new(),
        }
        .kind(),
        Part::Status {
            label: String::new(),
            detail: None,
            fallback_text: String::new(),
        }
        .kind(),
        Part::Unknown {
            kind: String::new(),
            fallback_text: String::new(),
        }
        .kind(),
    ];

    let unique: std::collections::BTreeSet<&str> = kinds.iter().copied().collect();

    assert_eq!(unique.len(), kinds.len());
}
