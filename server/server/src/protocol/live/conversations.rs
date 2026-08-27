use crate::protocol::live::{BlockWatch, LiveTerminals, ResumeGate};
use crate::protocol::ports::{ConversationPort, TerminalPort};
use crate::terminal::{Picker, PromptDetector};
use crate::transcript::{
    AssetIndex, AssetNaming, SessionCatalog, StatusRule, TranscriptReader, TranscriptWatcher,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tethera_common::protocol::capability::{self, CapabilityId, CapabilitySet};
use tethera_common::protocol::error::{EntityKind, WireError};
use tethera_common::protocol::response::{ConversationPreview, Page};
use tethera_common::protocol::terminal::{Key, Mods, TerminalInput};
use tethera_common::protocol::watch::WatchEvent;
use tethera_common::structs::agent::{Agent, AgentSpawn, AgentStatus};
use tethera_common::structs::conversation::{Conversation, ConversationFilter};
use tethera_common::structs::ids::{
    AssetId, ConversationId, PaneId, ProfileId, QuestionId, WorkspaceId,
};
use tethera_common::structs::primitives::{Cursor, Fingerprint, Timestamp};
use tethera_common::structs::terminal::Pane;
use tethera_common::structs::transcript::{Answer, Question, Turn};
use tethera_common::traits::AgentTrait;
use tokio::sync::broadcast;

/// `ConversationPort` over an agent's own records.
///
/// The read half is real. The half that types into a pane is not: answering a
/// question, sending a prompt and starting a process all need a terminal
/// backend to drive, and this says so rather than pretending.
pub struct LiveConversations {
    terminals: Arc<LiveTerminals>,
    catalog: Arc<SessionCatalog>,
    watcher: Arc<TranscriptWatcher>,
    /// One reader per conversation, kept so an index survives between pages.
    ///
    /// A `std::sync::Mutex` and every use inside `spawn_blocking`, with the lock
    /// taken inside the closure. The guard therefore never crosses an `.await`
    /// and the futures this port returns stay `Send`. A `tokio::sync::Mutex`
    /// would compile and then hold a lock across a synchronous read of a file
    /// measured at 57.5 MB, on a runtime thread.
    readers: Arc<Mutex<HashMap<ConversationId, TranscriptReader>>>,
    /// Where the files these records name actually live.
    ///
    /// Held so every reader this port opens records them as it reads. The id on
    /// a `Part::File` is a one-way hash, so the read that produces the card is
    /// the only moment the path is in hand — and without this the card would
    /// arrive on a phone and open onto nothing.
    assets: Arc<AssetIndex>,
    /// This machine's own upload directory.
    ///
    /// A file a person sends reaches the agent as an `Attached: <path>` line
    /// inside their prompt, written by `naming` below. Reading those lines back
    /// out is what gives their own attachments a card instead of a raw path —
    /// and this directory is the anchor that distinguishes a line this machine
    /// wrote from a person who happened to type the word.
    uploads: PathBuf,
}

impl LiveConversations {
    /// Which agent's records this build can read.
    ///
    /// One arm today. Every difference between two harnesses is a table on
    /// `AgentTrait`, so a second is another arm and no change here.
    const READS: Agent = Agent::Claude;

    /// How much of a conversation's tail decides its status and its preview.
    const TAIL: u16 = 16;

    /// How many times a review screen is looked for before giving up, and how
    /// long between looks.
    ///
    /// Together a little over three seconds, which is the slowest repaint
    /// measured on a busy agent. Long enough to catch the screen appearing,
    /// short enough that a set the harness submitted itself does not hold the
    /// call open.
    const REVIEW_READS: usize = 8;
    const REVIEW_WAIT: std::time::Duration = std::time::Duration::from_millis(400);

    /// How long the question is watched for, afterwards, before this machine
    /// admits it could not drive the screen.
    ///
    /// Longer than the review window, because it has to outlast a harness that
    /// is thinking rather than one that is merely repainting.
    const CONFIRM_READS: usize = 12;

    /// How recently a file must have grown for its agent to count as working.
    const WORKING_WITHIN_MS: i64 = 30_000;

    /// How many sessions a directory suggestion looks back over.
    ///
    /// Each one costs a summary read, so the whole catalog is not walked to fill
    /// a short list. Deep enough that a machine whose newest sessions all sit in
    /// one repository still offers the ones before it.
    const RECENT_CWD_SCAN: usize = 60;

    pub const NEEDS_BACKEND: &'static str =
        "this machine cannot drive an agent's pane yet; start or answer the agent at the machine";

    pub fn new(
        terminals: Arc<LiveTerminals>,
        assets: Arc<AssetIndex>,
        uploads: PathBuf,
    ) -> Self {
        let home = directories::UserDirs::new()
            .map(|dirs| dirs.home_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        Self::at(terminals, &home, assets, uploads)
    }

    /// The same port with the home directory named.
    ///
    /// A behavioural seam rather than a builder: what this port answers depends
    /// entirely on what is on disk under that directory, and there is no other
    /// way to observe an empty machine, or a machine with exactly two known
    /// sessions, without depending on whose machine the test is running on.
    pub fn at(
        terminals: Arc<LiveTerminals>,
        home: &Path,
        assets: Arc<AssetIndex>,
        uploads: PathBuf,
    ) -> Self {
        Self {
            terminals,
            uploads,
            catalog: Arc::new(SessionCatalog::new(home)),
            watcher: Arc::new(TranscriptWatcher::new(Self::READS)),
            readers: Arc::new(Mutex::new(HashMap::new())),
            assets,
        }
    }

    pub fn new_shared(
        terminals: Arc<LiveTerminals>,
        assets: Arc<AssetIndex>,
        uploads: PathBuf,
    ) -> Arc<Self> {
        Arc::new(Self::new(terminals, assets, uploads))
    }

    /// What this port does with no terminal backend at all.
    ///
    /// Reading and paging a transcript works, and so does describing what a
    /// start would create, because both are answered from disk and from the
    /// catalog. Everything that needs to type into a pane is absent, and
    /// `questions` with it: a client told this machine answers questions would
    /// draw a control that refuses, which is the one failure a capability set
    /// exists to prevent.
    pub fn capabilities() -> CapabilitySet {
        [capability::TRANSCRIPT_PAGING, capability::CONVERSATION_PREVIEW]
            .into_iter()
            .map(CapabilityId::from)
            .collect()
    }

    /// What this port can additionally do while a terminal backend answers.
    ///
    /// Separate from `capabilities` because a backend can die under a running
    /// server. These are unioned in behind the same live probe the pane
    /// capabilities are gated on, so a machine that has lost herdr stops
    /// offering to start conversations at the same moment it stops offering to
    /// open panes.
    ///
    /// `questions` covers both kinds a person is asked, because the client cannot
    /// tell them apart and must not be given a control that answers only one: an
    /// agent-initiated question, read from the records, and a permission prompt,
    /// read off the screen. Both are answered by pressing a row's number, which
    /// is what let one path serve both.
    ///
    /// `conversation_stop` is not here — closing a pane and ending an agent are
    /// different acts, and only the first is written.
    pub fn backed_capabilities() -> CapabilitySet {
        [
            capability::CONVERSATION_START,
            capability::CONVERSATION_RESUME,
            capability::PROMPT_SEND,
            capability::INTERRUPT,
            capability::QUESTIONS,
        ]
        .into_iter()
        .map(CapabilityId::from)
        .collect()
    }

    /// Directories agents have recently worked in, newest first, deduplicated.
    ///
    /// Derived from the sessions on disk rather than from a list of its own,
    /// because a separate record would be a second truth to keep and would be
    /// empty on a machine that has been working for months. A directory that no
    /// longer exists is dropped: `start` would refuse it, and offering a
    /// suggestion the next call rejects is worse than offering nothing.
    pub async fn recent_cwds(&self, limit: u16) -> Vec<String> {
        let wanted = usize::from(limit);

        if wanted == 0 {
            return Vec::new();
        }

        let catalog = self.catalog.clone();

        tokio::task::spawn_blocking(move || {
            let mut found: Vec<String> = Vec::new();

            for path in catalog.discover().into_iter().take(Self::RECENT_CWD_SCAN) {
                if found.len() >= wanted {
                    break;
                }

                let Some(cwd) = catalog.summarise(&path).and_then(|summary| summary.cwd) else {
                    continue;
                };

                if found.iter().any(|known| known == &cwd) || !Path::new(&cwd).is_dir() {
                    continue;
                }

                found.push(cwd);
            }

            found
        })
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(%error, "could not read the recently used directories");

            Vec::new()
        })
    }

    /// How many conversations this port is tailing.
    ///
    /// One poller per conversation however many clients watch it: `watch.rs`
    /// subscribes again on a lagged receiver, so anything else leaks a task per
    /// lag.
    pub fn watching(&self) -> usize {
        self.watcher.watching()
    }

    /// The agent a profile names, when this build can follow one it starts.
    ///
    /// A machine can run an agent whose records it cannot read, and the pane
    /// would be real — but there would be no conversation to answer with, and
    /// `Conversation` is what this call returns. The refusal names the profile
    /// so a client can say which one, and `AgentProfile.provides_transcript`
    /// already tells it which rows will be refused before anything is tapped.
    fn launchable(profile: &ProfileId) -> Result<Agent, WireError> {
        let named = Agent::ALL
            .iter()
            .find(|agent| agent.profile().id == *profile)
            .copied()
            .ok_or(WireError::NotFound {
                kind: EntityKind::Conversation,
            })?;

        if named != Self::READS {
            return Err(WireError::Backend {
                message: format!(
                    "this machine can open a pane for {} but cannot follow its conversation, \
                     so it has nothing to hand back; start it at the machine instead",
                    named.profile().label
                ),
            });
        }

        Ok(named)
    }

    /// A working directory this machine will really open a pane in.
    ///
    /// Every part of this is a refusal a phone can trigger. A relative path
    /// would resolve against whatever directory the server happens to be
    /// running in, which is not a directory the person picked; a path that is
    /// not a directory, or is not there at all, would open a pane somewhere
    /// surprising or fail inside the backend with a message about herdr.
    async fn directory_on_this_machine(cwd: &str) -> Result<String, WireError> {
        let cwd = cwd.trim().to_owned();

        if cwd.is_empty() {
            return Err(WireError::Backend {
                message: "a conversation needs a working directory to start in".to_string(),
            });
        }

        let asked = cwd.clone();
        let resolved = tokio::task::spawn_blocking(move || {
            let path = Path::new(&asked);

            if !path.is_absolute() {
                return Err(format!(
                    "{asked} is not a full path; a working directory has to be named from the \
                     root of this machine"
                ));
            }

            match std::fs::metadata(path) {
                Ok(found) if found.is_dir() => Ok(asked),
                Ok(_) => Err(format!("{asked} is a file on this machine, not a directory")),
                Err(_) => Err(format!("there is no directory {asked} on this machine")),
            }
        })
        .await
        .map_err(|error| {
            tracing::error!(%error, "a working directory check did not finish");

            WireError::Backend {
                message: "this machine could not check the working directory".to_string(),
            }
        })?;

        resolved.map_err(|message| WireError::Backend { message })
    }

    /// The conversation a start just created.
    ///
    /// Described from the pane rather than by re-listing, and its records are
    /// read only if they are already on disk. An agent that has begun a session
    /// has not necessarily written a turn, and a summary is not what a start
    /// owes the caller: the id, the pane it is bound to and the directory it is
    /// in are all facts of the start itself.
    async fn started(
        &self,
        id: ConversationId,
        agent: Agent,
        pane: &Pane,
        cwd: String,
    ) -> Result<Conversation, WireError> {
        if let Some(described) = self
            .locate(&id)
            .map(|path| self.describe(path, Some(pane.id.clone())))
        {
            if let Some(conversation) = described.await {
                return Ok(Conversation {
                    workspace: Some(pane.workspace_id.clone()),
                    ..conversation
                });
            }
        }

        let profile = agent.profile();

        Ok(Conversation {
            id,
            profile: profile.id,
            profile_label: profile.label,
            // Claude Code titles a session from its first turn, and there has
            // not been one. The directory is what a person would call it, and
            // the next listing replaces this with the agent's own title.
            title: Some(
                Path::new(&cwd)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| cwd.clone()),
            ),
            preview: None,
            cwd,
            workspace: Some(pane.workspace_id.clone()),
            started_at: Timestamp(chrono::Utc::now().timestamp_millis()),
            last_active: None,
            turn_count: None,
            // Started and holding a keyboard nobody has typed at. `Working`
            // would claim a turn is under way, which no records support.
            status: AgentStatus::Idle,
            has_transcript: profile.provides_transcript,
            resumable: true,
            binding: Some(pane.id.clone()),
        })
    }

    /// A watcher over this conversation, built from what this port already holds.
    ///
    /// Cheap: every field it needs is already reference counted, and the reader
    /// map is shared — so a watcher reuses the index a page has already built
    /// rather than re-reading the file from the start.
    fn blocks(&self) -> BlockWatch {
        BlockWatch::new(
            self.terminals.clone(),
            self.catalog.clone(),
            self.readers.clone(),
            Self::READS,
        )
    }

    /// The set of questions this conversation is waiting on, read fresh.
    ///
    /// Read now rather than remembered, because the answer this is checked
    /// against was composed on a phone some seconds ago and the only thing worth
    /// comparing to is what is true at this moment. The same source the watcher
    /// publishes from, so a set detected one way and answered against the other
    /// cannot refuse every answer as stale.
    async fn pending_question(&self, id: &ConversationId) -> Result<Question, WireError> {
        // A read that failed propagates as itself. Reported as `NotFound` it
        // would tell a person their question no longer exists — when what
        // actually happened is that this machine was busy for a moment — and the
        // answer they composed is discarded rather than retried.
        self.blocks()
            .pending(id)
            .await?
            .ok_or(WireError::NotFound {
                kind: EntityKind::Question,
            })
    }

    /// The kinds of answer in a set, as one readable field.
    ///
    /// `choice,text` rather than a debug dump: the values are a person's own
    /// words and a log is not the place for them.
    fn shapes_of(answers: &[Answer]) -> String {
        answers
            .iter()
            .map(|answer| match answer {
                Answer::Choice(_) => "choice",
                Answer::Multi(_) => "multi",
                Answer::Text(_) => "text",
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Whether the question this machine just answered is really gone.
    ///
    /// **Every failure of this path has been a keystroke reporting success into
    /// a screen it did not change.** The harness draws more than one picker and
    /// they do not take the same keys: the side-by-side one prints `1. Alpha`
    /// and ignores `1` entirely. Keys were sent, nothing moved, `Ok` was
    /// returned, and a person answered the same question over and over with
    /// nothing anywhere saying why.
    ///
    /// So the screen is read back rather than trusted. A different question, or
    /// no question, is success — a set advances question by question, and the
    /// review screen is not the question that was answered. Only the same
    /// prompt still standing is a failure, and it is reported as one.
    ///
    /// A refusal here is recoverable in a way a silent success is not: if the
    /// answer did land after all, the retry meets `NotFound` and says so, rather
    /// than answering twice.
    async fn confirm_taken(&self, pane: &PaneId, answered: &Question) -> Result<(), WireError> {
        for _ in 0..Self::CONFIRM_READS {
            tokio::time::sleep(Self::REVIEW_WAIT).await;

            let screen = self.terminals.screen(pane).await?;

            let unchanged = PromptDetector::detect(&screen)
                .is_some_and(|still| still.fingerprint == answered.fingerprint);

            if !unchanged {
                return Ok(());
            }
        }

        tracing::warn!(
            pane = pane.as_str(),
            question = answered.id.as_str(),
            "the question is still on screen after being answered; this machine could not drive it"
        );

        Err(WireError::Backend {
            message: "this machine typed the answer into the agent's pane and the question is \
                      still there, so it could not drive that screen; answer it at the machine"
                .to_string(),
        })
    }

    /// Presses submit once the review screen has actually appeared.
    ///
    /// **The harness does not repaint when a key lands.** A screen read straight
    /// after a press still shows the picker exactly as it was — measured at up
    /// to three seconds on a working agent — so a single read concludes there is
    /// no review to submit, and the set is left selected and never sent. What
    /// that looks like to a person is answering, watching nothing happen, and
    /// answering again.
    ///
    /// The marker is still read rather than assumed. Which screen appears is the
    /// harness's business, and a rule inferred from today's would one day press
    /// `1` into whatever else is there.
    async fn submit_review(&self, pane: &PaneId) -> Result<(), WireError> {
        for _ in 0..Self::REVIEW_READS {
            if self
                .terminals
                .screen(pane)
                .await?
                .contains(Picker::REVIEW_MARKER)
            {
                return self
                    .terminals
                    .send_key(pane, Picker::SUBMIT, Mods::NONE)
                    .await;
            }

            tokio::time::sleep(Self::REVIEW_WAIT).await;
        }

        // Not an error. A harness that submitted the set itself looks exactly
        // like this from here, and telling somebody their answer failed when it
        // may well have landed sends them to answer it a second time - which is
        // the failure this whole function exists to stop.
        tracing::info!(
            pane = pane.as_str(),
            "no review screen appeared after a set of answers"
        );

        Ok(())
    }

    /// The paths of the files a prompt is carrying.
    ///
    /// **Nothing is copied.** An upload already sits in this machine's own store
    /// under an absolute path, and an agent reads a file by path — so naming
    /// where it is beats putting a second copy somewhere else and naming that.
    ///
    /// It is also not this product's business to write into somebody's
    /// repository. A file that arrived from a phone landing in a working tree is
    /// a file that turns up in `git status`, and an inbox directory beside their
    /// source is clutter they did not ask for and would have to clean up.
    async fn deliver(&self, attachments: &[AssetId]) -> Result<Vec<String>, WireError> {
        let mut named = Vec::new();

        for asset in attachments {
            let path = self.assets.locate(asset).ok_or_else(|| {
                // A client only ever holds an id this machine issued, so a miss
                // here is the machine having forgotten. Logged because this is
                // the path an attachment actually travels — a refusal with
                // nothing anywhere to say which id went missing is a person told
                // their file did not go and nobody able to say why.
                tracing::warn!(
                    asset = asset.as_str(),
                    "an attachment's id resolved to nothing"
                );

                WireError::NotFound {
                    kind: EntityKind::Asset,
                }
            })?;

            // As a person would write it. The id is minted from the canonical
            // spelling and stays so, but that form carries Windows' extended-length
            // prefix — which reaches the agent inside the prompt, where some
            // tools refuse it and where it is noise to anybody reading their own
            // transcript.
            let spelled = AssetNaming::plain(&path.to_string_lossy());

            // A path that came back empty is a file this machine cannot name,
            // and an empty `Attached:` line tells the agent nothing, tells the
            // person nothing, and is indistinguishable from a file that was
            // never attached. Refusing is the behaviour a failed attachment
            // already has.
            if spelled.is_empty() {
                return Err(WireError::NotFound {
                    kind: EntityKind::Asset,
                });
            }

            named.push(spelled);
        }

        Ok(named)
    }

    /// The prompt as the agent will read it, with the files named.
    ///
    /// Named in the text rather than handed over some other way, because a path
    /// in the message is the whole of what an agent needs and the person can see
    /// exactly what was sent.
    fn naming(text: &str, delivered: &[String]) -> String {
        if delivered.is_empty() {
            return text.to_string();
        }

        let mut spoken = text.trim_end().to_string();
        spoken.push_str("\n");

        for path in delivered {
            spoken.push_str(&format!("\nAttached: {path}"));
        }

        spoken
    }

    /// The same prompt, saying so when files did not reach the machine.
    ///
    /// In the prompt because that is the one place the person and the agent are
    /// both looking. The alternative — logging it and returning a conversation
    /// that looks entirely successful — leaves the agent hunting for a file it
    /// was never given, and the person wondering why.
    fn regretting(text: &str, missing: usize) -> String {
        if missing == 0 {
            return text.to_string();
        }

        let files = if missing == 1 { "file" } else { "files" };

        format!(
            "{}\n\n({missing} attached {files} could not be placed on this machine, so you \
             have not been given {}.)",
            text.trim_end(),
            if missing == 1 { "it" } else { "them" }
        )
    }

    /// The pane a conversation is running in, as of a fresh backend read.
    ///
    /// Not `bindings()` on its own: that answers from the last tree render,
    /// which is right for describing a list and wrong for a write. A pane that
    /// closed since then would take a person's message to whatever is at that
    /// prompt now. One snapshot call is cheap against somebody typing.
    async fn running_in(&self, id: &ConversationId) -> Result<PaneId, WireError> {
        self.terminals.tree().await?;

        self.bindings()
            .get(id)
            .cloned()
            .ok_or_else(|| WireError::NotRunning {
                conversation: id.clone(),
            })
    }

    /// Closes a pane a failed start had already opened.
    ///
    /// Best effort, and it must not replace the failure that caused it: the
    /// caller needs to hear why the start failed, not why the tidying did.
    async fn discard(&self, pane: &PaneId) {
        if let Err(error) = self.terminals.close(pane).await {
            tracing::warn!(
                ?error,
                pane = pane.as_str(),
                "could not close the pane a failed start had opened"
            );
        }
    }

    /// The agent started and has begun no session of its own.
    ///
    /// Not a tidy-up case: the agent is running, and on a first run in a new
    /// directory it is running and waiting for somebody at the machine to trust
    /// that directory. Closing the pane would kill it. The pane is named because
    /// it is in the tree the client already draws.
    fn began_no_session(pane: &PaneId) -> WireError {
        WireError::AwaitingAgent { pane: pane.clone() }
    }

    fn unavailable<T>() -> Result<T, WireError> {
        Err(WireError::Backend {
            message: Self::NEEDS_BACKEND.to_string(),
        })
    }

    /// The conversation a session id names.
    ///
    /// The whole of the cross-task contract with the herdr backend, which mints
    /// the same value from the pane's reported `agent_session` - and the tree's
    /// `conversation` points at nothing if the two ever disagree.
    fn conversation_id(session: &str) -> ConversationId {
        ConversationId::mint(session)
    }

    fn session_of(id: &ConversationId) -> Option<String> {
        id.as_str()
            .strip_prefix(ConversationId::PREFIX)
            .filter(|session| !session.is_empty())
            .map(str::to_string)
    }

    fn locate(&self, id: &ConversationId) -> Option<PathBuf> {
        let session = Self::session_of(id)?;

        self.catalog.locate(&session)
    }

    /// Runs a reader's work off the runtime, taking the lock inside the closure.
    async fn with_reader<T, F>(&self, id: &ConversationId, work: F) -> Result<T, WireError>
    where
        T: Send + 'static,
        F: FnOnce(&mut TranscriptReader) -> Result<T, WireError> + Send + 'static,
    {
        let Some(path) = self.locate(id) else {
            return Err(WireError::NotFound {
                kind: EntityKind::Conversation,
            });
        };

        let readers = self.readers.clone();
        let assets = self.assets.clone();
        let uploads = self.uploads.clone();
        let id = id.clone();

        tokio::task::spawn_blocking(move || {
            let mut open = readers.lock().expect("lock");
            let reader = open
                .entry(id)
                .or_insert_with(|| {
                    TranscriptReader::indexing(path, Self::READS, assets, uploads)
                });

            work(reader)
        })
        .await
        .map_err(|error| {
            tracing::error!(%error, "a transcript read did not finish");

            WireError::Backend {
                message: "this machine could not read the conversation's records".to_string(),
            }
        })?
    }

    /// Which pane each conversation is running in, as of the last tree read.
    ///
    /// A read of the terminal port's memory, never a second backend call: a tree
    /// render already made one immediately before asking for this rank.
    fn bindings(&self) -> HashMap<ConversationId, PaneId> {
        self.terminals
            .bindings()
            .into_iter()
            .map(|(pane, conversation)| (conversation, pane))
            .collect()
    }

    async fn describe(
        &self,
        path: PathBuf,
        binding: Option<PaneId>,
    ) -> Option<Conversation> {
        let catalog = self.catalog.clone();
        let summary =
            tokio::task::spawn_blocking(move || catalog.summarise(&path)).await.ok()??;

        let id = Self::conversation_id(&summary.session_id);
        let bound = binding.is_some();

        // Only a bound conversation is worth reading a tail for. An unbound one
        // is `Done` whatever its records say, because nothing is running.
        let (status, preview) = if bound {
            let tail = self
                .with_reader(&id, |reader| reader.page(None, Self::TAIL))
                .await
                .map(|page| page.items)
                .unwrap_or_default();

            let grew = summary.last_active.is_some_and(|at| {
                chrono::Utc::now().timestamp_millis() - at.0 < Self::WORKING_WITHIN_MS
            });

            (
                StatusRule::decide(true, &tail, grew),
                StatusRule::preview(&tail),
            )
        } else {
            (AgentStatus::Done, None)
        };

        let cwd = summary.cwd.unwrap_or_default();
        let title = summary.title;
        // A bound conversation is already running where it says it is, so
        // nothing is in doubt. Only an unbound one has to be ruled out.
        let resumable =
            binding.is_some() || ResumeGate::admits(&cwd, title.as_deref(), &self.terminals.panes());

        Some(Conversation {
            id,
            profile: Self::READS.profile().id,
            profile_label: Self::READS.profile().label,
            title,
            preview,
            cwd,
            // The workspace a conversation belongs to is the pane's, and an
            // unbound conversation has no pane. Absent rather than guessed.
            workspace: None,
            started_at: summary.started_at.unwrap_or(Timestamp(0)),
            last_active: summary.last_active,
            // A count nobody measured is not zero. Reading every session whole
            // to produce one would cost more than the number is worth.
            turn_count: None,
            status,
            has_transcript: true,
            resumable,
            binding,
        })
    }
}

impl ConversationPort for LiveConversations {
    /// Every session on disk, newest first, with the pane bindings joined on.
    ///
    /// No `Result` on this signature, so an empty page is the only way to say
    /// nothing. It is honest here: a machine with no agent projects directory
    /// genuinely has no conversations. The one case where empty would be a lie -
    /// a directory that exists and cannot be read - is logged by the catalog.
    async fn list(
        &self,
        filter: ConversationFilter,
        _before: Option<Cursor>,
        limit: u16,
    ) -> Page<Conversation> {
        let catalog = self.catalog.clone();
        let Ok(paths) = tokio::task::spawn_blocking(move || catalog.discover()).await else {
            return Page {
                items: Vec::new(),
                next_before: None,
                has_earlier: false,
            };
        };

        let bindings = self.bindings();
        let mut items = Vec::new();

        for path in paths {
            let Some(session) = SessionCatalog::session_of(&path) else {
                continue;
            };

            let binding = bindings.get(&Self::conversation_id(&session)).cloned();

            let Some(conversation) = self.describe(path, binding).await else {
                continue;
            };

            let keep = match filter {
                ConversationFilter::All => true,
                ConversationFilter::Live => conversation.binding.is_some(),
                ConversationFilter::Blocked => conversation.status == AgentStatus::Blocked,
            };

            if keep {
                items.push(conversation);
            }

            if items.len() >= usize::from(limit) {
                break;
            }
        }

        Page {
            items,
            // The scan is capped rather than paged; its cursor would be a
            // different space from a transcript's and nothing walks it.
            next_before: None,
            has_earlier: false,
        }
    }

    async fn get(&self, id: &ConversationId) -> Result<Conversation, WireError> {
        let path = self.locate(id).ok_or(WireError::NotFound {
            kind: EntityKind::Conversation,
        })?;

        let binding = self.bindings().get(id).cloned();

        self.describe(path, binding)
            .await
            .ok_or(WireError::NotFound {
                kind: EntityKind::Conversation,
            })
    }

    async fn transcript(
        &self,
        id: &ConversationId,
        before: Option<Cursor>,
        limit: u16,
    ) -> Result<Page<Turn>, WireError> {
        self.with_reader(id, move |reader| reader.page(before.as_ref(), limit))
            .await
    }

    /// Where the stream starts, and everything after it.
    ///
    /// The turns between the client's cursor and the tail are replayed into this
    /// subscriber's own channel before the tail is joined. Without the replay a
    /// reconnecting client is told there is no gap and then never receives what
    /// it was missing; without a channel of its own, one client's history would
    /// reach every other watcher of the same conversation as duplicates.
    async fn subscribe(
        &self,
        id: &ConversationId,
        after: Option<Cursor>,
    ) -> Result<(Cursor, broadcast::Receiver<WatchEvent>), WireError> {
        let path = self.locate(id).ok_or(WireError::NotFound {
            kind: EntityKind::Conversation,
        })?;

        let asked = after.clone();
        let from = self
            .with_reader(id, move |reader| reader.open_from(asked.as_ref()))
            .await?;

        let replayed = match &after {
            Some(_) => {
                let since = from.clone();

                self.with_reader(id, move |reader| reader.turns_after(&since))
                    .await?
            }
            None => Vec::new(),
        };

        let (mine, receiver) = broadcast::channel(TranscriptWatcher::CAPACITY);

        for turn in replayed {
            let _ = mine.send(WatchEvent::Turn(turn));
        }

        // Whether this conversation is waiting on a person is watched
        // separately from its turns, and into this subscriber's own channel.
        // Half of what an agent asks is drawn on screen and never written to the
        // records, so the tail cannot see it — and one watcher per source would
        // send two `Blocked` events for the one question a harness draws both
        // ways.
        self.blocks().publish(id.clone(), mine.clone());

        let mut shared = self.watcher.subscribe(id, path, from.clone());

        tokio::spawn(async move {
            while let Ok(event) = shared.recv().await {
                if mine.send(event).is_err() {
                    return;
                }
            }
        });

        Ok((from, receiver))
    }

    async fn preview(
        &self,
        profile: &ProfileId,
        cwd: &str,
        workspace: Option<&WorkspaceId>,
    ) -> Result<ConversationPreview, WireError> {
        let named = Agent::ALL
            .iter()
            .map(|agent| agent.profile())
            .find(|candidate| candidate.id == *profile)
            .ok_or(WireError::NotFound {
                kind: EntityKind::Conversation,
            })?;

        Ok(ConversationPreview {
            workspace_label: std::path::Path::new(cwd)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| cwd.to_string()),
            tab_label: named.id.as_str().to_string(),
            creates_workspace: workspace.is_none(),
            // The pre-start counterpart of `has_transcript`, and the answer a
            // person should have before they commit rather than after.
            will_have_transcript: named.provides_transcript,
        })
    }

    /// Opens a pane, launches the profile's agent in it, and answers the
    /// conversation that agent began.
    ///
    /// The order is what makes it safe. Everything a client sent is validated
    /// against what this machine publishes *before* a pane exists — an unknown
    /// profile and a working directory that is not a directory here both refuse
    /// without starting anything. The launch line that follows then carries
    /// nothing a client sent at all, which matters because a launch line is
    /// typed at a shell.
    ///
    /// The prompt is delivered after the agent owns the keyboard, never as an
    /// argument on that line.
    async fn start(
        &self,
        profile: &ProfileId,
        cwd: &str,
        prompt: Option<&str>,
        attachments: &[AssetId],
    ) -> Result<Conversation, WireError> {
        let agent = Self::launchable(profile)?;
        let cwd = Self::directory_on_this_machine(cwd).await?;

        let spawn = AgentSpawn::new(agent, cwd.clone(), None);
        let pane = self.terminals.open(None, Some(&cwd)).await?;

        // A pane that opened and an agent that would not start leaves a bare
        // shell nobody asked for, so the pane goes with the failure. A start
        // that succeeded and announced nothing is the opposite case: the agent
        // is running and the pane is the only way to reach it.
        let announced = match self.terminals.start_agent(&pane.id, &spawn).await {
            Ok(announced) => announced,
            Err(error) => {
                self.discard(&pane.id).await;

                return Err(error);
            }
        };

        let id = match announced {
            Some(id) => id,
            None => return Err(Self::began_no_session(&pane.id)),
        };

        // The files go beside the session before the prompt names them, and a
        // failure to place one fails the prompt rather than the start: the pane
        // is up and the agent is running either way, and the person can say it
        // again.
        // Best effort here, unlike `send_prompt`. A failed send is recoverable —
        // the text is still in the box and the person says it again — but a
        // failed start would kill a pane that came up fine and an agent that is
        // already running, to report a problem with a file.
        //
        // Said out loud rather than swallowed. The prompt names what did not go,
        // so the person and the agent learn the same thing in the same place
        // rather than the agent being told about files it will not find.
        let placed = self.deliver(attachments).await;

        let missing = match &placed {
            Ok(_) => 0,
            Err(error) => {
                tracing::warn!(?error, "could not place the attachments for a new conversation");

                attachments.len()
            }
        };

        let delivered = placed.unwrap_or_default();

        let spoken = prompt
            .map(|text| Self::naming(text, &delivered))
            .map(|text| Self::regretting(&text, missing))
            .filter(|text| !text.trim().is_empty());

        if let Some(spoken) = spoken {
            self.terminals.submit_prompt(&pane.id, &spoken).await?;
        }

        self.started(id, agent, &pane, cwd).await
    }

    /// Puts a conversation back in front of a person, in a pane of its own.
    ///
    /// A session id survives its pane, so this costs nothing that a start does
    /// not: the harness is asked to pick up the same records it wrote, and the
    /// conversation keeps the id it always had — measured, and the reason a
    /// resumed conversation's transcript, cursors and watch all keep working
    /// rather than starting again beside the old ones.
    ///
    /// Resuming one that is already running is **not** done twice. Two agents
    /// appending to one set of records would corrupt the history this whole
    /// surface reads, so a conversation that already has a pane is answered with
    /// that pane.
    async fn resume(
        &self,
        id: &ConversationId,
        cwd: Option<&str>,
    ) -> Result<Conversation, WireError> {
        let existing = self.get(id).await?;

        if existing.binding.is_some() {
            return Ok(existing);
        }

        let session = Self::session_of(id).ok_or(WireError::NotFound {
            kind: EntityKind::Conversation,
        })?;

        // The recorded directory unless the caller named another. A harness
        // indexes its sessions per directory, so resuming somewhere else is a
        // resume the harness will not find - the caller's choice to make, not a
        // default to guess at.
        let cwd = Self::directory_on_this_machine(cwd.unwrap_or(&existing.cwd)).await?;

        // A pane binding is not the only way this conversation can already be
        // running. A backend routinely reports a live agent it has no session
        // identity for, and nothing on this machine can say whether that agent
        // is this conversation — which is a reason to refuse rather than a
        // reason to proceed. A second one would put two agents on one set of
        // records, and the interleaved history is what every other screen reads
        // from.
        //
        // After the directory is settled, so a resume that cannot happen at all
        // is refused by the cheaper and more specific check first, and so the
        // directory compared against is the one the agent would really run in.
        //
        // Read fresh rather than from the last tree render: this is a write, and
        // a render from thirty seconds ago is thirty seconds of agents starting
        // and stopping.
        self.terminals.tree().await?;

        if !ResumeGate::admits(&cwd, existing.title.as_deref(), &self.terminals.panes()) {
            return Err(WireError::Backend {
                message: format!(
                    "an agent is running in {cwd} that this machine cannot identify, and it may \
                     be this conversation; resuming it could put a second agent on the same \
                     history. open that pane to see what it is, or close it and try again"
                ),
            });
        }

        let spawn = AgentSpawn::resuming(Self::READS, cwd.clone(), session);
        let pane = self.terminals.open(None, Some(&cwd)).await?;

        let announced = match self.terminals.start_agent(&pane.id, &spawn).await {
            Ok(announced) => announced,
            Err(error) => {
                self.discard(&pane.id).await;

                return Err(error);
            }
        };

        // A harness that picked up a different session than it was asked for has
        // put something else in front of the person. Saying so beats returning
        // the id that was asked for, which would name a conversation this pane
        // is not running.
        if let Some(running) = &announced {
            if running != id {
                tracing::warn!(
                    asked = id.as_str(),
                    running = running.as_str(),
                    "the harness resumed a different session than the one requested"
                );
            }
        }

        self.started(announced.unwrap_or_else(|| id.clone()), Self::READS, &pane, cwd)
            .await
    }

    /// Types a person's message into the pane a conversation is running in.
    ///
    /// The binding is re-read from the backend rather than taken from the last
    /// tree render. A write goes to a real pane, and a pane that closed since the
    /// last read is a message typed at whatever is there now.
    async fn send_prompt(
        &self,
        id: &ConversationId,
        text: &str,
        attachments: &[AssetId],
    ) -> Result<(), WireError> {
        // A prompt with neither words nor files is an empty turn: it costs the
        // person a round trip and tells the agent nothing.
        if text.trim().is_empty() && attachments.is_empty() {
            return Err(WireError::Backend {
                message: "a prompt with nothing in it is not sent".to_string(),
            });
        }

        let pane = self.running_in(id).await?;

        // Resolved before the prompt goes, so an id this machine cannot place
        // refuses the whole send rather than reaching the agent as a sentence
        // naming a file that is not there.
        let delivered = self.deliver(attachments).await?;

        self.terminals
            .submit_prompt(&pane, &Self::naming(text, &delivered))
            .await
    }

    /// Stops what the agent is doing, without ending it.
    ///
    /// Escape rather than Ctrl-C. Escape is the harness's own "stop this turn"
    /// and leaves the session running with its history intact; Ctrl-C is a signal
    /// to the process, and on an agent that does not trap it that ends the
    /// session — which is `stop`, a different request that this must not become
    /// by accident.
    async fn interrupt(&self, id: &ConversationId) -> Result<(), WireError> {
        let pane = self.running_in(id).await?;

        self.terminals
            .send_key(&pane, Key::Escape, Mods::NONE)
            .await
    }

    async fn stop(&self, _id: &ConversationId) -> Result<(), WireError> {
        Self::unavailable()
    }

    /// Answers the whole set, by driving the picker the agent has on screen.
    ///
    /// The fingerprint is checked against the set as it is **now**, not as the
    /// client last saw it. A person works through a set and reviews it before
    /// sending, so there is a real window in which the agent can move on or
    /// somebody can answer at the machine — and pressing keys into a picker that
    /// has changed underneath would answer a different question than the one on
    /// the phone.
    ///
    /// Every press is computed before any of them is sent, so a set this machine
    /// cannot express is refused with the picker untouched rather than left
    /// half-driven.
    async fn answer(
        &self,
        id: &ConversationId,
        question: &QuestionId,
        fingerprint: &Fingerprint,
        answers: &[Answer],
    ) -> Result<(), WireError> {
        // Logged on the way in, because until now nothing anywhere recorded
        // that an answer had arrived. A person pressing send and seeing nothing
        // happen left no trace on either side of the wire, so the first question
        // to ask - did it reach the machine at all - had no answer.
        tracing::info!(
            conversation = id.as_str(),
            question = question.as_str(),
            answers = answers.len(),
            // The shape, not just the count. A typed answer and a chosen row are
            // delivered by different keys onto different rows, so a report that
            // one behaved like the other cannot be checked against a log that
            // does not say which arrived.
            shapes = Self::shapes_of(answers),
            "an answer arrived"
        );

        let pending = self.pending_question(id).await.inspect_err(|error| {
            tracing::info!(
                ?error,
                conversation = id.as_str(),
                question = question.as_str(),
                "an answer arrived for a question this machine could not produce"
            );
        })?;

        if pending.id != *question || pending.fingerprint != *fingerprint {
            // Both halves named. A stale answer is either aimed at a question
            // that has since been replaced, or at the same question after its
            // text moved - and the two are different bugs entirely.
            tracing::info!(
                conversation = id.as_str(),
                sent_question = question.as_str(),
                live_question = pending.id.as_str(),
                same_question = pending.id == *question,
                same_fingerprint = pending.fingerprint == *fingerprint,
                "an answer was refused as stale"
            );

            return Err(WireError::Stale);
        }

        let steps = Picker::steps(&pending.asks, answers)?;
        let pane = self.running_in(id).await?;

        for step in steps {
            match step {
                TerminalInput::Key { key, mods } => {
                    self.terminals.send_key(&pane, key, mods).await?
                }
                TerminalInput::Text(text) => self.terminals.send_text(&pane, &text).await?,
            }
        }

        // A set of several ends on a review screen; a single question submits
        // the moment its row is pressed. Measured on a live harness rather than
        // predicted, and the count is what separates them - looking for a review
        // after a single answer waits for a screen that is never coming.
        if pending.asks.len() > 1 {
            self.submit_review(&pane).await?;
        }

        tracing::info!(
            conversation = id.as_str(),
            question = question.as_str(),
            pane = pane.as_str(),
            "an answer was typed into the pane"
        );

        self.confirm_taken(&pane, &pending).await
    }

}
