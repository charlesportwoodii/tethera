//! The herdr backend's boundary and its mapping.
//!
//! Everything here is a pure function over a string, so none of it needs herdr
//! installed. The fixtures under `tests/fixtures/herdr/` were captured from
//! `herdr 0.8.0-preview.2026-08-04-d78e3d3b5126`, socket API protocol 19, and
//! from `herdr 0.8.2`, protocol 20, on real sessions; the hand-authored ones
//! are named for the hostile case they carry. Paths, labels and session ids in
//! the protocol 20 capture are rewritten.

use std::collections::BTreeMap;

use tethera_common::protocol::error::EntityKind;
use tethera_common::structs::ids::{PaneId, TabId, WorkspaceId};
use tethera_common::structs::terminal::Size;
use tethera_server_lib::backend::herdr::mapping::Foreground;
use tethera_server_lib::backend::herdr::wire::{
    AgentSession, AgentSessionKind, Created, Envelope, PaneBody, ProcessInfoBody, Snapshot,
    SnapshotBody,
};
use tethera_server_lib::backend::herdr::{HerdrIds, Mapping, ScrollbackWindow};
use tethera_server_lib::backend::{BackendError, HerdrBackend};

/// A captured herdr answer, read from disk.
struct Fixture;

impl Fixture {
    const DEFAULT: Size = Size {
        cols: 120,
        rows: 40,
    };

    fn raw(name: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/herdr")
            .join(name);

        std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("fixture {}: {error}", path.display()))
    }

    /// The whole decode path a listing call takes: envelope, then result body,
    /// then the snapshot inside it.
    fn snapshot(name: &str) -> Snapshot {
        Envelope::<SnapshotBody>::decode(&Self::raw(name))
            .expect("envelope")
            .into_result()
            .expect("result")
            .snapshot
    }

    fn empty_foreground() -> Foreground {
        BTreeMap::new()
    }
}

// ---------------------------------------------------------------------------
// The boundary: one test per answer the backend actually decodes.
// ---------------------------------------------------------------------------

#[test]
fn the_snapshot_answer_nests_its_payload_under_a_result_body() {
    // herdr's `result` is a tagged union and `session_snapshot` puts the
    // session under `snapshot`. Decoding `Snapshot` straight out of `result`
    // fails on the first required field, which is what it did.
    let raw = Fixture::raw("snapshot.json");

    // The wrong shape fails while the envelope itself is being read, because
    // `result` is decoded as part of it. Naming the field it missed is the
    // whole value of decoding into a type rather than into a map.
    let shallow = Envelope::<Snapshot>::decode(&raw);

    match shallow {
        Err(BackendError::Backend { message }) => {
            assert!(message.contains("version"), "unexpected message: {message}")
        }
        other => panic!("the snapshot must not decode without its result body: {other:?}"),
    }

    let snapshot = Fixture::snapshot("snapshot.json");

    assert_eq!(snapshot.protocol, 19);
    assert_eq!(snapshot.workspaces.len(), 7);
    assert_eq!(snapshot.tabs.len(), 7);
    assert_eq!(snapshot.panes.len(), 7);
    assert_eq!(snapshot.layouts.len(), 7);
}

#[test]
fn both_protocols_in_the_field_decode_and_are_known() {
    let old = Fixture::snapshot("snapshot.json");
    let new = Fixture::snapshot("snapshot-20.json");

    assert_eq!(old.protocol, 19);
    assert_eq!(new.protocol, 20);
    assert!(old.speaks_known_protocol());
    assert!(new.speaks_known_protocol());

    let ahead = Snapshot {
        protocol: 21,
        ..new
    };

    assert!(!ahead.speaks_known_protocol());
}

#[test]
fn a_protocol_20_session_maps_to_the_tree_it_actually_had() {
    let snapshot = Fixture::snapshot("snapshot-20.json");
    let foreground = Fixture::empty_foreground();

    let workspaces = Mapping::workspaces(&snapshot);
    let ids: Vec<&str> = workspaces.iter().map(|w| w.id.as_str()).collect();

    assert_eq!(ids, vec!["ws_w1M", "ws_w1N"]);

    let panes = Mapping::panes(&snapshot, Some("w1M:t1"), &foreground, Fixture::DEFAULT);

    assert_eq!(panes.len(), 1);
    assert_eq!(panes[0].id.as_str(), "pn_w1M:p1");
    assert_eq!(panes[0].size, Size { cols: 114, rows: 49 });
    assert_eq!(
        panes[0].title.as_deref(),
        Some("Herdr protocol 20 alignment")
    );
}

// The protocol 19 capture has no announced session on any pane, so the bound
// case was only ever read from a hand-authored fixture until this one.
#[test]
fn a_protocol_20_pane_carries_the_session_its_agent_announced() {
    let snapshot = Fixture::snapshot("snapshot-20.json");
    let panes = Mapping::panes(&snapshot, None, &Fixture::empty_foreground(), Fixture::DEFAULT);

    assert!(panes.iter().all(|pane| pane.agent.is_some()));
    assert_eq!(
        panes
            .iter()
            .filter_map(|pane| pane.conversation.as_ref())
            .count(),
        panes.len()
    );
}

#[test]
fn the_process_info_answer_nests_its_payload_under_a_result_body() {
    let raw = Fixture::raw("pane-process-info.json");

    let body = Envelope::<ProcessInfoBody>::decode(&raw)
        .expect("envelope")
        .into_result()
        .expect("result");

    assert_eq!(body.process_info.pane_id, "w67:p1");
    // The executable suffix is dropped, so a tab row reads the same on every
    // platform.
    assert_eq!(body.process_info.command().as_deref(), Some("claude"));
}

#[test]
fn a_workspace_create_answer_carries_the_workspace_the_tab_and_the_root_pane() {
    let created = Envelope::<Created>::decode(&Fixture::raw("workspace-created.json"))
        .expect("envelope")
        .into_result()
        .expect("result");

    let workspace = created.workspace.expect("a workspace create names one");

    assert_eq!(workspace.workspace_id, "w6A");
    assert_eq!(workspace.label, "tethera-verify-02");
    assert_eq!(workspace.tab_count, 1);
    assert_eq!(created.tab.tab_id, "w6A:t1");
    assert_eq!(created.root_pane.pane_id, "w6A:p1");
}

#[test]
fn a_tab_create_answer_names_its_tab_and_no_workspace() {
    let created = Envelope::<Created>::decode(&Fixture::raw("tab-created.json"))
        .expect("envelope")
        .into_result()
        .expect("result");

    assert!(created.workspace.is_none());
    assert_eq!(created.tab.tab_id, "w6A:t2");
    assert_eq!(created.tab.number, 2);
    assert_eq!(created.root_pane.pane_id, "w6A:p2");
}

#[test]
fn a_split_answer_decodes_as_the_pane_it_made() {
    let body = Envelope::<PaneBody>::decode(&Fixture::raw("pane-split.json"))
        .expect("envelope")
        .into_result()
        .expect("result");

    assert_eq!(body.pane.pane_id, "w6A:p5");
    assert_eq!(body.pane.tab_id, "w6A:t1");
}

#[test]
fn each_not_found_code_keeps_the_kind_it_names() {
    let pane = Envelope::<SnapshotBody>::decode(&Fixture::raw("error-pane-not-found.json"))
        .expect("envelope")
        .into_result();

    assert!(matches!(
        pane,
        Err(BackendError::NotFound {
            kind: EntityKind::Pane
        })
    ));

    let workspace =
        Envelope::<SnapshotBody>::decode(&Fixture::raw("error-workspace-not-found.json"))
            .expect("envelope")
            .into_result();

    assert!(matches!(
        workspace,
        Err(BackendError::NotFound {
            kind: EntityKind::Workspace
        })
    ));
}

#[test]
fn an_error_envelope_is_read_as_an_error_even_though_it_carries_no_result() {
    // The error is checked before the missing result, or the code that says why
    // would be replaced by "no result".
    let envelope = Envelope::<SnapshotBody>::decode(&Fixture::raw("error-pane-not-found.json"))
        .expect("an error envelope is still valid json");

    assert!(envelope.result.is_none());
    assert!(envelope.error.is_some());
}

#[test]
fn truncated_output_is_classified_as_a_backend_failure() {
    let decoded = Envelope::<SnapshotBody>::decode(&Fixture::raw("truncated.json"));

    assert!(matches!(decoded, Err(BackendError::Backend { .. })));
}

#[test]
fn an_agent_status_this_backend_has_never_heard_of_still_parses() {
    // Deliberate forward compatibility: `agent_status` is a closed five-value
    // enum in herdr's schema today and nothing here reads it, so a release that
    // adds a status must not fail a whole listing.
    let snapshot = Fixture::snapshot("unknown-agent-status.json");

    assert_eq!(snapshot.workspaces.len(), 1);
    assert_eq!(snapshot.tabs.len(), 1);
    assert_eq!(snapshot.panes.len(), 1);
}

#[test]
fn an_agent_session_kind_this_backend_has_never_heard_of_yields_no_conversation() {
    let snapshot = Fixture::snapshot("unknown-agent-status.json");
    let session = snapshot.panes[0]
        .agent_session
        .as_ref()
        .expect("the fixture carries one");

    assert_eq!(session.kind, AgentSessionKind::Unknown);
    assert!(Mapping::conversation_of(session).is_none());
}

#[test]
fn a_session_with_nothing_open_maps_to_empty_ranks_rather_than_an_error() {
    let snapshot = Fixture::snapshot("empty-session.json");
    let foreground = Fixture::empty_foreground();

    assert!(Mapping::workspaces(&snapshot).is_empty());
    assert!(Mapping::tabs(&snapshot, None, &foreground).is_empty());
    assert!(Mapping::panes(&snapshot, None, &foreground, Fixture::DEFAULT).is_empty());
}

// ---------------------------------------------------------------------------
// The mapping: our own decisions.
// ---------------------------------------------------------------------------

#[test]
fn a_tabs_index_is_herdrs_own_ordinal_and_not_its_position_in_the_list() {
    // Measured against the real binary: creating four tabs and closing the
    // second left the survivors numbered 1, 3, 4. A positional index would
    // have renumbered them, and `2:build` would stop being the tab the person
    // calls `2:build`. The fixture puts number and position deliberately out
    // of step.
    let snapshot = Fixture::snapshot("reordered-tabs.json");
    let tabs = Mapping::tabs(&snapshot, Some("w70"), &Fixture::empty_foreground());

    let indices: Vec<u16> = tabs.iter().map(|tab| tab.index).collect();

    assert_eq!(indices, vec![4, 2, 9]);
    assert_eq!(tabs[0].title, "build");
}

#[test]
fn a_workspace_prefers_its_worktree_checkout_over_a_panes_directory() {
    let snapshot = Fixture::snapshot("reordered-tabs.json");
    let workspaces = Mapping::workspaces(&snapshot);

    assert_eq!(
        workspaces[0].cwd.as_deref(),
        Some("C:\\Users\\charl\\worktrees\\herdr-backend")
    );
    // No worktree, so the active tab's primary pane answers instead.
    assert_eq!(workspaces[1].cwd.as_deref(), Some("C:\\Users\\charl\\scratch"));
}

#[test]
fn a_field_herdr_left_blank_is_absent_rather_than_an_empty_value() {
    let snapshot = Fixture::snapshot("reordered-tabs.json");
    let panes = Mapping::panes(
        &snapshot,
        Some("w70:t4"),
        &Fixture::empty_foreground(),
        Fixture::DEFAULT,
    );

    let blank = panes
        .iter()
        .find(|pane| pane.id.as_str() == "pn_w70:p8")
        .expect("the fixture carries a pane with blank strings");

    // herdr sent `cwd: ""` and `title: "   "`. Neither is a value.
    assert_eq!(blank.cwd, None);
    assert_eq!(blank.title, None);
    // `label` has no honest absence to report, so the pane is named by its id
    // rather than by a blank that would render as an empty row.
    assert_eq!(blank.label, "w70:p8");
}

#[test]
fn a_pane_no_layout_describes_takes_its_size_from_its_tabs_area() {
    let snapshot = Fixture::snapshot("reordered-tabs.json");
    let panes = Mapping::panes(
        &snapshot,
        None,
        &Fixture::empty_foreground(),
        Fixture::DEFAULT,
    );

    let placed = panes
        .iter()
        .find(|pane| pane.id.as_str() == "pn_w70:p9")
        .expect("placed");
    assert_eq!(placed.size, Size { cols: 100, rows: 50 });

    // `w70:t9` has no layout entry at all, and its pane reports no scroll, so
    // the configured default is the only answer left.
    let unplaced = panes
        .iter()
        .find(|pane| pane.id.as_str() == "pn_w70:p7")
        .expect("unplaced");
    assert_eq!(unplaced.size, Fixture::DEFAULT);
}

#[test]
fn a_tab_draws_the_foreground_command_of_its_primary_pane_in_layout_order() {
    let snapshot = Fixture::snapshot("reordered-tabs.json");

    let mut foreground = Foreground::new();
    // `w70:p9` is first in the layout; `w70:p8` is second. Only the second is
    // given a command, so a mapping reading `panes` order would pick it up.
    foreground.insert("w70:p8".to_string(), "wrong".to_string());
    foreground.insert("w70:p9".to_string(), "cargo".to_string());

    let tabs = Mapping::tabs(&snapshot, Some("w70"), &foreground);
    let build = tabs.iter().find(|tab| tab.title == "build").expect("build");

    assert_eq!(build.foreground_command.as_deref(), Some("cargo"));
}

#[test]
fn herdrs_own_agent_answers_for_a_foreground_command_before_a_process_lookup() {
    // `pane process-info` is one subprocess call per pane and the snapshot
    // already names the agent, so a pane running one costs nothing.
    let snapshot = Fixture::snapshot("reordered-tabs.json");
    let mut foreground = Foreground::new();
    foreground.insert("w70:p2".to_string(), "claude.exe".to_string());

    let tabs = Mapping::tabs(&snapshot, Some("w70"), &foreground);
    let agent_tab = tabs.iter().find(|tab| tab.title == "claude").expect("claude");

    assert_eq!(agent_tab.foreground_command.as_deref(), Some("claude"));
}

#[test]
fn a_count_beyond_the_wires_width_saturates_rather_than_wrapping() {
    let snapshot = Fixture::snapshot("huge-counts.json");

    let workspaces = Mapping::workspaces(&snapshot);
    assert_eq!(workspaces[0].tab_count, u16::MAX);

    let tabs = Mapping::tabs(&snapshot, None, &Fixture::empty_foreground());
    assert_eq!(tabs[0].index, u16::MAX);
}

#[test]
fn a_pane_whose_tab_is_gone_is_left_out_rather_than_attached_to_another_tab() {
    let snapshot = Fixture::snapshot("orphan-pane.json");
    let foreground = Fixture::empty_foreground();

    let in_tab = Mapping::panes(&snapshot, Some("w1:t1"), &foreground, Fixture::DEFAULT);
    assert_eq!(in_tab.len(), 1);
    assert_eq!(in_tab[0].id.as_str(), "pn_w1:p1");

    // Unfiltered, it is still reported: it exists, and it is honest about the
    // tab it claims. It simply belongs to no tab a client can list.
    let all = Mapping::panes(&snapshot, None, &foreground, Fixture::DEFAULT);
    assert_eq!(all.len(), 2);
}

#[test]
fn a_conversation_id_comes_from_an_agents_own_session_identity() {
    // The rule the transcript reader has to match, or the tree's `conversation`
    // points at nothing.
    let by_id = AgentSession {
        source: "report".into(),
        agent: "claude".into(),
        kind: AgentSessionKind::Id,
        value: "7b3f9c21-0000-4444-8888-abcdef012345".into(),
    };

    assert_eq!(
        Mapping::conversation_of(&by_id).map(|id| id.as_str().to_owned()),
        Some("cv_7b3f9c21-0000-4444-8888-abcdef012345".to_string())
    );

    // A path reduces to its file stem, which is the session id for every agent
    // that records one.
    let by_path = AgentSession {
        source: "report".into(),
        agent: "claude".into(),
        kind: AgentSessionKind::Path,
        value: "C:\\Users\\charl\\.claude\\projects\\tethera\\9f3c1122.jsonl".into(),
    };

    assert_eq!(
        Mapping::conversation_of(&by_path).map(|id| id.as_str().to_owned()),
        Some("cv_9f3c1122".to_string())
    );

    // The same session, recorded by a herdr on a POSIX machine. The id has to
    // come out identical: `Path::file_stem` splits on the *reader's* separator,
    // so one of these two spellings would otherwise mint an id containing the
    // whole path -- and every `conversation` in the tree would point at nothing
    // on that platform only.
    let posix = AgentSession {
        source: "report".into(),
        agent: "claude".into(),
        kind: AgentSessionKind::Path,
        value: "/home/charl/.claude/projects/tethera/9f3c1122.jsonl".into(),
    };

    assert_eq!(
        Mapping::conversation_of(&posix).map(|id| id.as_str().to_owned()),
        Some("cv_9f3c1122".to_string())
    );
}

#[test]
fn a_pane_whose_agent_never_announced_itself_has_no_conversation() {
    // True of every live Claude Code pane observed: herdr does not discover a
    // session identity, and nothing had called `pane report-agent-session`.
    let snapshot = Fixture::snapshot("snapshot.json");
    let panes = Mapping::panes(
        &snapshot,
        None,
        &Fixture::empty_foreground(),
        Fixture::DEFAULT,
    );

    assert!(panes.iter().all(|pane| pane.conversation.is_none()));
}

#[test]
fn the_real_session_maps_to_the_tree_it_actually_had() {
    let snapshot = Fixture::snapshot("snapshot.json");
    let foreground = Fixture::empty_foreground();

    let workspaces = Mapping::workspaces(&snapshot);
    let ids: Vec<&str> = workspaces.iter().map(|w| w.id.as_str()).collect();

    assert_eq!(
        ids,
        vec!["ws_w62", "ws_w63", "ws_w64", "ws_w65", "ws_w67", "ws_w68", "ws_w69"]
    );
    assert!(workspaces.iter().all(|w| w.tab_count == 1));

    let panes = Mapping::panes(&snapshot, Some("w63:t1"), &foreground, Fixture::DEFAULT);
    assert_eq!(panes.len(), 1);
    assert_eq!(panes[0].id.as_str(), "pn_w63:p1");
    // Observed geometry, not the configured default.
    assert_eq!(panes[0].size, Size { cols: 177, rows: 36 });
    assert_eq!(
        panes[0].title.as_deref(),
        Some("BVC Server Iroh tunnel for HTTPS and WSS")
    );
}

// A live agent and a named session are two different facts, and this capture is
// the proof: every one of these panes is running Claude and **not one of them
// announced a session**. Read through a single nullable `conversation`, a whole
// machine of working agents is indistinguishable from a machine of empty shells
// — which is what made a conversation say nothing was running behind it and
// offer to resume a session that may already have been live.
#[test]
fn a_pane_reports_its_agent_even_when_no_session_was_announced() {
    let snapshot = Fixture::snapshot("snapshot.json");
    let panes = Mapping::panes(&snapshot, None, &Fixture::empty_foreground(), Fixture::DEFAULT);

    let running = panes.iter().filter(|pane| pane.agent.is_some()).count();

    assert!(
        panes.iter().all(|pane| pane.conversation.is_none()),
        "this capture is the unidentified case; a bound pane here would prove nothing"
    );
    assert_eq!(
        running,
        panes.len() - 1,
        "every pane in this capture but the bare shell is running an agent"
    );
    assert!(
        panes.iter().any(|pane| pane.agent.is_none()),
        "the shell is what proves the field is read rather than always set"
    );
}

// herdr spells the agent the way the binary is spelled, and a profile id is the
// binary name — so the two agree by construction rather than by a table somebody
// maintains. Pinned because nothing else would notice them diverging: a name
// this build did not recognise would silently report no agent at all, and every
// consequence of that is a control quietly going missing.
#[test]
fn the_agent_name_herdr_reports_resolves_to_a_profile_this_build_knows() {
    let snapshot = Fixture::snapshot("snapshot.json");
    let panes = Mapping::panes(&snapshot, None, &Fixture::empty_foreground(), Fixture::DEFAULT);

    let named: Vec<&str> = panes
        .iter()
        .filter_map(|pane| pane.agent.as_ref())
        .map(|profile| profile.as_str())
        .collect();

    assert!(named.iter().all(|profile| *profile == "claude"), "{named:?}");
}

// ---------------------------------------------------------------------------
// The argv boundary: what a client sends reaches herdr's command line.
// ---------------------------------------------------------------------------

#[test]
fn an_id_carrying_a_colon_survives_the_round_trip() {
    let pane = HerdrIds::pane("w62:p1");

    assert_eq!(pane.as_str(), "pn_w62:p1");
    assert_eq!(HerdrIds::native_pane(&pane).expect("native"), "w62:p1");
}

#[test]
fn a_workspace_counter_past_the_hex_digits_is_still_a_valid_id() {
    // Every one of these was read off a real running herdr. `w6A` was once the
    // whole of the evidence, and a hex check written from it refused `w6G`
    // onwards - so a long-lived session's newest workspaces stopped resolving,
    // and every pane inside them with it.
    for native in ["w62", "w6A", "w6G", "w6H", "w6J"] {
        assert_eq!(
            HerdrIds::native_workspace(&HerdrIds::workspace(native)).expect("native"),
            native
        );
    }

    assert_eq!(
        HerdrIds::native_pane(&HerdrIds::pane("w6J:p12")).expect("native"),
        "w6J:p12"
    );
}

#[test]
fn an_id_that_is_shaped_like_a_flag_is_refused_before_it_reaches_a_command_line() {
    // herdr's clap honours no `--` separator, and `herdr pane close --help`
    // prints help and exits 0. Without this refusal, closing `pn_--help` would
    // report success having closed nothing.
    for hostile in ["pn_--help", "pn_-v", "pn_--version"] {
        let pane = PaneId::parse(hostile).expect("a prefixed value");

        assert!(
            matches!(
                HerdrIds::native_pane(&pane),
                Err(BackendError::NotFound {
                    kind: EntityKind::Pane
                })
            ),
            "{hostile} must be refused"
        );
    }
}

#[test]
fn an_id_of_the_wrong_rank_is_refused_rather_than_resolving_to_another_entity() {
    // A tab id where a pane was expected. The prefix is part of the value for
    // exactly this reason.
    let tab = TabId::parse("tb_w62:t1").expect("valid");

    assert!(matches!(
        HerdrIds::native_tab(&tab),
        Ok("w62:t1")
    ));

    // A workspace id has no child part, so a pane-shaped suffix is not one.
    let workspace = WorkspaceId::parse("ws_w62:p1").expect("prefixed");

    assert!(matches!(
        HerdrIds::native_workspace(&workspace),
        Err(BackendError::NotFound {
            kind: EntityKind::Workspace
        })
    ));
}

#[test]
fn an_id_with_no_prefix_or_nothing_after_it_is_refused() {
    for hostile in ["w62:p1", "pn_", "pn_w62:t1", "pn_x9", "pn_w:p1"] {
        let refused = match PaneId::parse(hostile) {
            Some(pane) => HerdrIds::native_pane(&pane).is_err(),
            None => true,
        };

        assert!(refused, "{hostile} must not reach herdr");
    }
}

#[test]
fn a_working_directory_shaped_like_a_flag_is_refused() {
    // `--cwd -x` makes clap read `-x` as a flag rather than as the option's
    // argument, so the create would silently land somewhere else.
    assert!(HerdrIds::cwd("-x").is_err());
    assert!(HerdrIds::cwd("").is_err());
    assert!(HerdrIds::cwd("C:\\Users\\charl\\projects\\tethera").is_ok());
}

#[test]
fn a_workspace_name_shaped_like_a_flag_is_refused() {
    assert!(HerdrIds::label("--focus").is_err());
    assert!(HerdrIds::label("   ").is_err());
    assert!(HerdrIds::label("tethera-4").is_ok());
}

// ---------------------------------------------------------------------------
// Scrollback: the page against what the read actually returned.
// ---------------------------------------------------------------------------

#[test]
fn a_buffer_shorter_than_the_page_asked_for_reports_no_earlier_history() {
    // The defect this replaces: a pane reporting a 36-row viewport that holds
    // one line. Planning from `max_offset_from_bottom + viewport_rows` claimed
    // the line was rows 26..36 with 26 older ones behind it, and paging back
    // returned the same line again and again.
    let window = ScrollbackWindow::plan(None, 10);
    let page = window.resolve(vec!["only line"]);

    assert_eq!(page.lines, vec!["only line"]);
    assert!(!page.has_earlier);
    assert_eq!(page.next_before_line, None);
}

#[test]
fn a_full_page_offers_the_cursor_for_the_page_before_it() {
    let window = ScrollbackWindow::plan(None, 10);
    let lines: Vec<String> = (0..10).map(|n| format!("line {n}")).collect();
    let page = window.resolve(lines);

    assert_eq!(page.lines.len(), 10);
    assert!(page.has_earlier);
    assert_eq!(page.next_before_line, Some(10));
}

#[test]
fn a_later_page_returns_the_lines_in_front_of_the_ones_already_shown() {
    // The read is anchored at the bottom, so the ten lines the client has seen
    // are the tail of what came back and the page is what sits before them.
    let window = ScrollbackWindow::plan(Some(10), 10);
    assert_eq!(window.lines_to_request, 20);

    let lines: Vec<String> = (0..20).map(|n| format!("line {n}")).collect();
    let page = window.resolve(lines);

    assert_eq!(page.lines.first().map(String::as_str), Some("line 0"));
    assert_eq!(page.lines.last().map(String::as_str), Some("line 9"));
    assert_eq!(page.lines.len(), 10);
    assert!(page.has_earlier);
    assert_eq!(page.next_before_line, Some(20));
}

#[test]
fn paging_stops_exactly_where_the_buffer_does() {
    // Walks the case measured against the real binary: a pane holding 51
    // lines, paged ten at a time. The last page is one line and offers no
    // cursor, so a client cannot page into a hole.
    const HELD: u32 = 51;
    let mut before = None;
    let mut seen = 0u32;
    let mut pages = 0;

    loop {
        let window = ScrollbackWindow::plan(before, 10);
        let returned = window.lines_to_request.min(HELD);
        let lines: Vec<String> = (0..returned).map(|n| format!("line {n}")).collect();
        let page = window.resolve(lines);

        seen += page.lines.len() as u32;
        pages += 1;

        match page.next_before_line {
            Some(next) => before = Some(next),
            None => break,
        }

        assert!(pages < 20, "paging did not terminate");
    }

    assert_eq!(seen, HELD);
    assert_eq!(pages, 6);
}

#[test]
fn a_page_of_nothing_is_asked_of_herdr_at_all() {
    let window = ScrollbackWindow::plan(None, 0);

    assert_eq!(window.limit, 0);

    let page = window.resolve(Vec::<String>::new());

    assert!(page.lines.is_empty());
    assert!(!page.has_earlier);
    assert_eq!(page.next_before_line, None);
}

#[test]
fn a_request_for_the_whole_buffer_is_capped() {
    // Paging deeper re-reads from the bottom every time, so an unbounded
    // `before_line` would let a client pay for the whole buffer on every page.
    let window = ScrollbackWindow::plan(Some(u32::MAX), 500);

    assert_eq!(window.lines_to_request, ScrollbackWindow::MAX_LINES);
}

// A typed launch must ask herdr to type, not to start.
//
// `agent start` is the supervised route and it inspects the process herdr
// spawned in the pane: measured against herdr 0.8.2, `available_pane_shell`
// requires that process to carry one of fifteen known shell names and, on
// Windows, to have no descendants at all. A pane whose shell is wrapped by the
// shim fails both, and the refusal arrives as `agent_pane_busy` — "agent target
// pane w6H:p1 is not an available shell" — however healthy the shell inside is.
//
// So a wrapped pane has exactly one route left, and this pins which one it is.
// Reaching for `agent start` here costs the caller the whole readiness deadline
// and then fails, which reads as a broken agent rather than a wrong call.
#[test]
fn a_typed_launch_asks_herdr_to_run_the_line_not_to_start_an_agent() {
    let argv = vec!["claude".to_string(), "--permission-mode".to_string()];

    let args = HerdrBackend::typed_launch_args("w6H:p1", &argv).expect("an argv names a binary");

    assert_eq!(
        args,
        vec!["pane", "run", "w6H:p1", "claude", "--permission-mode"],
        "a typed launch must go through `pane run`"
    );
}

// The argv stays split all the way to herdr.
//
// herdr takes the command as trailing arguments, so a flag value carrying a
// space survives. Joined into one string it would be re-split by whatever reads
// it next, at a boundary nobody chose.
#[test]
fn a_typed_launch_keeps_an_argument_that_contains_a_space_whole() {
    let argv = vec![
        "claude".to_string(),
        "--append-system-prompt".to_string(),
        "be terse".to_string(),
    ];

    let args = HerdrBackend::typed_launch_args("w6H:p1", &argv).expect("an argv names a binary");

    assert_eq!(args.last().map(String::as_str), Some("be terse"));
}

// An agent that names no binary is refused before herdr is called.
//
// `pane run` with no command is herdr's own usage error, which would reach a
// client as a backend failure naming a CLI it never asked about.
#[test]
fn a_typed_launch_with_nothing_to_run_is_refused() {
    let error = HerdrBackend::typed_launch_args("w6H:p1", &[])
        .expect_err("an empty argv names no binary");

    assert!(
        error.to_string().contains("names no binary"),
        "the refusal must name the cause: {error}"
    );
}
