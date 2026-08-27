use super::Fixture;
use tethera_common::structs::agent::AgentStatus;
use tethera_common::structs::transcript::{Part, Question, Turn};
use tethera_server_lib::transcript::{SessionCatalog, StatusRule};

/// A projects tree shaped the way the harness shapes one, holding the fixtures
/// under a flattened directory name.
struct Tree {
    home: tempfile::TempDir,
}

impl Tree {
    fn with(fixtures: &[(&str, &str)]) -> Self {
        let home = tempfile::tempdir().expect("a temporary home");
        let projects = home.path().join(".claude").join("projects");

        for (project, fixture) in fixtures {
            let directory = projects.join(project);
            std::fs::create_dir_all(&directory).expect("a project directory");

            let session = fixture.trim_end_matches(".jsonl");
            std::fs::copy(
                Fixture::path(fixture),
                directory.join(format!("{session}.jsonl")),
            )
            .expect("a session file");
        }

        Self { home }
    }

    fn catalog(&self) -> SessionCatalog {
        SessionCatalog::new(self.home.path())
    }
}

#[test]
fn a_session_is_summarised_from_its_own_records() {
    let tree = Tree::with(&[("-home-dev-project", "plain.jsonl")]);
    let catalog = tree.catalog();

    let path = catalog.locate("plain").expect("the session file");
    let summary = catalog.summarise(&path).expect("a summary");

    assert_eq!(summary.session_id, "plain");
    assert_eq!(summary.cwd.as_deref(), Some("/home/dev/project"));
    assert!(summary.started_at.expect("a start") < summary.last_active.expect("an end"));
}

// The harness's rename command writes its own kind of record, `custom-title`,
// and leaves the `ai-title` it wrote earlier in place. Reading only the machine's
// title is why a rename appeared to do nothing: the session kept the name it was
// given on its first turn for the rest of its life.
#[test]
fn a_session_a_person_renamed_reports_the_name_they_gave_it() {
    let tree = Tree::with(&[("-home-dev-project", "renamed.jsonl")]);
    let catalog = tree.catalog();

    let path = catalog.locate("renamed").expect("the session file");
    let summary = catalog.summarise(&path).expect("a summary");

    assert_eq!(summary.title.as_deref(), Some("Renamed By Probe"));
}

// And the reason recency alone cannot decide it. The harness goes on writing
// records after a rename but never revises its own title, so the newest record
// carrying any title at all is routinely still the machine's — picking that one
// would put the old name back the moment the conversation continued.
#[test]
fn a_persons_name_outranks_the_machines_however_old_it_is() {
    let tree = Tree::with(&[("-home-dev-project", "renamed.jsonl")]);
    let catalog = tree.catalog();

    let path = catalog.locate("renamed").expect("the session file");
    let summary = catalog.summarise(&path).expect("a summary");

    assert_ne!(
        summary.title.as_deref(),
        Some("Say hello briefly"),
        "the title the harness wrote survived a rename"
    );
}

// A session nobody renamed still carries the machine's title, which is the case
// that must not break: most conversations are never renamed at all.
#[test]
fn a_session_nobody_renamed_keeps_the_title_the_harness_wrote() {
    let tree = Tree::with(&[("-home-dev-project", "plain.jsonl")]);
    let catalog = tree.catalog();

    let path = catalog.locate("plain").expect("the session file");
    let summary = catalog.summarise(&path).expect("a summary");

    assert_eq!(summary.title, None, "plain.jsonl carries no title record");
}

// The only route from a conversation to its records when no working directory is
// in hand, which is every route: herdr records the session id and discards the
// path.
#[test]
fn a_session_is_found_by_its_id_alone_across_every_project() {
    let tree = Tree::with(&[
        ("-home-dev-one", "plain.jsonl"),
        ("-home-dev-two", "tools.jsonl"),
    ]);
    let catalog = tree.catalog();

    assert!(catalog.locate("tools").is_some());
    assert!(catalog.locate("plain").is_some());
    assert!(catalog.locate("never-written").is_none());
}

// A projects directory that is not there is a machine that has never run the
// harness, which is a real and unremarkable state.
#[test]
fn a_machine_that_has_never_run_the_harness_lists_nothing() {
    let home = tempfile::tempdir().expect("a temporary home");
    let catalog = SessionCatalog::new(home.path());

    assert!(catalog.discover().is_empty());
    assert!(catalog.locate("anything").is_none());
}

#[test]
fn discovery_finds_every_session_under_every_project() {
    let tree = Tree::with(&[
        ("-home-dev-one", "plain.jsonl"),
        ("-home-dev-two", "tools.jsonl"),
        ("-home-dev-three", "grouped.jsonl"),
    ]);

    assert_eq!(tree.catalog().discover().len(), 3);
}

// Nothing is running, so nothing is working or waiting. It also keeps the cost
// honest: only a bound conversation is worth reading a tail for.
#[test]
fn a_conversation_with_no_pane_is_done_whatever_its_records_say() {
    let turns = Fixture::turns("tools.jsonl");

    assert_eq!(StatusRule::decide(false, &turns, true), AgentStatus::Done);
}

#[test]
fn a_bound_conversation_with_a_call_in_flight_is_working() {
    let turns = Fixture::turns("tools.jsonl");

    assert_eq!(StatusRule::decide(true, &turns, true), AgentStatus::Working);
}

// A file that has not grown is not working, whatever its last record says. A
// process that died mid-call would otherwise report Working forever.
//
// Stalled and not Idle, which is the distinction that matters to a person: an
// agent reported idle looks finished, and nobody goes to look at a machine that
// says it is done.
#[test]
fn a_call_in_flight_in_a_file_that_stopped_growing_is_stalled() {
    let turns = Fixture::turns("tools.jsonl");

    assert_eq!(StatusRule::decide(true, &turns, false), AgentStatus::Stalled);
}

#[test]
fn a_bound_conversation_whose_newest_question_is_unanswered_is_blocked() {
    let turns = vec![asking(None)];

    assert_eq!(StatusRule::decide(true, &turns, false), AgentStatus::Blocked);
}

// An earlier question the agent moved on from is history, not something a person
// is being asked now.
#[test]
fn a_question_that_was_answered_does_not_leave_a_conversation_blocked() {
    let turns = vec![asking(Some(()))];

    assert_eq!(StatusRule::decide(true, &turns, false), AgentStatus::Idle);
}

// A harness offers ways out of its own picker — "chat about this", or simply
// typing a reply — and taking one leaves the tool call with **no answer ever
// recorded against it**. The question is over; the record of it never closes.
//
// Reproduced on a device: a card read "waiting on you" nine minutes after the
// agent had answered and gone quiet, and nothing could clear it. So a question
// with anything after it is history, whatever its own record says.
#[test]
fn a_question_the_conversation_moved_past_is_not_still_pending() {
    let mut turns = vec![asking(None)];
    turns.push(said("Blue"));

    assert_eq!(
        StatusRule::decide(true, &turns, false),
        AgentStatus::Idle,
        "an unanswered question with a later turn after it still read as blocked"
    );
    assert!(StatusRule::pending_question(&turns).is_none());
}

// And the case it must not break: a question genuinely being waited on is the
// newest thing there is, because nothing follows it until it is answered.
#[test]
fn a_question_that_is_the_newest_thing_said_is_still_pending() {
    let turns = vec![said("which store?"), asking(None)];

    assert_eq!(StatusRule::decide(true, &turns, false), AgentStatus::Blocked);
    assert!(StatusRule::pending_question(&turns).is_some());
}

#[test]
fn the_preview_of_a_blocked_conversation_is_the_question_it_is_waiting_on() {
    let turns = vec![asking(None)];

    assert_eq!(
        StatusRule::preview(&turns).as_deref(),
        Some("Which store should the server use?")
    );
}

#[test]
fn the_preview_of_an_ordinary_conversation_is_the_agents_last_line() {
    let turns = Fixture::turns("plain.jsonl");

    assert_eq!(StatusRule::preview(&turns).as_deref(), Some("Renaming it now."));
}

/// A turn in which somebody said something. Ordinary conversation after a
/// question, which is what proves the question is over.
fn said(text: &str) -> Turn {
    use tethera_common::structs::ids::TurnId;
    use tethera_common::structs::primitives::{Cursor, Timestamp};
    use tethera_common::structs::transcript::Role;

    Turn::new(
        Cursor("o9".into()),
        TurnId("t9".into()),
        Timestamp(1),
        Role::Operator,
        vec![Part::Text { text: text.to_string() }],
    )
}

fn asking(answered: Option<()>) -> Turn {
    use tethera_common::structs::ids::{QuestionId, TurnId};
    use tethera_common::structs::primitives::{Cursor, Timestamp};
    use tethera_common::structs::transcript::{Answer, AnswerRecord, Ask, Role};

    let prompt = "Which store should the server use?";
    let options = Vec::new();

    Turn::new(
        Cursor("o0".into()),
        TurnId("q1".into()),
        Timestamp(0),
        Role::Agent,
        vec![Part::Question {
            question: {
                let asks = vec![Ask {
                    header: None,
                    prompt: prompt.to_string(),
                    options,
                    multi_select: false,
                    allows_free_text: true,
                }];

                Question {
                    id: QuestionId::mint("q1"),
                    fingerprint: Question::fingerprint_of(&asks),
                    asks,
                }
            },
            answered: answered.map(|_| AnswerRecord {
                answers: vec![Some(Answer::Choice(0))],
                at: Timestamp(1),
            }),
            fallback_text: prompt.to_string(),
        }],
    )
}
