use std::path::PathBuf;
use std::sync::Arc;
use tethera_common::protocol::capability;
use tethera_common::protocol::error::{EntityKind, WireError};
use tethera_common::structs::agent::{Agent, AgentStatus, ClaudeAgent};
use tethera_common::structs::conversation::ConversationFilter;
use tethera_common::structs::ids::{ConversationId, ProfileId};
use tethera_common::structs::terminal::Size;
use tethera_server_lib::backend::TerminalBackend;
use tethera_server_lib::protocol::live::{LiveConversations, LiveTerminals};
use tethera_server_lib::protocol::ports::ConversationPort;
use tethera_common::traits::AgentTrait;
use tethera_server_lib::terminal::{PaneRegistry, PtyBackend};
use tethera_server_lib::transcript::AssetIndex;

/// A machine whose agent projects tree holds exactly what a test put there.
struct Machine {
    home: tempfile::TempDir,
}

impl Machine {
    fn with(sessions: &[(&str, &str)]) -> Self {
        let home = tempfile::tempdir().expect("a temporary home");
        let projects = home.path().join(".claude").join("projects");

        for (project, fixture) in sessions {
            let directory = projects.join(project);
            std::fs::create_dir_all(&directory).expect("a project directory");

            std::fs::copy(Self::fixture(fixture), directory.join(fixture))
                .expect("a session file");
        }

        Self { home }
    }

    fn empty() -> Self {
        Self {
            home: tempfile::tempdir().expect("a temporary home"),
        }
    }

    /// One session naming a working directory that really exists.
    ///
    /// The committed fixtures all name `/home/dev/...`, which is deliberate and
    /// is what makes them useless for anything that asks whether a directory is
    /// still there. This writes the one line a summary reads.
    fn record(&self, session: &str, cwd: &str) {
        let directory = self
            .home
            .path()
            .join(".claude")
            .join("projects")
            .join(ClaudeAgent::project_directory(cwd));

        std::fs::create_dir_all(&directory).expect("a project directory");

        let line = serde_json::json!({
            "type": "user",
            "uuid": session,
            "sessionId": session,
            "cwd": cwd,
            "timestamp": "2026-01-01T10:00:01.000Z",
            "promptSource": "typed",
            "message": { "role": "user", "content": "hello" },
        });

        std::fs::write(
            directory.join(format!("{session}.jsonl")),
            format!("{line}\n"),
        )
        .expect("a session file");
    }

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("transcripts")
            .join(name)
    }

    /// A conversation port over this machine, with a terminal backend that
    /// resolves to nothing - so no pane binds, and every conversation is
    /// history.
    fn conversations(&self) -> LiveConversations {
        LiveConversations::at(
            self.terminals(),
            self.home.path(),
            AssetIndex::new_shared(),
            self.home.path().join("uploads"),
        )
    }

    fn terminals(&self) -> Arc<LiveTerminals> {
        let panes = PaneRegistry::new_shared();
        let backend = Arc::new(TerminalBackend::herdr(
            "a-binary-nothing-resolves".to_string(),
            Size { cols: 80, rows: 24 },
        ));

        LiveTerminals::new_shared(backend, panes)
    }

    /// The same machine over a terminal backend that really answers, with no
    /// panes open in it.
    ///
    /// The difference is not cosmetic. "This machine cannot reach its terminal
    /// backend" and "this conversation has no pane" are different answers, and
    /// only a backend that answers can produce the second.
    fn conversations_with_a_live_backend(&self) -> LiveConversations {
        let panes = PaneRegistry::new_shared();
        let backend = Arc::new(TerminalBackend::pty(
            panes.clone(),
            Size { cols: 80, rows: 24 },
            PtyBackend::default_shell(),
        ));

        LiveConversations::at(
            LiveTerminals::new_shared(backend, panes),
            self.home.path(),
            AssetIndex::new_shared(),
            self.home.path().join("uploads"),
        )
    }
}

// A machine that has never run the harness genuinely has no conversations, so an
// empty page is true rather than evasive. That is what makes advertising
// `transcript_paging` honest despite `list` having no error path.
#[tokio::test]
async fn a_machine_that_has_never_run_the_harness_lists_nothing() {
    let machine = Machine::empty();
    let listed = machine
        .conversations()
        .list(ConversationFilter::All, None, 10)
        .await;

    assert!(listed.items.is_empty());
    assert!(!listed.has_earlier);
    assert!(listed.next_before.is_none());
}

#[tokio::test]
async fn every_session_on_disk_is_listed_as_a_conversation() {
    let machine = Machine::with(&[
        ("-home-dev-one", "plain.jsonl"),
        ("-home-dev-two", "tools.jsonl"),
    ]);

    let listed = machine
        .conversations()
        .list(ConversationFilter::All, None, 10)
        .await;

    assert_eq!(listed.items.len(), 2);
    assert!(listed.items.iter().all(|item| item.has_transcript));
}

// The cross-task contract with the herdr backend, which mints the same value
// from a pane's reported `agent_session`. If the two ever disagree the tree's
// `conversation` points at nothing.
#[tokio::test]
async fn a_conversations_id_is_its_session_id_behind_the_prefix() {
    let machine = Machine::with(&[("-home-dev-one", "plain.jsonl")]);
    let listed = machine
        .conversations()
        .list(ConversationFilter::All, None, 10)
        .await;

    assert_eq!(listed.items[0].id, ConversationId::mint("plain"));
}

// History with nothing running. The client's cue to offer a resume, which starts
// a process and must never happen implicitly.
#[tokio::test]
async fn a_session_no_pane_is_running_is_unbound_and_done() {
    let machine = Machine::with(&[("-home-dev-one", "plain.jsonl")]);
    let listed = machine
        .conversations()
        .list(ConversationFilter::All, None, 10)
        .await;

    assert!(listed.items[0].binding.is_none());
    assert_eq!(listed.items[0].status, AgentStatus::Done);
}

#[tokio::test]
async fn a_live_filter_over_a_machine_with_no_panes_is_empty() {
    let machine = Machine::with(&[("-home-dev-one", "plain.jsonl")]);
    let listed = machine
        .conversations()
        .list(ConversationFilter::Live, None, 10)
        .await;

    assert!(listed.items.is_empty());
}

#[tokio::test]
async fn a_conversation_whose_records_are_not_on_disk_is_not_found() {
    let machine = Machine::with(&[("-home-dev-one", "plain.jsonl")]);
    let conversations = machine.conversations();
    let absent = ConversationId::mint("never-written");

    assert!(matches!(
        conversations.get(&absent).await,
        Err(WireError::NotFound {
            kind: EntityKind::Conversation
        })
    ));

    assert!(matches!(
        conversations.transcript(&absent, None, 10).await,
        Err(WireError::NotFound {
            kind: EntityKind::Conversation
        })
    ));

    assert!(matches!(
        conversations.subscribe(&absent, None).await,
        Err(WireError::NotFound {
            kind: EntityKind::Conversation
        })
    ));
}

#[tokio::test]
async fn a_transcript_reads_through_the_port_the_same_way_it_reads_directly() {
    let machine = Machine::with(&[("-home-dev-one", "grouped.jsonl")]);
    let page = machine
        .conversations()
        .transcript(&ConversationId::mint("grouped"), None, 10)
        .await
        .expect("a page");

    assert_eq!(page.items.len(), 3);
    assert!(!page.has_earlier);
}

// Without the replay a reconnecting client is told there is no gap - `from`
// equals what it sent - and then never receives what it was missing.
#[tokio::test]
async fn resuming_a_watch_replays_the_turns_the_client_has_not_seen() {
    let machine = Machine::with(&[("-home-dev-one", "paging.jsonl")]);
    let conversations = machine.conversations();
    let id = ConversationId::mint("paging");

    let page = conversations
        .transcript(&id, None, 5)
        .await
        .expect("a page");
    let held = page.items[0].cursor.clone();

    let (from, mut events) = conversations
        .subscribe(&id, Some(held.clone()))
        .await
        .expect("a subscription");

    assert_eq!(from, held, "a cursor the source holds is honoured exactly");

    let mut replayed = Vec::new();

    while let Ok(event) = events.try_recv() {
        if let tethera_common::protocol::watch::WatchEvent::Turn(turn) = event {
            replayed.push(turn);
        }
    }

    assert_eq!(replayed.len(), 4);
    assert_eq!(replayed[3].cursor, page.items[4].cursor);
}

// The client asked for the tail and already holds the page it just fetched.
#[tokio::test]
async fn opening_a_watch_with_no_cursor_replays_nothing() {
    let machine = Machine::with(&[("-home-dev-one", "paging.jsonl")]);
    let conversations = machine.conversations();

    let (_, mut events) = conversations
        .subscribe(&ConversationId::mint("paging"), None)
        .await
        .expect("a subscription");

    assert!(events.try_recv().is_err());
}

// `watch.rs` subscribes again on a lagged receiver, wanting only the cursor.
// Without reuse every lag would leak a task reading a file for the life of the
// process.
#[tokio::test]
async fn two_watches_on_one_conversation_share_one_poller() {
    let machine = Machine::with(&[("-home-dev-one", "paging.jsonl")]);
    let conversations = machine.conversations();
    let id = ConversationId::mint("paging");

    let (_first, _one) = conversations.subscribe(&id, None).await.expect("a watch");
    let (_second, _two) = conversations.subscribe(&id, None).await.expect("a watch");

    assert_eq!(conversations.watching(), 1);
}

// A subscriber's replay must not reach the other watchers of the same
// conversation, which would arrive there as duplicates of history they have.
#[tokio::test]
async fn one_clients_replay_does_not_reach_another_client() {
    let machine = Machine::with(&[("-home-dev-one", "paging.jsonl")]);
    let conversations = machine.conversations();
    let id = ConversationId::mint("paging");

    let (_, mut quiet) = conversations.subscribe(&id, None).await.expect("a watch");

    let page = conversations.transcript(&id, None, 5).await.expect("a page");
    let (_, _loud) = conversations
        .subscribe(&id, Some(page.items[0].cursor.clone()))
        .await
        .expect("a watch");

    assert!(
        quiet.try_recv().is_err(),
        "a replay reached a subscriber that did not ask for one"
    );
}

// An advertised capability that refuses renders as a control a person taps and
// watches fail.
#[tokio::test]
async fn nothing_that_needs_to_drive_a_pane_is_advertised() {
    let advertised = LiveConversations::capabilities();

    assert!(advertised.contains(&capability::TRANSCRIPT_PAGING.into()));

    for absent in [
        capability::QUESTIONS,
        capability::CONVERSATION_START,
        capability::CONVERSATION_RESUME,
        capability::PROMPT_SEND,
        capability::INTERRUPT,
    ] {
        assert!(
            !advertised.contains(&absent.into()),
            "{absent} is advertised and refuses"
        );
    }
}

// The pre-start counterpart of `has_transcript`: a person should be told before
// they commit, not after.
#[tokio::test]
async fn a_preview_says_whether_the_profile_it_names_will_have_a_transcript() {
    let machine = Machine::empty();
    let conversations = machine.conversations();

    let readable = conversations
        .preview(
            &tethera_common::structs::ids::ProfileId("claude".into()),
            "/home/dev/project",
            None,
        )
        .await
        .expect("a preview");

    assert!(readable.will_have_transcript);
    assert!(readable.creates_workspace);
    assert_eq!(readable.workspace_label, "project");

    let unreadable = conversations
        .preview(
            &tethera_common::structs::ids::ProfileId("codex".into()),
            "/home/dev/project",
            None,
        )
        .await
        .expect("a preview");

    assert!(
        !unreadable.will_have_transcript,
        "a harness nobody has measured must not promise a conversation surface"
    );
}

/// The profile every machine in this suite can follow.
fn claude() -> ProfileId {
    Agent::Claude.profile().id
}

// Each refusal below has to happen before a pane exists, and this suite can
// prove it: the machine's terminal backend resolves to nothing, so anything that
// reached the backend would come back as `Backend`. A `NotFound` or an
// `Unsupported` here is therefore evidence that the value was stopped at the
// boundary rather than carried into a command line.

#[tokio::test]
async fn a_profile_no_catalog_row_names_cannot_start_anything() {
    let machine = Machine::empty();
    let started = machine
        .conversations()
        .start(&ProfileId("gpt-9".to_string()), "/", None, &[])
        .await;

    assert!(matches!(
        started,
        Err(WireError::NotFound {
            kind: EntityKind::Conversation
        })
    ));
}

// A machine can run an agent whose records it cannot read. What it cannot do is
// hand back a conversation for one, and this call returns a conversation.
#[tokio::test]
async fn a_profile_this_machine_cannot_follow_is_refused_by_name() {
    let machine = Machine::empty();
    let started = machine
        .conversations()
        .start(&Agent::Codex.profile().id, "/", None, &[])
        .await;

    let Err(WireError::Backend { message }) = started else {
        panic!("a profile with no readable records must be refused");
    };

    assert!(
        message.contains(&Agent::Codex.profile().label),
        "the refusal has to name the profile it refused: {message}"
    );
}

// A working directory arrives from a phone. A relative one would resolve against
// whatever directory the server happens to be running in, which is nobody's
// choice, and one that is not there would fail inside the backend with a message
// about herdr.
#[tokio::test]
async fn a_working_directory_is_refused_unless_it_is_a_directory_here() {
    let machine = Machine::empty();
    let file = machine.home.path().join("not-a-directory");
    std::fs::write(&file, b"x").expect("a file");

    for refused in [
        "",
        "   ",
        "projects/tethera",
        "../../etc",
        &machine.home.path().join("no-such-child").to_string_lossy(),
        &file.to_string_lossy(),
    ] {
        assert!(
            matches!(
                machine
                    .conversations()
                    .start(&claude(), refused, None, &[])
                    .await,
                Err(WireError::Backend { .. })
            ),
            "started in {refused:?}, which is not a directory on this machine"
        );
    }
}

#[tokio::test]
async fn a_machine_that_has_never_run_the_harness_suggests_no_directory() {
    assert!(Machine::empty().conversations().recent_cwds(10).await.is_empty());
}

// `start` refuses a directory that is not there, so suggesting one would be
// offering a choice the next call rejects. Every fixture session names
// `/home/dev/...`, which exists on no machine running this.
#[tokio::test]
async fn a_directory_that_has_since_gone_is_not_suggested() {
    let machine = Machine::with(&[
        ("-home-dev-one", "plain.jsonl"),
        ("-home-dev-two", "tools.jsonl"),
    ]);

    assert!(machine.conversations().recent_cwds(10).await.is_empty());
}

#[tokio::test]
async fn a_directory_is_suggested_once_however_many_sessions_ran_in_it() {
    let worked_in = tempfile::tempdir().expect("a working directory");
    let machine = Machine::empty();

    for session in ["aaaa", "bbbb", "cccc"] {
        machine.record(session, &worked_in.path().to_string_lossy());
    }

    assert_eq!(
        machine.conversations().recent_cwds(10).await,
        vec![worked_in.path().to_string_lossy().into_owned()]
    );
}

// A session id outlives its pane, which is what makes every conversation in a
// machine's history resumable. What must not happen is a second agent appending
// to records an agent is already writing: two of them interleaved would corrupt
// the history this whole surface reads.
//
// Unreachable here without a live harness, so what this pins is the half that
// can be: a conversation nobody has records for is not resumable at all.
#[tokio::test]
async fn a_conversation_with_no_records_cannot_be_resumed() {
    let machine = Machine::with(&[("-home-dev-one", "plain.jsonl")]);

    assert!(matches!(
        machine
            .conversations()
            .resume(&ConversationId::mint("never-written"), None)
            .await,
        Err(WireError::NotFound {
            kind: EntityKind::Conversation
        })
    ));
}

// The directory a session was recorded in can be gone by the time somebody picks
// it up - a deleted worktree, an unplugged drive. Refusing names it, where
// opening a pane somewhere else would silently resume into a directory whose
// harness has never heard of that session.
#[tokio::test]
async fn resuming_into_a_directory_that_has_gone_is_refused() {
    let machine = Machine::with(&[("-home-dev-one", "plain.jsonl")]);

    let refused = machine
        .conversations()
        .resume(&ConversationId::mint("plain"), None)
        .await;

    let Err(WireError::Backend { message }) = refused else {
        panic!("a resume into a directory that is not there must be refused");
    };

    assert!(
        message.contains("/home/dev/project"),
        "the refusal has to name the directory it could not use: {message}"
    );
}

// A conversation with no pane cannot be typed at, and that is not the same as a
// conversation that is gone: its records are here and it can be picked up again.
// A client that could not tell them apart would offer "gone" where it should
// offer "resume".
#[tokio::test]
async fn a_prompt_for_a_conversation_nothing_is_running_says_so() {
    let machine = Machine::with(&[("-home-dev-one", "plain.jsonl")]);
    let id = ConversationId::mint("plain");

    let refused = machine
        .conversations_with_a_live_backend()
        .send_prompt(&id, "carry on", &[])
        .await;

    assert!(
        matches!(refused, Err(WireError::NotRunning { conversation }) if conversation == id),
        "expected NotRunning naming the conversation"
    );
}

// Both refusals land before anything is typed anywhere, which is why they hold
// on a machine whose terminal backend resolves to nothing.
#[tokio::test]
async fn a_prompt_with_nothing_in_it_is_not_sent() {
    let machine = Machine::with(&[("-home-dev-one", "plain.jsonl")]);
    let conversations = machine.conversations();
    let id = ConversationId::mint("plain");

    for blank in ["", "   ", "\n\t "] {
        assert!(
            matches!(
                conversations.send_prompt(&id, blank, &[]).await,
                Err(WireError::Backend { .. })
            ),
            "sent a prompt made only of {blank:?}"
        );
    }
}

#[tokio::test]
async fn no_directory_is_suggested_when_none_was_asked_for() {
    let worked_in = tempfile::tempdir().expect("a working directory");
    let machine = Machine::empty();
    machine.record("aaaa", &worked_in.path().to_string_lossy());

    assert!(machine.conversations().recent_cwds(0).await.is_empty());
}
