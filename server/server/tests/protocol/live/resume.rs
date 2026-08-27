use tethera_common::structs::ids::{ConversationId, PaneId, ProfileId, TabId, WorkspaceId};
use tethera_common::structs::terminal::{Pane, Size};
use tethera_server_lib::protocol::live::ResumeGate;

/// A pane, described by the two facts this gate reads and the two it decides on.
fn pane(cwd: Option<&str>, title: Option<&str>, agent: bool, named: bool) -> Pane {
    Pane {
        id: PaneId::mint("p1"),
        tab_id: TabId::mint("t1"),
        workspace_id: WorkspaceId::mint("w1"),
        label: "1".into(),
        title: title.map(str::to_string),
        cwd: cwd.map(str::to_string),
        size: Size {
            cols: 80,
            rows: 24,
        },
        focused: false,
        foreground_command: None,
        conversation: named.then(|| ConversationId::mint("other")),
        agent: agent.then(|| ProfileId("claude".into())),
    }
}

const HERE: &str = "C:/Users/charl/projects/tethera";
const MINE: Option<&str> = Some("Transcript reader spec");

// The ordinary case, and the one that must not regress into a refusal. A machine
// with nothing running has nothing to protect, and a person whose agent finished
// an hour ago has to be able to pick it back up.
#[test]
fn a_machine_running_nothing_admits_every_resume() {
    assert!(ResumeGate::admits(HERE, MINE, &[]));
    assert!(ResumeGate::admits(
        HERE,
        MINE,
        &[pane(Some(HERE), MINE, false, false)]
    ));
}

// The whole reason this exists. herdr reports a live agent for a pane and no
// session identity for it perhaps a third of the time on a working machine, so
// the binding that `resume` used to guard on is simply absent — and starting a
// second agent puts two processes on one set of records.
#[test]
fn an_unnamed_agent_on_the_same_session_refuses_the_resume() {
    let running = pane(Some(HERE), MINE, true, false);

    assert!(!ResumeGate::admits(HERE, MINE, &[running]));
}

// An agent that announced which session it is running is not this one, or the
// caller would have had a binding and never reached here. Refusing on it would
// block every resume on a busy machine.
#[test]
fn an_agent_that_named_its_session_is_not_in_the_way() {
    let elsewhere = pane(Some(HERE), MINE, true, true);

    assert!(ResumeGate::admits(HERE, MINE, &[elsewhere]));
}

// A harness indexes its sessions per directory, so an agent running somewhere
// else cannot be this session however it is titled.
#[test]
fn an_unnamed_agent_in_another_directory_is_ruled_out() {
    let far = pane(Some("C:/Users/charl/projects/bvc"), MINE, true, false);

    assert!(ResumeGate::admits(HERE, MINE, &[far]));
}

// A terminal's title and a session's title come from the same record the harness
// writes, so two that differ are two sessions. Measured against live panes, not
// assumed: three unnamed agents were matched to their sessions by title exactly.
#[test]
fn a_different_title_in_the_same_directory_is_ruled_out() {
    let other = pane(Some(HERE), Some("Something else entirely"), true, false);

    assert!(ResumeGate::admits(HERE, MINE, &[other]));
}

// The same directory spelled the other way round is the same directory. A person
// types one separator, the harness records another, and the backend reports a
// third; a comparison that missed on that would rule the pane out and start the
// second agent.
#[test]
fn a_directory_spelled_differently_is_still_the_same_directory() {
    for spelling in [
        "C:\\Users\\charl\\projects\\tethera",
        "c:/users/charl/projects/tethera",
        "C:/Users/charl/projects/tethera/",
    ] {
        let running = pane(Some(spelling), MINE, true, false);

        assert!(
            !ResumeGate::admits(HERE, MINE, &[running]),
            "{spelling} was treated as a different directory"
        );
    }
}

// Uncertainty resolves to refusing, in both directions it can arrive from. A
// title is only ever read to rule a pane *out*, so its absence cannot be used to
// rule one in.
#[test]
fn an_unnamed_agent_nothing_can_be_compared_against_still_refuses() {
    let untitled = pane(Some(HERE), None, true, false);
    let no_directory = pane(None, MINE, true, false);

    assert!(!ResumeGate::admits(HERE, MINE, &[untitled]));
    assert!(!ResumeGate::admits(HERE, MINE, &[no_directory]));
    assert!(!ResumeGate::admits(HERE, None, &[pane(Some(HERE), MINE, true, false)]));
}

// One pane in the way is enough, whatever else is running. The gate answers
// about the whole machine rather than about a pane the caller picked.
#[test]
fn one_pane_in_the_way_refuses_a_machine_full_of_harmless_ones() {
    let panes = vec![
        pane(Some("C:/elsewhere"), Some("far away"), true, false),
        pane(Some(HERE), MINE, true, true),
        pane(Some(HERE), MINE, true, false),
        pane(Some(HERE), MINE, false, false),
    ];

    assert!(!ResumeGate::admits(HERE, MINE, &panes));
}
