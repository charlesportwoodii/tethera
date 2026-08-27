//! In-memory ports.
//!
//! Deliberately dumb: they hold vectors and broadcast senders and answer from
//! them. Every behaviour they model is one the protocol requires - a stale
//! fingerprint, a cursor older than the source, an upload offset the server
//! chose - and none of them models herdr, a terminal emulator or a real
//! transcript. That separation is the point: a failure in this suite is a
//! protocol failure.

use std::sync::Mutex;

use tethera_common::protocol::capability::{self, CapabilityId, CapabilitySet};
use tethera_common::protocol::error::{EntityKind, WireError};
use tethera_common::protocol::handshake::{DeviceRecord, ServerInfo};
use tethera_common::protocol::response::{ConversationPreview, Describe, Limits, Page};
use tethera_common::protocol::terminal::{
    attrs, AttachSpec, Color, CursorShape, CursorState, RowUpdate, Span, Style, TerminalFrame,
    TerminalInput,
};
use tethera_common::protocol::transfer::{FetchHead, PutReady, PutResult, PutSpec};
use tethera_common::protocol::watch::WatchEvent;
use tethera_common::structs::agent::{Agent, AgentProfile};
use tethera_common::structs::asset::{AssetCard, AssetScope};
use tethera_common::structs::conversation::{Conversation, ConversationFilter};
use tethera_common::structs::ids::{
    AssetId, ConversationId, DeviceId, PaneId, ProfileId, QuestionId, TabId, TurnId,
    WorkspaceId,
};
use tethera_common::structs::primitives::{Cursor, Fingerprint, Sha256, Timestamp};
use tethera_common::structs::terminal::{
    Pane, Size, SplitDirection, Tab, TabLayout, Workspace,
};
use tethera_common::structs::transcript::{
    Ask,
    Answer, Part, Question, QuestionOption, Role, Turn,
};
use tethera_common::traits::AgentTrait;
use tethera_server_lib::protocol::ports::{
    AssetPort, ConversationPort, EnrollOffer, Enrolment, MachinePort, Ports, ScrollbackPage,
    TerminalSession, TerminalPort, TreeSnapshot,
};
use tokio::sync::broadcast;

pub const CODE: &str = "732914";
pub const TURNS: usize = 25;
pub const ASSET_BYTES: usize = 5000;
pub const RESUMED_NAME: &str = "already-started.png";
pub const RESUMED_OFFSET: u64 = 2048;

pub fn bound_conversation() -> ConversationId {
    ConversationId::parse("cv_bound").expect("valid")
}

pub fn unbound_conversation() -> ConversationId {
    ConversationId::parse("cv_unbound").expect("valid")
}

pub fn agent_pane() -> PaneId {
    PaneId::parse("pn_agent").expect("valid")
}

pub fn question_id() -> QuestionId {
    QuestionId::parse("qs_route").expect("valid")
}

pub fn question_options() -> Vec<QuestionOption> {
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

pub const QUESTION_PROMPT: &str = "Which route should own tethera://pair?";

pub fn question_asks() -> Vec<Ask> {
    vec![Ask {
        header: None,
        prompt: QUESTION_PROMPT.into(),
        options: question_options(),
        multi_select: false,
        allows_free_text: false,
    }]
}

pub fn question() -> Question {
    Question {
        id: question_id(),
        fingerprint: Question::fingerprint_of(&question_asks()),
        asks: question_asks(),
    }
}

pub struct FakePorts {
    machine: FakeMachine,
    conversations: FakeConversations,
    terminals: FakeTerminals,
    assets: FakeAssets,
}

impl FakePorts {
    pub fn new() -> Self {
        let (tree_tx, _) = broadcast::channel(64);
        let (turn_tx, _) = broadcast::channel(64);

        Self {
            machine: FakeMachine {
                enrolled: Mutex::new(None),
                revoked: Mutex::new(false),
                attempts_left: Mutex::new(3),
                events: tree_tx,
            },
            conversations: FakeConversations { events: turn_tx },
            terminals: FakeTerminals {
                opened: Mutex::new(Vec::new()),
                inputs: std::sync::Arc::new(Mutex::new(Vec::new())),
                focused: Mutex::new(Vec::new()),
            },
            assets: FakeAssets {
                uploaded: Mutex::new(Vec::new()),
            },
        }
    }

    /// Every `TerminalInput` any attach handed to a pane, in order.
    pub fn recorded_inputs(&self) -> Vec<TerminalInput> {
        self.terminals.inputs.lock().expect("lock").clone()
    }

    pub fn publish_tree_event(&self, event: WatchEvent) {
        let _ = self.machine.events.send(event);
    }

    pub fn publish_turn(&self, event: WatchEvent) {
        let _ = self.conversations.events.send(event);
    }
}

impl Ports for FakePorts {
    type Machine = FakeMachine;
    type Conversations = FakeConversations;
    type Terminals = FakeTerminals;
    type Assets = FakeAssets;

    fn machine(&self) -> &Self::Machine {
        &self.machine
    }

    fn conversations(&self) -> &Self::Conversations {
        &self.conversations
    }

    fn terminals(&self) -> &Self::Terminals {
        &self.terminals
    }

    fn assets(&self) -> &Self::Assets {
        &self.assets
    }
}

pub struct FakeMachine {
    enrolled: Mutex<Option<DeviceRecord>>,
    revoked: Mutex<bool>,
    attempts_left: Mutex<u8>,
    events: broadcast::Sender<WatchEvent>,
}

impl FakeMachine {
    fn info() -> ServerInfo {
        ServerInfo {
            id: tethera_common::structs::ids::ServerId::parse("sv_atlas").expect("valid"),
            label: "atlas".into(),
            app_version: "0.1.0".into(),
            os: "linux".into(),
            arch: "x86_64".into(),
        }
    }

    fn capabilities() -> CapabilitySet {
        [
            capability::AGENT_CATALOG,
            capability::TRANSCRIPT_PAGING,
            capability::QUESTIONS,
            capability::TERMINAL_ATTACH,
            capability::TERMINAL_INPUT,
            capability::PANE_OPEN,
            capability::ASSETS_READ,
            capability::ASSETS_WRITE,
            capability::DEVICE_SELF_REVOKE,
        ]
        .into_iter()
        .map(CapabilityId::from)
        .collect()
    }
}

impl MachinePort for FakeMachine {
    async fn describe(&self) -> Describe {
        Describe {
            server: Self::info(),
            capabilities: Self::capabilities(),
            limits: Limits {
                max_control_frame: 64 * 1024,
                max_streams: 64,
                transcript_page: 200,
                scrollback_page: 500,
                max_upload: None,
            },
        }
    }

    fn server_info(&self) -> ServerInfo {
        Self::info()
    }

    async fn agent_profiles(&self) -> Vec<AgentProfile> {
        Agent::ALL.iter().map(Agent::profile).collect()
    }

    async fn recent_cwds(&self, limit: u16) -> Vec<String> {
        vec!["/home/charl/projects/tethera".to_string()]
            .into_iter()
            .take(usize::from(limit))
            .collect()
    }

    async fn tree(&self) -> Result<TreeSnapshot, WireError> {
        let workspace = WorkspaceId::parse("ws_tethera").expect("valid");
        let other = WorkspaceId::parse("ws_scratch").expect("valid");
        let agent_tab = TabId::parse("tb_claude").expect("valid");

        Ok(TreeSnapshot {
            layouts: Vec::new(),
            workspaces: vec![
                Workspace {
                    id: workspace.clone(),
                    name: "tethera-3".into(),
                    cwd: Some("/home/charl/projects/tethera".into()),
                    tab_count: 2,
                    conversation: Some(bound_conversation()),
                },
                Workspace {
                    id: other.clone(),
                    name: "scratch".into(),
                    cwd: None,
                    tab_count: 1,
                    conversation: None,
                },
            ],
            tabs: vec![
                Tab {
                    id: agent_tab.clone(),
                    workspace_id: workspace.clone(),
                    index: 1,
                    title: "claude".into(),
                    conversation: Some(bound_conversation()),
                    foreground_command: None,
                },
                Tab {
                    id: TabId::parse("tb_build").expect("valid"),
                    workspace_id: workspace.clone(),
                    index: 2,
                    title: "build".into(),
                    conversation: None,
                    foreground_command: Some("cargo".into()),
                },
                Tab {
                    id: TabId::parse("tb_scratch").expect("valid"),
                    workspace_id: other.clone(),
                    index: 1,
                    title: "zsh".into(),
                    conversation: None,
                    foreground_command: None,
                },
            ],
            panes: vec![
                Pane {
                    id: agent_pane(),
                    tab_id: agent_tab,
                    workspace_id: workspace.clone(),
                    label: "claude".into(),
                    title: None,
                    cwd: Some("/home/charl/projects/tethera".into()),
                    size: Size { cols: 120, rows: 40 },
                    focused: true,
                    foreground_command: Some("claude".into()),
                    conversation: Some(bound_conversation()),
                    agent: None,
                },
                Pane {
                    id: PaneId::parse("pn_build").expect("valid"),
                    tab_id: TabId::parse("tb_build").expect("valid"),
                    workspace_id: workspace,
                    label: "build".into(),
                    title: None,
                    cwd: None,
                    size: Size { cols: 120, rows: 40 },
                    focused: false,
                    foreground_command: Some("cargo".into()),
                    conversation: None,
                    agent: None,
                },
                Pane {
                    id: PaneId::parse("pn_scratch").expect("valid"),
                    tab_id: TabId::parse("tb_scratch").expect("valid"),
                    workspace_id: other,
                    label: "zsh".into(),
                    title: None,
                    cwd: None,
                    size: Size { cols: 80, rows: 24 },
                    focused: false,
                    foreground_command: None,
                    conversation: None,
                    agent: None,
                },
            ],
            conversations: vec![
                FakeConversations::bound(),
                FakeConversations::unbound(),
            ],
        })
    }

    fn tree_events(&self) -> broadcast::Receiver<WatchEvent> {
        self.events.subscribe()
    }

    async fn enrolment(&self, _endpoint_id: &str) -> Enrolment {
        if *self.revoked.lock().expect("lock") {
            return Enrolment::Revoked;
        }

        match self.enrolled.lock().expect("lock").clone() {
            Some(device) => Enrolment::Known(device),
            None => Enrolment::Unknown,
        }
    }

    async fn pairing_window(&self) -> Option<EnrollOffer> {
        // Open for the whole test. A closed window is covered by its own case,
        // which constructs the decision directly rather than through a
        // connection.
        Some(EnrollOffer {
            server: Self::info(),
            code_length: CODE.len() as u8,
            expires_in_ms: 120_000,
        })
    }

    async fn redeem_code(
        &self,
        _endpoint_id: &str,
        code: &str,
        device_name: &str,
    ) -> Result<DeviceRecord, u8> {
        if code != CODE {
            let mut left = self.attempts_left.lock().expect("lock");
            *left = left.saturating_sub(1);

            return Err(*left);
        }

        let device = DeviceRecord {
            id: DeviceId::parse("dv_phone").expect("valid"),
            name: device_name.to_string(),
            paired_at: Timestamp(1_766_000_000_000),
        };

        *self.enrolled.lock().expect("lock") = Some(device.clone());

        Ok(device)
    }

    async fn revoke(&self, _endpoint_id: &str) -> Result<(), WireError> {
        *self.revoked.lock().expect("lock") = true;
        *self.enrolled.lock().expect("lock") = None;

        Ok(())
    }
}

pub struct FakeConversations {
    events: broadcast::Sender<WatchEvent>,
}

impl FakeConversations {
    pub fn bound() -> Conversation {
        Conversation {
            id: bound_conversation(),
            profile: ProfileId("claude".into()),
            profile_label: "Claude Code".into(),
            title: Some("protocol design".into()),
            preview: Some(QUESTION_PROMPT.into()),
            cwd: "/home/charl/projects/tethera".into(),
            workspace: Some(WorkspaceId::parse("ws_tethera").expect("valid")),
            started_at: Timestamp(1_766_000_000_000),
            last_active: Some(Timestamp(1_766_000_900_000)),
            turn_count: Some(TURNS as u32),
            status: tethera_common::structs::agent::AgentStatus::Blocked,
            has_transcript: true,
            resumable: true,
            binding: Some(agent_pane()),
        }
    }

    pub fn unbound() -> Conversation {
        Conversation {
            id: unbound_conversation(),
            profile: ProfileId("claude".into()),
            profile_label: "Claude Code".into(),
            title: Some("last week".into()),
            preview: Some("that is everything".into()),
            cwd: "/home/charl/projects/bvc".into(),
            workspace: None,
            started_at: Timestamp(1_765_000_000_000),
            last_active: Some(Timestamp(1_765_000_900_000)),
            turn_count: Some(3),
            status: tethera_common::structs::agent::AgentStatus::Done,
            has_transcript: true,
            resumable: true,
            binding: None,
        }
    }

    /// Turn `n`, oldest first, cursor `t{n}`.
    fn turn(index: usize) -> Turn {
        Turn {
            cursor: Cursor(format!("t{index}")),
            id: TurnId(format!("rec-{index}")),
            at: Timestamp(1_766_000_000_000 + index as i64),
            role: if index % 2 == 0 {
                Role::Operator
            } else {
                Role::Agent
            },
            parts: vec![Part::Text {
                text: format!("turn {index}"),
            }],
        }
    }

    /// The newest cursor, which is where a live tail starts.
    pub fn newest_cursor() -> Cursor {
        Cursor(format!("t{}", TURNS - 1))
    }
}

impl ConversationPort for FakeConversations {
    async fn list(
        &self,
        filter: ConversationFilter,
        _before: Option<Cursor>,
        limit: u16,
    ) -> Page<Conversation> {
        let all = vec![Self::bound(), Self::unbound()];
        let items: Vec<Conversation> = all
            .into_iter()
            .filter(|c| match filter {
                ConversationFilter::All => true,
                ConversationFilter::Live => c.binding.is_some(),
                ConversationFilter::Blocked => {
                    c.status == tethera_common::structs::agent::AgentStatus::Blocked
                }
            })
            .take(usize::from(limit))
            .collect();

        Page {
            items,
            next_before: None,
            has_earlier: false,
        }
    }

    async fn get(&self, id: &ConversationId) -> Result<Conversation, WireError> {
        if *id == bound_conversation() {
            Ok(Self::bound())
        } else if *id == unbound_conversation() {
            Ok(Self::unbound())
        } else {
            Err(WireError::NotFound {
                kind: EntityKind::Conversation,
            })
        }
    }

    async fn transcript(
        &self,
        id: &ConversationId,
        before: Option<Cursor>,
        limit: u16,
    ) -> Result<Page<Turn>, WireError> {
        if *id != bound_conversation() {
            return Err(WireError::NotFound {
                kind: EntityKind::Conversation,
            });
        }

        // `before: None` is the most recent page. Paging backwards walks
        // `next_before`, and `has_earlier` is the source's own answer.
        let end = match &before {
            None => TURNS,
            Some(cursor) => cursor
                .as_str()
                .strip_prefix('t')
                .and_then(|n| n.parse::<usize>().ok())
                .ok_or(WireError::Stale)?,
        };

        let take = usize::from(limit).min(end);
        let start = end - take;
        let items: Vec<Turn> = (start..end).map(Self::turn).collect();

        Ok(Page {
            items,
            next_before: if start == 0 {
                None
            } else {
                Some(Cursor(format!("t{start}")))
            },
            has_earlier: start > 0,
        })
    }

    async fn subscribe(
        &self,
        id: &ConversationId,
        after: Option<Cursor>,
    ) -> Result<(Cursor, broadcast::Receiver<WatchEvent>), WireError> {
        if *id != bound_conversation() {
            return Err(WireError::NotFound {
                kind: EntityKind::Conversation,
            });
        }

        // A cursor older than the earliest surviving record cannot be honoured.
        // Saying so - rather than silently starting later - is what lets the
        // client refetch the gap instead of rendering a hole it cannot see.
        let requested = after
            .as_ref()
            .and_then(|c| c.as_str().strip_prefix('t'))
            .and_then(|n| n.parse::<usize>().ok());

        let from = match requested {
            Some(index) if index >= TURNS / 2 => Cursor(format!("t{index}")),
            _ => Cursor(format!("t{}", TURNS / 2)),
        };

        Ok((from, self.events.subscribe()))
    }

    async fn preview(
        &self,
        _profile: &ProfileId,
        _cwd: &str,
        workspace: Option<&WorkspaceId>,
    ) -> Result<ConversationPreview, WireError> {
        Ok(ConversationPreview {
            workspace_label: "tethera-4".into(),
            tab_label: "claude".into(),
            creates_workspace: workspace.is_none(),
            will_have_transcript: false,
        })
    }

    async fn start(
        &self,
        _profile: &ProfileId,
        _cwd: &str,
        _prompt: Option<&str>,
        _attachments: &[AssetId],
    ) -> Result<Conversation, WireError> {
        Ok(Self::bound())
    }

    async fn resume(
        &self,
        id: &ConversationId,
        _cwd: Option<&str>,
    ) -> Result<Conversation, WireError> {
        self.get(id).await
    }

    async fn send_prompt(
        &self,
        id: &ConversationId,
        _text: &str,
        _attachments: &[AssetId],
    ) -> Result<(), WireError> {
        // A conversation with no live pane cannot be typed into. This is the
        // client's cue to offer a resume, which starts a process and must never
        // happen implicitly.
        if *id == unbound_conversation() {
            return Err(WireError::NotFound {
                kind: EntityKind::Pane,
            });
        }

        Ok(())
    }

    async fn interrupt(&self, _id: &ConversationId) -> Result<(), WireError> {
        Ok(())
    }

    async fn stop(&self, _id: &ConversationId) -> Result<(), WireError> {
        Ok(())
    }

    async fn answer(
        &self,
        _id: &ConversationId,
        asked: &QuestionId,
        fingerprint: &Fingerprint,
        _answers: &[Answer],
    ) -> Result<(), WireError> {
        if *asked != question_id() {
            return Err(WireError::NotFound {
                kind: EntityKind::Question,
            });
        }

        if *fingerprint != question().fingerprint {
            return Err(WireError::Stale);
        }

        Ok(())
    }
}

pub struct FakeTerminals {
    opened: Mutex<Vec<PaneId>>,
    inputs: std::sync::Arc<Mutex<Vec<TerminalInput>>>,
    /// Every tab a caller asked the desk to move to, in order.
    ///
    /// Recorded rather than acted on, so a test can assert that a tab tap
    /// reached the backend without a screen to read it off.
    focused: Mutex<Vec<TabId>>,
}

impl FakeTerminals {
    fn plain() -> Style {
        Style {
            fg: Color::Default,
            bg: Color::Default,
            attrs: attrs::NONE,
        }
    }

    fn row(y: u16, text: &str) -> RowUpdate {
        RowUpdate {
            y,
            from_x: 0,
            spans: vec![Span {
                style: 0,
                text: text.into(),
            }],
        }
    }
}

impl TerminalPort for FakeTerminals {
    type Session = FakeSession;

    async fn list_tabs(&self, _workspace: &WorkspaceId) -> Result<Vec<Tab>, WireError> {
        Ok(Vec::new())
    }

    async fn list_panes(&self, _tab: &TabId) -> Result<Vec<Pane>, WireError> {
        Ok(Vec::new())
    }

    /// No tab this fake reports has a geometry, so every tab is a miss.
    ///
    /// `NotFound` rather than an empty layout: a client told a tab has no panes
    /// draws an empty workspace, which is a different and wrong statement.
    async fn layout(&self, _tab: &TabId) -> Result<TabLayout, WireError> {
        Err(WireError::NotFound {
            kind: EntityKind::Tab,
        })
    }

    async fn focus_tab(&self, tab: &TabId) -> Result<(), WireError> {
        self.focused.lock().expect("lock").push(tab.clone());

        Ok(())
    }

    async fn open(
        &self,
        workspace: Option<&WorkspaceId>,
        cwd: Option<&str>,
    ) -> Result<Pane, WireError> {
        let id = PaneId::mint("opened");
        self.opened.lock().expect("lock").push(id.clone());

        Ok(Pane {
            id,
            tab_id: TabId::mint("opened"),
            workspace_id: workspace
                .cloned()
                .unwrap_or_else(|| WorkspaceId::parse("ws_tethera").expect("valid")),
            label: "zsh".into(),
            title: None,
            cwd: cwd.map(str::to_string),
            // Decided here and stable for the pane's life. There is no resize
            // anywhere in this protocol.
            size: Size { cols: 120, rows: 40 },
            focused: true,
            foreground_command: None,
            conversation: None,
            agent: None,
        })
    }

    async fn split(&self, _pane: &PaneId, _direction: SplitDirection) -> Result<Pane, WireError> {
        Err(WireError::Backend {
            message: "the fake does not split".into(),
        })
    }

    async fn close(&self, _pane: &PaneId) -> Result<(), WireError> {
        Ok(())
    }

    async fn attach(&self, spec: &AttachSpec) -> Result<Self::Session, WireError> {
        if spec.pane != agent_pane() {
            return Err(WireError::NotFound {
                kind: EntityKind::Pane,
            });
        }

        Ok(FakeSession {
            remaining: vec![
                TerminalFrame::Damage {
                    styles: vec![Self::plain()],
                    rows_data: vec![Self::row(1, "second")],
                    cursor: None,
                },
                TerminalFrame::Damage {
                    styles: vec![Self::plain()],
                    rows_data: vec![Self::row(0, "first ")],
                    cursor: Some(CursorState {
                        x: 0,
                        y: 0,
                        visible: true,
                        shape: CursorShape::Block,
                    }),
                },
                // The snapshot is popped first, so it is pushed last.
                TerminalFrame::Snapshot {
                    cols: 6,
                    rows: 2,
                    styles: vec![Self::plain()],
                    rows_data: vec![Self::row(0, "hello ")],
                    cursor: Some(CursorState {
                        x: 5,
                        y: 0,
                        visible: true,
                        shape: CursorShape::Block,
                    }),
                    alt_screen: false,
                    scrollback_len: Some(12),
                },
            ],
            inputs: self.inputs.clone(),
        })
    }

    async fn scrollback(
        &self,
        _pane: &PaneId,
        _before_line: Option<u32>,
        _limit: u16,
    ) -> Result<ScrollbackPage, WireError> {
        Ok((
            vec![Self::plain()],
            vec![Self::row(0, "older")],
            None,
            false,
        ))
    }
}

pub struct FakeSession {
    remaining: Vec<TerminalFrame>,
    inputs: std::sync::Arc<Mutex<Vec<TerminalInput>>>,
}

impl TerminalSession for FakeSession {
    async fn next_frame(&mut self) -> Option<TerminalFrame> {
        match self.remaining.pop() {
            Some(frame) => Some(frame),
            None => {
                // Nothing further, but do not end the stream: the test still has
                // input to send, and a pane that stopped producing output has
                // not stopped existing.
                std::future::pending::<()>().await;
                None
            }
        }
    }

    async fn send_input(&mut self, input: TerminalInput) -> Result<(), WireError> {
        self.inputs.lock().expect("lock").push(input);

        Ok(())
    }
}

pub struct FakeAssets {
    uploaded: Mutex<Vec<String>>,
}

impl FakeAssets {
    pub fn asset() -> AssetId {
        AssetId::parse("as_report").expect("valid")
    }

    pub fn body() -> Vec<u8> {
        (0..ASSET_BYTES).map(|i| (i % 251) as u8).collect()
    }
}

impl AssetPort for FakeAssets {
    type Body = std::io::Cursor<Vec<u8>>;

    async fn list(
        &self,
        _scope: &AssetScope,
        _before: Option<Cursor>,
        _limit: u16,
    ) -> Result<Page<AssetCard>, WireError> {
        Ok(Page {
            items: vec![AssetCard {
                asset: Self::asset(),
                name: "report.md".into(),
                mime: Some("text/markdown".into()),
                size: Some(ASSET_BYTES as u64),
                modified: None,
            }],
            next_before: None,
            has_earlier: false,
        })
    }

    async fn fetch(
        &self,
        asset: &AssetId,
        offset: u64,
    ) -> Result<(FetchHead, Self::Body), WireError> {
        if *asset != Self::asset() {
            return Err(WireError::NotFound {
                kind: EntityKind::Asset,
            });
        }

        let body = Self::body();
        let start = (offset as usize).min(body.len());

        Ok((
            FetchHead {
                // The whole asset, not what is about to be sent.
                len: body.len() as u64,
                mime: Some("text/markdown".into()),
                sha256: Sha256("e3b0c44298fc1c14".into()),
                offset: start as u64,
            },
            std::io::Cursor::new(body[start..].to_vec()),
        ))
    }

    async fn put_ready(&self, spec: &PutSpec) -> Result<PutReady, WireError> {
        // A name this machine has seen before resumes where its bytes stopped.
        // The client's proposal is ignored on purpose: only the server knows how
        // much reached disk.
        if spec.name == RESUMED_NAME {
            return Ok(PutReady {
                offset: RESUMED_OFFSET,
            });
        }

        Ok(PutReady { offset: 0 })
    }

    async fn put_finish(&self, spec: &PutSpec, body: &[u8]) -> Result<PutResult, WireError> {
        let expected = spec.len
            - if spec.name == RESUMED_NAME {
                RESUMED_OFFSET
            } else {
                0
            };

        if body.len() as u64 != expected {
            return Err(WireError::Backend {
                message: format!("expected {expected} bytes, received {}", body.len()),
            });
        }

        self.uploaded.lock().expect("lock").push(spec.name.clone());

        Ok(PutResult {
            asset: AssetId::mint("uploaded"),
        })
    }
}
