use tethera_common::protocol::WireVersion;
use tethera_common::structs::ids::QuestionId;
use tethera_common::structs::primitives::Timestamp;
use tethera_common::structs::transcript::{
    Answer, AnswerRecord, Ask, Part, Question, QuestionOption,
};

fn options() -> Vec<QuestionOption> {
    vec![
        QuestionOption {
            label: "Rewrite before the router sees it".into(),
            description: Some("One place to fix.".into()),
        },
        QuestionOption {
            label: "Register pair as a real route".into(),
            description: Some("Fights the framework.".into()),
        },
    ]
}

fn ask(prompt: &str, options: Vec<QuestionOption>) -> Ask {
    Ask {
        header: None,
        prompt: prompt.into(),
        options,
        multi_select: false,
        allows_free_text: false,
    }
}

fn asks() -> Vec<Ask> {
    vec![ask("Which route owns pair?", options())]
}

fn a_question() -> Question {
    Question {
        id: QuestionId::parse("qs_a1").expect("valid"),
        fingerprint: Question::fingerprint_of(&asks()),
        asks: asks(),
    }
}

#[test]
fn a_question_round_trips_through_postcard() {
    let question = a_question();
    let bytes = postcard::to_stdvec(&question).expect("encode");

    assert_eq!(
        postcard::from_bytes::<Question>(&bytes).expect("decode"),
        question
    );
}

// The design draws a short description under each choice, because that is what
// the agent's own question modal renders. As one string an option cannot carry
// it.
#[test]
fn an_option_carries_a_description_beside_its_label() {
    assert_eq!(
        a_question().asks[0].options[0].description.as_deref(),
        Some("One place to fix.")
    );
}

// A harness asks up to four at once and stays blocked until it has every
// answer, so the set is what a person is put in front of and what gets
// answered. One fingerprint over the whole set is what makes that atomic.
#[test]
fn a_set_of_questions_is_one_question_with_one_fingerprint() {
    let several = vec![
        ask("Which route owns pair?", options()),
        ask("Rename the widget too?", options()),
    ];

    let question = Question {
        id: QuestionId::parse("qs_a1").expect("valid"),
        fingerprint: Question::fingerprint_of(&several),
        asks: several.clone(),
    };

    assert_eq!(question.asks.len(), 2);
    assert_eq!(question.prompt(), "Which route owns pair?");
    assert_ne!(
        Question::fingerprint_of(&several),
        Question::fingerprint_of(&several[..1]),
        "a set that lost a question must not fingerprint as the whole set"
    );
}

// Reordering is not a no-op: the answers go back positionally, so a set whose
// questions swapped would take each answer onto the wrong one.
#[test]
fn reordering_the_questions_changes_the_fingerprint() {
    let forward = vec![ask("First?", options()), ask("Second?", options())];
    let reversed = vec![ask("Second?", options()), ask("First?", options())];

    assert_ne!(
        Question::fingerprint_of(&forward),
        Question::fingerprint_of(&reversed)
    );
}

// The fingerprint is what stops an answer landing on a question the pane has
// moved past. Both ends compute it identically from identical inputs, so it
// lives here rather than in the server.
#[test]
fn the_same_question_fingerprints_the_same_way_every_time() {
    assert_eq!(
        Question::fingerprint_of(&asks()),
        Question::fingerprint_of(&asks())
    );
}

#[test]
fn a_changed_prompt_changes_the_fingerprint() {
    assert_ne!(
        Question::fingerprint_of(&[ask("Run this?", options())]),
        Question::fingerprint_of(&[ask("Delete this?", options())])
    );
}

#[test]
fn a_changed_option_set_changes_the_fingerprint() {
    let mut fewer = options();
    fewer.pop();

    assert_ne!(
        Question::fingerprint_of(&[ask("Run this?", options())]),
        Question::fingerprint_of(&[ask("Run this?", fewer)])
    );
}

// Two options whose label and description are split differently must not
// collide, or a question could be answered against a stale option list whose
// fingerprint happened to match.
#[test]
fn moving_text_between_a_label_and_its_description_changes_the_fingerprint() {
    let split = vec![QuestionOption {
        label: "Allow".into(),
        description: Some("always".into()),
    }];
    let joined = vec![QuestionOption {
        label: "Allowalways".into(),
        description: None,
    }];

    assert_ne!(
        Question::fingerprint_of(&[ask("Run this?", split)]),
        Question::fingerprint_of(&[ask("Run this?", joined)])
    );
}

// A header is part of what the person is being asked, so changing it has to
// change the digest too.
#[test]
fn a_changed_header_changes_the_fingerprint() {
    let headed = |header: &str| {
        let mut one = ask("Run this?", options());
        one.header = Some(header.into());

        vec![one]
    };

    assert_ne!(
        Question::fingerprint_of(&headed("Permission")),
        Question::fingerprint_of(&headed("Confirm"))
    );
}

// AskUserQuestion supports a multi-select and a free-text "Other", so all three
// answer shapes have to survive the wire.
#[test]
fn every_answer_shape_round_trips() {
    for answer in [
        Answer::Choice(1),
        Answer::Multi(vec![0, 2]),
        Answer::Text("something else entirely".into()),
    ] {
        let bytes = postcard::to_stdvec(&answer).expect("encode");

        assert_eq!(
            postcard::from_bytes::<Answer>(&bytes).expect("decode"),
            answer
        );
    }
}

// A hole rather than a shorter list. The harness records what it has, and a
// missing answer that shortened the vector would shift every later answer onto
// the wrong question.
#[test]
fn a_part_answered_set_keeps_its_holes_in_place() {
    let record = AnswerRecord {
        answers: vec![None, Some(Answer::Choice(0))],
        at: Timestamp(1_766_000_000_000),
    };

    let bytes = postcard::to_stdvec(&record).expect("encode");
    let back: AnswerRecord = postcard::from_bytes(&bytes).expect("decode");

    assert_eq!(back.answers.len(), 2);
    assert!(back.answers[0].is_none());
    assert_eq!(back, record);
}

#[test]
fn an_answered_question_records_what_was_chosen_and_when() {
    let part = Part::Question {
        question: a_question(),
        answered: Some(AnswerRecord {
            answers: vec![Some(Answer::Choice(0))],
            at: Timestamp(1_766_000_000_000),
        }),
        fallback_text: "Which route? 1 Rewrite 2 Register".into(),
    };

    let bytes = postcard::to_stdvec(&part).expect("encode");

    assert_eq!(postcard::from_bytes::<Part>(&bytes).expect("decode"), part);
}

#[test]
fn a_question_part_degrades_to_its_fallback_text() {
    let part = Part::Question {
        question: a_question(),
        answered: None,
        fallback_text: "Which route? 1 Rewrite 2 Register".into(),
    };

    assert_eq!(
        part.for_version(WireVersion(0)),
        Part::Unknown {
            kind: "question".into(),
            fallback_text: "Which route? 1 Rewrite 2 Register".into(),
        }
    );
}
