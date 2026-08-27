use tethera_common::protocol::terminal::{Key, Mods, TerminalInput};
use tethera_common::structs::transcript::{Answer, Ask, QuestionOption};
use tethera_common::structs::agent::Agent;
use tethera_server_lib::terminal::Picker;

fn options(labels: &[&str]) -> Vec<QuestionOption> {
    labels
        .iter()
        .map(|label| QuestionOption {
            label: (*label).to_string(),
            description: None,
        })
        .collect()
}

fn ask(labels: &[&str]) -> Ask {
    Ask {
        header: None,
        prompt: "Which route owns pair?".into(),
        options: options(labels),
        multi_select: false,
        allows_free_text: true,
    }
}

fn pressed(steps: &[TerminalInput]) -> Vec<String> {
    steps
        .iter()
        .map(|step| match step {
            TerminalInput::Key { key, .. } => format!("{key:?}"),
            TerminalInput::Text(text) => format!("type {text:?}"),
        })
        .collect()
}

/// Built the way the server builds one: off the harness's own table.
///
/// Driving a picker requires having been handed one, and a harness nobody has
/// measured cannot be handed one — which is what stops this key sequence being
/// pressed at a screen that was never looked at.
fn picker() -> Picker {
    Picker::for_agent(Agent::Claude).expect("this harness has been measured")
}

// **The rows are numbered on screen and the number does not always select.**
// This asserted the opposite until the side-by-side picker was driven by hand:
// it prints `1. Alpha` and ignores `1` completely, which is how an answer could
// be typed into a pane, reported as sent, and never arrive. Moving to the row
// and pressing Enter drives both pickers, measured on each.
#[test]
fn a_choice_moves_to_its_row_and_takes_it() {
    let steps = picker().steps(&[ask(&["Rewrite", "Register"])], &[Answer::Choice(1)])
        .expect("a second choice");

    assert_eq!(pressed(&steps), vec!["Down", "Enter"]);
}

// A single-select answer advances the picker on its own, so a set of them is
// one walk after another. Anything else would answer the first question twice
// and never reach the second.
#[test]
fn a_set_is_answered_one_question_after_another() {
    let asks = vec![ask(&["SQLite", "Postgres"]), ask(&["8080", "9090"])];
    let steps =
        picker().steps(&asks, &[Answer::Choice(0), Answer::Choice(1)]).expect("both answered");

    assert_eq!(pressed(&steps), vec!["Enter", "Down", "Enter"]);
}

// Toggling leaves the picker on the same question, so something has to move off
// it. Measured: `Right` is what does.
#[test]
fn a_multi_select_toggles_each_row_and_then_moves_on() {
    let mut multi = ask(&["Drop the badge", "Close the gap", "Rename the widget"]);
    multi.multi_select = true;

    let steps = picker().steps(&[multi], &[Answer::Multi(vec![0, 2])]).expect("two toggled");

    assert_eq!(pressed(&steps), vec!["Char('1')", "Char('3')", "Right"]);
}

// The free-text row sits after the options — two options put "Type something"
// third — and it opens an editor that takes the text and a return. Reached the
// same way as any other row, so the picker that ignores numbers is driven too.
#[test]
fn free_text_selects_the_row_after_the_options_then_types() {
    let steps = picker().steps(
        &[ask(&["Widget", "Gadget"])],
        &[Answer::Text("Sprocket".into())],
    )
    .expect("free text");

    assert_eq!(
        pressed(&steps),
        vec!["Down", "Down", "Enter", "type \"Sprocket\"", "Enter"]
    );
}

// The set is answered at once or not at all. A count that does not line up would
// walk the picker off the end of the questions and press into whatever followed.
#[test]
fn a_set_answered_with_the_wrong_number_of_answers_is_refused() {
    let asks = vec![ask(&["SQLite", "Postgres"]), ask(&["8080", "9090"])];

    assert!(picker().steps(&asks, &[Answer::Choice(0)]).is_err());
    assert!(picker().steps(&asks, &[]).is_err());
}

// An option index nobody offered would press a number belonging to a different
// row — "Type something", or the harness's own "Chat about this".
#[test]
fn an_option_that_was_never_offered_is_refused() {
    assert!(picker().steps(&[ask(&["Rewrite", "Register"])], &[Answer::Choice(2)]).is_err());
    assert!(picker().steps(&[ask(&["Rewrite", "Register"])], &[Answer::Choice(99)]).is_err());
}

// The shape of the answer has to match the shape of the question, or the presses
// land somewhere the picker was not expecting.
#[test]
fn an_answer_of_the_wrong_shape_is_refused() {
    let single = ask(&["Rewrite", "Register"]);

    assert!(
        picker().steps(&[single.clone()], &[Answer::Multi(vec![0, 1])]).is_err(),
        "several choices for a question that takes one"
    );

    let mut no_text = single.clone();
    no_text.allows_free_text = false;

    assert!(
        picker().steps(&[no_text], &[Answer::Text("something".into())]).is_err(),
        "free text for a question that does not take it"
    );

    assert!(
        picker().steps(&[single], &[Answer::Text("   ".into())]).is_err(),
        "an answer of nothing at all"
    );
}

// Nothing is sent until every press is known. A set that is refused halfway
// would leave a person's picker part-driven, with the phone believing it had
// answered and the machine showing a question with some of its rows already
// pressed.
#[test]
fn a_set_with_one_bad_answer_sends_nothing() {
    let asks = vec![ask(&["SQLite", "Postgres"]), ask(&["8080", "9090"])];

    assert!(picker().steps(&asks, &[Answer::Choice(0), Answer::Choice(7)]).is_err());
}

// **A multi-select still toggles by number**, so past the ninth row it still has
// no key to press. The limit did not go away; it narrowed to the one answer
// shape that has not moved to the arrows, and is refused rather than mis-pressed.
#[test]
fn a_multi_select_row_beyond_the_number_keys_is_refused() {
    let mut many = ask(&[
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l",
    ]);
    many.multi_select = true;

    assert!(picker().steps(&[many], &[Answer::Multi(vec![10])]).is_err());
}


// The cursor is on the first row when a picker appears, so the first option is
// taken without moving at all.
#[test]
fn the_first_option_is_taken_where_the_cursor_already_is() {
    let ask = choice_of(&["Yes", "No"]);
    let steps = picker().steps(&[ask], &[Answer::Choice(0)]).expect("steps");

    assert_eq!(
        steps,
        vec![TerminalInput::Key { key: Key::Enter, mods: Mods::NONE }]
    );
}

// A number key runs out at nine; moving to a row does not. A picker longer than
// the digits stops being unanswerable.
#[test]
fn a_picker_longer_than_the_number_keys_is_still_answerable() {
    let labels: Vec<String> = (0..12).map(|n| format!("option {n}")).collect();
    let borrowed: Vec<&str> = labels.iter().map(String::as_str).collect();
    let steps = picker().steps(&[choice_of(&borrowed)], &[Answer::Choice(11)]).expect("steps");

    assert_eq!(steps.len(), 12, "eleven moves and one Enter");
    assert_eq!(
        steps.last(),
        Some(&TerminalInput::Key { key: Key::Enter, mods: Mods::NONE })
    );
}

fn choice_of(labels: &[&str]) -> Ask {
    Ask {
        header: None,
        prompt: "Which one?".to_string(),
        options: labels
            .iter()
            .map(|label| QuestionOption {
                label: (*label).to_string(),
                description: None,
            })
            .collect(),
        multi_select: false,
        allows_free_text: false,
    }
}
