use tethera_common::protocol::error::WireError;
use tethera_common::structs::ids::QuestionId;
use tethera_common::structs::transcript::{Ask, Question};
use tethera_server_lib::protocol::live::BlockWatch;

fn a_question(prompt: &str) -> Question {
    let asks = vec![Ask {
        header: None,
        prompt: prompt.to_string(),
        options: Vec::new(),
        multi_select: false,
        allows_free_text: false,
    }];

    Question {
        id: QuestionId::mint(prompt),
        fingerprint: Question::fingerprint_of(&asks),
        asks,
    }
}

// **The bug this exists to prevent.** Reading the screen is a subprocess behind
// an admission gate shared with every other terminal call, so losing that race
// and getting `Busy` is ordinary. Folded into "no question pending" it publishes
// `Unblocked`, which takes a live prompt off somebody's phone while they are
// part-way through answering it — and the answer they then send is refused,
// because the machine has just told itself there is nothing to answer.
//
// Reported twice from a device before it was found: the sheet closed and the
// answer never arrived.
#[test]
fn a_screen_that_could_not_be_read_is_not_a_screen_with_no_question() {
    let settled = BlockWatch::settle(Ok(None), Err(WireError::Busy));

    assert!(
        matches!(settled, Err(WireError::Busy)),
        "a busy backend was reported as no question pending: {settled:?}"
    );
}

// The same rule from the other side. A transcript this machine could not read
// says nothing about whether a question is waiting.
#[test]
fn records_that_could_not_be_read_are_not_records_with_no_question() {
    let unreadable = Err(WireError::Backend {
        message: "the records could not be read".to_string(),
    });

    assert!(BlockWatch::settle(unreadable, Ok(None)).is_err());
}

// And the case that must stay cheap and quiet: both sources answered, neither
// had anything. This is every poll of every idle conversation, and it has to
// report a plain absence or nothing would ever clear.
#[test]
fn nothing_pending_is_reported_as_nothing_when_both_sources_answered() {
    assert_eq!(BlockWatch::settle(Ok(None), Ok(None)), Ok(None));
}

// A question found is a question found. Once there is one, a source that failed
// has nothing left to be uncertain about.
#[test]
fn a_question_found_on_screen_stands_even_if_the_records_failed() {
    let found = a_question("Do you want to create the file?");
    let settled = BlockWatch::settle(Err(WireError::Busy), Ok(Some(found.clone())));

    assert_eq!(settled, Ok(Some(found)));
}

// The records are preferred where they have it: a screen carries the prompt and
// the rows and nothing else, while the records carry headers, descriptions and
// whether free text is allowed — all of which a person is shown.
#[test]
fn the_records_win_when_the_screen_shows_nothing() {
    let recorded = a_question("Which route owns pair?");
    let settled = BlockWatch::settle(Ok(Some(recorded.clone())), Ok(None));

    assert_eq!(settled, Ok(Some(recorded)));
}
