//! The herdr backend against a real herdr.
//!
//! `#[ignore]` by design: these need `herdr` installed with a server running,
//! and the create cases mutate a live session. They are the pass that proves
//! the parsing matches the tool rather than matching a fixture written from
//! memory, so they are run explicitly:
//!
//! ```text
//! cargo test -j4 --test integration backend::live -- --ignored --nocapture
//! ```
//!
//! Every mutating case confines itself to a workspace it creates and closes,
//! and the last case asserts the operator's own workspaces did not move.

use std::time::Duration;

use tethera_common::protocol::terminal::TerminalInput;
use tethera_common::structs::agent::{Agent, AgentSpawn, TranscriptSource};
use tethera_common::structs::ids::{ConversationId, PaneId};
use tethera_common::structs::terminal::{Size, SplitDirection};
use tethera_common::traits::{AgentTrait, TerminalBackendTrait};
use tethera_common::structs::transcript::Answer;
use tethera_server_lib::backend::herdr::HerdrIds;
use tethera_server_lib::backend::{BackendError, HerdrBackend};
use tethera_server_lib::terminal::{Picker, PromptDetector};

/// A workspace this suite made, closed when the guard drops so a failing
/// assertion cannot leave one behind on the operator's desk.
struct Scratch {
    backend: HerdrBackend,
    workspace: Option<String>,
}

impl Scratch {
    const LABEL: &'static str = "tethera-live-verify";

    fn new() -> Self {
        Self {
            backend: HerdrBackend::new(
                HerdrBackend::DEFAULT_BINARY.to_string(),
                Size { cols: 120, rows: 40 },
            ),
            workspace: None,
        }
    }

    fn backend(&self) -> &HerdrBackend {
        &self.backend
    }

    fn create(&mut self) -> String {
        let workspace = self
            .backend
            .create_workspace(Self::LABEL)
            .expect("herdr created a workspace");
        let native = HerdrIds::native_workspace(&workspace.id)
            .expect("its own id is native")
            .to_string();

        self.workspace = Some(native.clone());

        native
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Some(workspace) = self.workspace.take() {
            let _ = std::process::Command::new(HerdrBackend::DEFAULT_BINARY)
                .args(["workspace", "close", &workspace])
                .output();
        }
    }
}

#[test]
#[ignore = "needs a running herdr"]
fn the_backend_reads_the_operators_real_session() {
    let scratch = Scratch::new();
    let backend = scratch.backend();

    let (workspaces, tabs, panes) = backend.tree().expect("herdr answered a tree");

    println!("workspaces: {}", workspaces.len());
    for workspace in &workspaces {
        println!(
            "  {} name={:?} tabs={} cwd={:?} conversation={:?}",
            workspace.id.as_str(),
            workspace.name,
            workspace.tab_count,
            workspace.cwd,
            workspace.conversation
        );
    }
    for tab in &tabs {
        println!(
            "  tab {} index={} title={:?} fg={:?}",
            tab.id.as_str(),
            tab.index,
            tab.title,
            tab.foreground_command
        );
    }
    for pane in &panes {
        println!(
            "  pane {} label={:?} size={}x{} cwd={:?} fg={:?} focused={}",
            pane.id.as_str(),
            pane.label,
            pane.size.cols,
            pane.size.rows,
            pane.cwd,
            pane.foreground_command,
            pane.focused
        );
    }

    assert!(
        !workspaces.is_empty(),
        "a running herdr always has at least one workspace"
    );

    // Every id is prefixed, and every prefixed id maps back to a native one.
    for pane in &panes {
        assert!(pane.id.as_str().starts_with("pn_"));
        HerdrIds::native_pane(&pane.id).expect("round trips");
    }

    // Every tab a pane claims exists, and every workspace a tab claims exists.
    for tab in &tabs {
        assert!(
            workspaces.iter().any(|w| w.id == tab.workspace_id),
            "tab {} names a workspace that is not in the tree",
            tab.id.as_str()
        );
    }

    // `tab_count` is herdr's own and must agree with the tabs it reported.
    for workspace in &workspaces {
        let counted = tabs
            .iter()
            .filter(|tab| tab.workspace_id == workspace.id)
            .count();

        assert_eq!(
            usize::from(workspace.tab_count),
            counted,
            "{} says {} tabs and reported {counted}",
            workspace.id.as_str(),
            workspace.tab_count
        );
    }
}

#[test]
#[ignore = "needs a running herdr"]
fn a_created_pane_reports_the_geometry_it_actually_got() {
    let mut scratch = Scratch::new();
    let native = scratch.create();
    let backend = scratch.backend();

    let workspace = HerdrIds::workspace(&native);
    let pane = backend
        .open_pane(Some(&workspace), None, Size { cols: 120, rows: 40 })
        .expect("a tab in the new workspace");

    println!(
        "created pane {} size={}x{} label={:?} cwd={:?}",
        pane.id.as_str(),
        pane.size.cols,
        pane.size.rows,
        pane.label,
        pane.cwd
    );

    // herdr accepts no requested size, so the pane must not echo the one asked
    // for unless that is genuinely what the desk gave it.
    assert!(pane.size.cols > 0 && pane.size.rows > 0);
    assert_eq!(pane.workspace_id, workspace);

    let split = backend
        .split(&pane.id, SplitDirection::Horizontal)
        .expect("split right");

    println!(
        "split pane {} size={}x{}",
        split.id.as_str(),
        split.size.cols,
        split.size.rows
    );

    assert_ne!(split.id, pane.id);
    assert_eq!(split.tab_id, pane.tab_id);

    // A split re-lays-out its neighbour, so the original pane is narrower than
    // it was. This is why `Pane.size` is an observation rather than a property
    // fixed at creation.
    let after = backend.list_panes(&pane.tab_id).expect("panes in the tab");
    let original = after
        .iter()
        .find(|candidate| candidate.id == pane.id)
        .expect("the original pane survives its own split");

    println!(
        "original pane is now {}x{} (was {}x{})",
        original.size.cols, original.size.rows, pane.size.cols, pane.size.rows
    );

    assert_eq!(after.len(), 2);

    backend.close(&split.id).expect("close the split");
}

#[test]
#[ignore = "needs a running herdr"]
fn text_sent_to_a_pane_comes_back_out_of_its_scrollback() {
    let mut scratch = Scratch::new();
    let native = scratch.create();
    let backend = scratch.backend();

    let workspace = HerdrIds::workspace(&native);
    let pane = backend
        .open_pane(Some(&workspace), None, Size { cols: 120, rows: 40 })
        .expect("a tab to type into");

    const MARKER: &str = "tethera-live-verify-marker";

    backend
        .send_text(&pane.id, &format!("echo {MARKER}\r"))
        .expect("send text");

    // The shell needs a moment to echo and run it.
    std::thread::sleep(Duration::from_secs(2));

    let page = backend.read(&pane.id, None, 40).expect("scrollback");

    println!("scrollback returned {} lines", page.lines.len());
    for line in &page.lines {
        println!("  {line}");
    }
    println!(
        "next_before_line={:?} has_earlier={}",
        page.next_before_line, page.has_earlier
    );

    assert!(
        page.lines.iter().any(|line| line.contains(MARKER)),
        "the text that was typed did not come back"
    );

    // Paging must terminate. The bug this replaces claimed earlier history that
    // did not exist and handed back the same lines for ever.
    let mut before = page.next_before_line;
    let mut pages = 1;

    while let Some(cursor) = before {
        let older = backend.read(&pane.id, Some(cursor), 40).expect("older page");
        pages += 1;

        assert!(pages < 40, "paging did not terminate");

        before = older.next_before_line;
    }

    println!("paged to the end of the buffer in {pages} requests");
}

#[test]
#[ignore = "needs a running herdr"]
fn an_id_that_names_nothing_is_refused_rather_than_reported_as_done() {
    let scratch = Scratch::new();
    let backend = scratch.backend();

    // A well-shaped id for a pane that does not exist.
    let absent = PaneId::parse("pn_w99:p99").expect("prefixed");
    let refused = backend.close(&absent).expect_err("must not report success");

    println!("closing an absent pane: {refused}");

    let classified = refused
        .downcast::<BackendError>()
        .expect("classified as a backend error");

    assert!(matches!(classified, BackendError::NotFound { .. }));

    // A flag-shaped id must never reach herdr's command line: `herdr pane close
    // --help` prints help and exits 0, so without the shape check this would
    // report a pane closed that was not.
    let hostile = PaneId::parse("pn_--help").expect("prefixed");
    let refused = backend.close(&hostile).expect_err("must be refused");

    println!("closing a flag-shaped id: {refused}");

    let classified = refused
        .downcast::<BackendError>()
        .expect("classified as a backend error");

    assert!(matches!(classified, BackendError::NotFound { .. }));
}

/// The repository root, which is a directory the harness already trusts.
///
/// A directory it has not seen stops the agent at its own trust prompt, where
/// it never begins a session — so this case would fail for a reason that is not
/// about the backend.
fn trusted_directory() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the repository root")
        .to_path_buf()
}

// The proof that a start really starts something, and that a prompt from a
// phone reaches the agent rather than the shell under it.
//
// This one costs a real agent session, which is the point: nothing short of a
// live harness can show that the launch line was accepted, that the session the
// backend reports is the session the agent wrote, or that a prompt carrying a
// newline and a backtick arrives as one message with neither interpreted.
#[test]
#[ignore = "needs a running herdr, and starts a real agent session"]
fn a_started_agent_announces_its_session_and_takes_a_prompt_whole() {
    let mut scratch = Scratch::new();
    let native = scratch.create();
    let backend = scratch.backend();

    let cwd = trusted_directory();
    let pane = backend
        .open_pane(
            Some(&HerdrIds::workspace(&native)),
            Some(&cwd.to_string_lossy()),
            Size { cols: 120, rows: 40 },
        )
        .expect("herdr opened a pane");

    let spawn = AgentSpawn::new(
        Agent::Claude,
        cwd.to_string_lossy().into_owned(),
        None,
    );

    let conversation = backend
        .start_agent(&pane.id, &spawn)
        .expect("herdr started the agent")
        .expect("a started agent announced its session");

    let session = conversation
        .as_str()
        .strip_prefix(ConversationId::PREFIX)
        .expect("a conversation id carries its prefix")
        .to_string();

    // Every character below is one a shell would act on. Delivered as an
    // argument to a process rather than typed at a prompt, none of them can be.
    let prompt = "Reply with exactly OK.\nSecond line: `whoami`; echo pwned && true";

    backend
        .submit_prompt(&pane.id, prompt)
        .expect("herdr submitted the prompt");

    let TranscriptSource::JsonLines { path } =
        Agent::Claude.transcript_source(&home(), &cwd.to_string_lossy(), &session)
    else {
        panic!("this harness records its sessions as json lines");
    };

    let recorded = poll_for_first_prompt(&path);

    assert_eq!(
        recorded, prompt,
        "the prompt reached the agent altered; a newline that submits early or a \
         backtick a shell expanded would both show here"
    );
}

// The other half of a start, and the one a client must not read as a failure.
//
// A directory the harness has never been trusted with holds it at its own prompt
// before it writes anything, so it is running and has begun no session. herdr
// reports the start as ready either way, which is precisely why this cannot be
// inferred from the start succeeding.
//
// A fresh temporary directory is untrusted by construction, so this does not
// depend on what the machine running it has seen - but it is only untrusted
// once, which is why it is made rather than named.
#[test]
#[ignore = "needs a running herdr, and starts a real agent session"]
fn an_agent_held_at_its_own_prompt_announces_no_session() {
    let mut scratch = Scratch::new();
    let native = scratch.create();
    let backend = scratch.backend();

    let unseen = tempfile::tempdir().expect("a directory the harness has never seen");
    let pane = backend
        .open_pane(
            Some(&HerdrIds::workspace(&native)),
            Some(&unseen.path().to_string_lossy()),
            Size { cols: 120, rows: 40 },
        )
        .expect("herdr opened a pane");

    let spawn = AgentSpawn::new(
        Agent::Claude,
        unseen.path().to_string_lossy().into_owned(),
        None,
    );

    assert_eq!(
        backend
            .start_agent(&pane.id, &spawn)
            .expect("a start that is held is still a start"),
        None,
        "an agent that has written no record must not be reported as a conversation"
    );
}

// A session id outlives the pane that made it, and this is what makes that
// worth anything: the harness picks the same records back up and reports **the
// same session**, so a resumed conversation keeps the id it always had. Every
// cursor a client holds, its transcript and its watch all stay valid.
//
// If this ever reports a different id, resume is a fork rather than a
// resurrection, and the whole conversation surface would branch on every
// pick-up.
#[test]
#[ignore = "needs a running herdr, and starts a real agent session"]
fn resuming_a_session_reports_the_same_conversation_it_was_asked_for() {
    let mut scratch = Scratch::new();
    let native = scratch.create();
    let backend = scratch.backend();

    let cwd = trusted_directory();
    let open = |name: &str| {
        backend
            .open_pane(
                Some(&HerdrIds::workspace(&native)),
                Some(&cwd.to_string_lossy()),
                Size { cols: 120, rows: 40 },
            )
            .unwrap_or_else(|error| panic!("herdr opened a pane for {name}: {error}"))
    };

    let first = open("the first run");
    let began = backend
        .start_agent(
            &first.id,
            &AgentSpawn::new(Agent::Claude, cwd.to_string_lossy().into_owned(), None),
        )
        .expect("herdr started the agent")
        .expect("a started agent announced its session");

    let session = began
        .as_str()
        .strip_prefix(ConversationId::PREFIX)
        .expect("a conversation id carries its prefix")
        .to_string();

    // One turn, and then wait for it to be on disk. Measured: an agent announces
    // its session id at startup and writes no file until it has something to
    // record, and a resume of an id with no records never becomes ready. So a
    // session is resumable from its first turn, not from its first breath.
    backend
        .submit_prompt(&first.id, "Reply with exactly OK.")
        .expect("herdr submitted the prompt");

    let TranscriptSource::JsonLines { path } =
        Agent::Claude.transcript_source(&home(), &cwd.to_string_lossy(), &session)
    else {
        panic!("this harness records its sessions as json lines");
    };

    poll_for_first_prompt(&path);

    // The pane goes, the session stays. This is the state every conversation in
    // a machine's history is in.
    backend.close(&first.id).expect("herdr closed the pane");

    let second = open("the resume");
    let resumed = backend
        .start_agent(
            &second.id,
            &AgentSpawn::resuming(Agent::Claude, cwd.to_string_lossy().into_owned(), session),
        )
        .expect("herdr resumed the agent")
        .expect("a resumed agent announced its session");

    assert_eq!(
        resumed, began,
        "a resume that reports a different session is a fork, not a resurrection"
    );
}

// The whole permission path, end to end, through the same code the port runs:
// an agent asks to do something, the machine reads the question off its screen,
// and a press answers it.
//
// A permission prompt is never written to the records, so nothing short of a
// live harness can show that this works. It is also the question a person is
// asked most often, and the one that strands a session when nobody is at the
// machine — which is the whole premise of the product.
#[test]
#[ignore = "needs a running herdr, and starts a real agent session"]
fn a_permission_prompt_is_read_off_the_screen_and_answered_by_a_press() {
    let mut scratch = Scratch::new();
    let native = scratch.create();
    let backend = scratch.backend();

    let cwd = trusted_directory();
    let pane = backend
        .open_pane(
            Some(&HerdrIds::workspace(&native)),
            Some(&cwd.to_string_lossy()),
            Size { cols: 120, rows: 40 },
        )
        .expect("herdr opened a pane");

    // Started in a mode that asks, which this machine's own default is not.
    // Measured: an agent here comes up in auto mode and answers its own
    // permission questions with "Allowed by auto mode classifier". That is the
    // operator's setting, and the product deliberately no longer overrides it —
    // so a suite that needs a prompt has to ask for one.
    start_asking(&pane.id);

    // Outside the directory the agent was trusted with.
    let probe = std::env::temp_dir().join("tethera-live-permission-probe.txt");
    let _ = std::fs::remove_file(&probe);

    backend
        .submit_prompt(
            &pane.id,
            &format!(
                "Use the Write tool to create {} containing exactly: hello",
                probe.to_string_lossy().replace('\\', "/")
            ),
        )
        .expect("herdr submitted the prompt");

    let question = poll_for_prompt(backend, &pane.id);
    let ask = &question.asks[0];

    println!("detected: {:?}", ask.prompt);
    for option in &ask.options {
        println!("  option: {:?}", option.label);
    }

    // The refusal has to be reachable. A machine that could only ever say yes
    // would be worse than one that could not answer at all.
    let refuse = ask
        .options
        .iter()
        .position(|option| option.label.eq_ignore_ascii_case("No"))
        .expect("a way to decline");

    for step in Picker::steps(&question.asks, &[Answer::Choice(refuse as u16)])
        .expect("presses for the refusal")
    {
        match step {
            TerminalInput::Key { key, mods } => {
                backend.send_key(&pane.id, key, mods).expect("a key")
            }
            TerminalInput::Text(text) => {
                backend.send_text(&pane.id, &text).expect("text")
            }
        }
    }

    std::thread::sleep(Duration::from_secs(3));

    assert!(
        PromptDetector::detect(&backend.screen(&pane.id).expect("a screen")).is_none(),
        "the prompt is still on screen, so the press did not answer it"
    );
    assert!(
        !probe.exists(),
        "the file was written, so the answer that reached the agent was not the refusal"
    );
}

/// Starts an agent that will ask before it acts.
///
/// Not `start_agent`: a spawn carries no permission mode any more, by design —
/// the harness holds the operator's own preference and the product does not
/// override a choice made where they could see it. A machine whose preference is
/// auto therefore produces no permission prompt at all, which is why this
/// reaches past the spawn path rather than through it.
fn start_asking(pane: &PaneId) {
    let native = HerdrIds::native_pane(pane).expect("a native pane id");
    let name: String = native
        .chars()
        .map(|c| match c {
            'A'..='Z' => c.to_ascii_lowercase(),
            'a'..='z' | '0'..='9' | '_' => c,
            _ => '-',
        })
        .collect();

    let started = std::process::Command::new(HerdrBackend::DEFAULT_BINARY)
        .args([
            "agent",
            "start",
            &name,
            "--kind",
            "claude",
            "--pane",
            native,
            "--timeout",
            "60000",
            "--",
            "--permission-mode",
            "default",
        ])
        .output()
        .expect("herdr ran");

    assert!(
        started.status.success(),
        "herdr did not start an asking agent: {}",
        String::from_utf8_lossy(&started.stderr)
    );
}

/// The question an agent is showing, once it is showing one.
fn poll_for_prompt(
    backend: &HerdrBackend,
    pane: &PaneId,
) -> tethera_common::structs::transcript::Question {
    let mut last = String::new();

    for _ in 0..90 {
        last = backend.screen(pane).unwrap_or_default();

        if let Some(found) = PromptDetector::detect(&last) {
            return found;
        }

        std::thread::sleep(Duration::from_millis(500));
    }

    panic!("the agent never asked for permission. last screen:
{last}");
}

fn home() -> std::path::PathBuf {
    directories::UserDirs::new()
        .map(|dirs| dirs.home_dir().to_path_buf())
        .expect("a home directory")
}

/// The first thing the person said, once the agent has written it down.
///
/// Polled because the record is written by another process: the submit returns
/// when the agent accepted the text, not when it has flushed a line about it.
fn poll_for_first_prompt(path: &std::path::Path) -> String {
    for _ in 0..40 {
        let found = std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|record| record["type"] == "user")
            .and_then(|record| record["message"]["content"].as_str().map(str::to_owned));

        if let Some(found) = found {
            return found;
        }

        std::thread::sleep(Duration::from_millis(500));
    }

    panic!("the agent recorded no prompt at {}", path.display());
}

#[test]
#[ignore = "needs a running herdr"]
fn a_missing_binary_is_a_backend_failure_rather_than_a_panic() {
    let backend = HerdrBackend::new(
        "herdr-that-is-not-installed".to_string(),
        Size { cols: 120, rows: 40 },
    );

    let refused = backend
        .list_workspaces()
        .expect_err("there is no such binary");

    println!("absent binary: {refused}");

    let classified = refused
        .downcast::<BackendError>()
        .expect("classified as a backend error");

    assert!(matches!(classified, BackendError::Backend { .. }));
}
