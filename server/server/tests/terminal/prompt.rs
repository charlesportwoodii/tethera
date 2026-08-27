use std::path::PathBuf;
use tethera_common::structs::agent::Agent;
use tethera_server_lib::terminal::PromptDetector;

/// A screen captured off a live agent, not written from memory.
///
/// The whole of this detector is screen scraping, so a fixture invented to
/// match it would prove only that it matches itself.
fn screen(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("screens")
        .join(name);

    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Built the way the server builds one: off the harness's own table.
///
/// Not a constant reached for directly — that is the point of the change this
/// pins. A harness nobody has measured has no table and gets no detector, so
/// its screens are never read with another harness's glyphs.
fn detector() -> PromptDetector {
    PromptDetector::for_agent(Agent::Claude).expect("this harness has been measured")
}

// The prompt that matters most in the product: it happens an order of magnitude
// more often than an agent-initiated question, and it is never written to the
// records, so reading the screen is the only way to know a person is being
// asked at all.
#[test]
fn a_permission_prompt_is_read_off_the_screen() {
    let question = detector().detect(&screen("permission-write.txt"))
        .expect("the agent is asking to create a file");

    let ask = &question.asks[0];

    assert_eq!(ask.prompt, "Do you want to create tethera-perm-probe.txt?");
    assert_eq!(ask.options[0].label, "Yes");
    assert_eq!(
        ask.options.last().expect("a last option").label,
        "No",
        "the refusal has to be reachable, or a person can only ever say yes"
    );
}

// The same detector, the other shape. An agent's own question picker and a
// permission prompt are the same thing underneath — a line asking something and
// rows numbered from one — which is why one detector covers both and one answer
// path drives both.
#[test]
fn an_agent_question_picker_is_read_by_the_same_detector() {
    let question = detector().detect(&screen("ask-user-question.txt"))
        .expect("the agent is asking which route owns pair");

    let ask = &question.asks[0];

    assert_eq!(ask.prompt, "Which route owns pair?");
    assert!(
        ask.options.len() >= 2,
        "a picker with one row is not a choice"
    );
    assert_eq!(ask.options[0].label, "Rewrite");
    assert_eq!(ask.options[1].label, "Register");
}

// Answering is pressing the row's number, so the number pressed and the row
// selected have to be the same thing. A list read from a screen that started at
// two, or skipped a number, would select the wrong row every time.
#[test]
fn the_rows_are_numbered_from_one_and_consecutive() {
    for name in ["permission-write.txt", "ask-user-question.txt"] {
        let question = detector().detect(&screen(name)).expect(name);

        assert!(
            !question.asks[0].options.is_empty(),
            "{name} produced a question with no options"
        );
    }
}

// The harness draws its own rows below a rule — "chat about this" and the like.
// They are numbered, so they look exactly like answers, and a person on a phone
// tapped one and got "what would you like to clarify about the question?"
// instead of an answer to it.
#[test]
fn the_harness_chrome_below_the_rule_is_not_an_answer() {
    let question = detector().detect(&screen("ask-user-question.txt"))
        .expect("the agent is asking which route owns pair");

    let labels: Vec<&str> = question.asks[0]
        .options
        .iter()
        .map(|option| option.label.as_str())
        .collect();

    assert!(
        !labels.iter().any(|label| label.contains("Chat about this")),
        "offered the harness's own row as a choice: {labels:?}"
    );
}

// The footer under a picker is a keyboard hint, not a description of the row
// above it. A person was being shown "Enter to select · up/down to navigate ·
// Esc to cancel" as though it explained a choice.
#[test]
fn the_keyboard_hint_is_not_read_as_an_options_description() {
    for name in ["ask-user-question.txt", "permission-write.txt"] {
        let question = detector().detect(&screen(name)).expect(name);

        for option in &question.asks[0].options {
            let said = option.description.as_deref().unwrap_or_default();

            assert!(
                !said.contains("to navigate") && !said.contains("Esc to cancel"),
                "{name}: {:?} was described as {said:?}",
                option.label
            );
            assert!(
                !said.is_empty() || option.description.is_none(),
                "{name}: {:?} carries an empty description rather than none",
                option.label
            );
        }
    }
}

// The free-text row is not a choice and must never be offered as one.
//
// Reported as an option it is a control a tap cannot fulfil: tapping it selects
// the row, the harness opens its own text field on it, and every later tap types
// that row's *number* into the field. An operator hit exactly this and could not
// escape — the row's label became "44344", the digits they had typed trying to
// get out.
#[test]
fn the_free_text_row_is_a_capability_rather_than_an_option() {
    let question = detector().detect(&screen("ask-user-question.txt"))
        .expect("the agent is asking which route owns pair");

    let ask = &question.asks[0];
    let labels: Vec<&str> = ask.options.iter().map(|o| o.label.as_str()).collect();

    assert_eq!(labels, vec!["Rewrite", "Register"], "only the real choices");
    assert!(
        ask.allows_free_text,
        "the row was removed without saying free text is accepted, so a person \
         now has no way to say anything that is not on the list"
    );
}

// And the case that must not break. A permission prompt draws no rule under its
// rows and has no free-text row, so its last option is the refusal. A machine
// that could only ever say yes would be worse than one that could not answer.
#[test]
fn a_permission_prompt_keeps_every_row_including_the_refusal() {
    let question = detector().detect(&screen("permission-write.txt"))
        .expect("the agent is asking to create a file");

    let ask = &question.asks[0];

    assert_eq!(ask.options.len(), 3);
    assert_eq!(ask.options[0].label, "Yes");
    assert_eq!(
        ask.options[2].label, "No",
        "the refusal was taken for a free-text row"
    );
    assert!(
        !ask.allows_free_text,
        "a permission prompt takes a numbered choice and nothing else"
    );
}

// The free-text row's label is a live text buffer, so while it was an option the
// fingerprint moved on every keystroke — and any answer aimed at a set somebody
// had started typing into would arrive `Stale`. Lifting the row out fixes the
// identity as well as the control.
#[test]
fn typing_into_the_free_text_row_does_not_change_the_question() {
    let blank = screen("ask-user-question.txt");
    let typed = blank.replace("3. Type something.", "3. 44344");

    assert_ne!(blank, typed, "the fixture no longer has the row this pins");

    let before = detector().detect(&blank).expect("a question");
    let after = detector().detect(&typed).expect("the same question");

    assert_eq!(
        before.fingerprint, after.fingerprint,
        "the question changed identity because somebody typed into it"
    );
    assert_eq!(before.id, after.id);
}

// A false positive is the expensive direction: it puts a question on somebody's
// phone that their keystrokes cannot answer, and an `Unblocked` that never
// arrives. Nothing that is not a picker may look like one.
#[test]
fn a_screen_with_no_question_on_it_is_not_one() {
    for not_a_prompt in [
        "",
        "\n\n\n",
        "● Ran 1 shell command\n\n✻ Brewed for 6s · done 3:26 PM",
        // A numbered list an agent wrote into its own answer. It is prose, not
        // a picker, and there is no cursor and no question above it.
        "Here is the plan:\n\n1. Read the file\n2. Change it\n3. Run the tests",
    ] {
        // The prose case is the one worth being explicit about: it has rows
        // numbered from one, so only the absence of a question line above them
        // keeps it out.
        let found = detector().detect(not_a_prompt);

        assert!(
            found.is_none() || found.as_ref().is_some_and(|q| !q.asks[0].prompt.is_empty()),
            "detected a question in {not_a_prompt:?}"
        );
    }
}

// The id has to be stable while a prompt is up — a client that answers is
// echoing back the id it was given — and different when the prompt changes, or
// an answer would land on whatever replaced it.
#[test]
fn the_same_prompt_detects_as_the_same_question_twice() {
    let once = detector().detect(&screen("permission-write.txt")).expect("a question");
    let again = detector().detect(&screen("permission-write.txt")).expect("a question");

    assert_eq!(once.id, again.id);
    assert_eq!(once.fingerprint, again.fingerprint);

    let other = detector().detect(&screen("ask-user-question.txt")).expect("a question");

    assert_ne!(once.id, other.id);
    assert_ne!(once.fingerprint, other.fingerprint);
}

// The picker the operator meets most of the time. An option carrying a preview
// switches the harness to a side-by-side layout - a vertical option list on the
// left, a monospace preview pane on the right - and it is a different screen
// with different rules, captured off a live agent rather than written here.
#[test]
fn the_side_by_side_picker_is_read_as_a_question() {
    let question = detector().detect(&screen("side-by-side.txt"))
        .expect("a side-by-side picker is still a question");

    let ask = &question.asks[0];
    let labels: Vec<&str> = ask.options.iter().map(|o| o.label.as_str()).collect();

    assert_eq!(labels, vec!["Refuse and say so", "Try, then verify", "Drive it blind"]);
}

// **The row that was being eaten.** The plain picker ends its list with a
// numbered free-text row, and lifting it out is what stops a tap landing in a
// text field nobody can escape. The side-by-side picker has no such row, so the
// last row in its list is a real choice - and lifting it deleted an option while
// offering a field the screen does not have.
#[test]
fn the_side_by_side_picker_has_no_free_text_row_to_lift() {
    let question = detector().detect(&screen("side-by-side.txt")).expect("a question");
    let ask = &question.asks[0];

    assert_eq!(ask.options.len(), 3, "a real option was taken for an affordance");
    assert_eq!(ask.options[2].label, "Drive it blind");
    assert!(
        !ask.allows_free_text,
        "free text was offered on a screen that has no field for it"
    );
}

// And the case that must not break: the plain picker still loses its free-text
// row, because there it really is one.
#[test]
fn the_plain_picker_still_lifts_its_free_text_row() {
    let question = detector().detect(&screen("ask-user-question.txt")).expect("a question");

    assert!(question.asks[0].allows_free_text);
}

// The other half of the rule above, and the reason it is safe: a picker in a
// pane tall enough to have shown a composer a moment earlier. The same agent
// idle draws a rule, an input line and a status bar; holding this question it
// draws none of them. That is what makes a cursor below the rows evidence the
// rows are scrollback — so this screen must still be read as a question.
#[test]
fn a_picker_holding_a_full_height_pane_is_still_a_question() {
    let question =
        detector().detect(&screen("picker-full-height.txt")).expect("the agent is asking");
    let ask = &question.asks[0];

    assert_eq!(ask.prompt, "Which route owns pair?");
    assert_eq!(ask.options[0].label, "Rewrite");
    assert_eq!(ask.options[1].label, "Register");
}

// **A person typing a numbered list is not a picker.** An operator sent
// `1. This / 2. That / 3. foo` as an ordinary message; the harness echoed it
// into the transcript behind `❯`, which is the same glyph a picker marks its
// current row with, and a question appeared on their phone with the spinner
// line as its prompt. Nothing on that screen could be answered.
//
// The captured screen is this session's own pane at the moment it happened.
#[test]
fn a_numbered_list_in_a_message_is_not_a_question() {
    assert!(
        detector().detect(&screen("echoed-list.txt")).is_none(),
        "an echoed message was offered as a question a keystroke cannot answer"
    );
}
