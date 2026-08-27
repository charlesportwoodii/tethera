//! The ports the running binary serves.
//!
//! Every method answers from what this machine actually has. Where it has
//! nothing yet the answer is `WireError::Backend` naming what is missing, and
//! the capability that would advertise it is absent from `Describe` — an
//! advertised feature that does not work is worse than one that is not there,
//! because a client renders a control for it and a person taps it.

mod assets;
mod blocks;
mod conversations;
mod digests;
mod machine;
mod resume;
mod session;
mod terminals;
mod watcher;

pub use assets::LiveAssets;
pub use blocks::BlockWatch;
pub use conversations::LiveConversations;
pub use digests::AssetDigests;
pub use machine::LiveMachine;
pub use resume::ResumeGate;
pub use session::PaneSession;
pub use terminals::LiveTerminals;
pub use watcher::TreeWatcher;

use crate::config::ApplicationConfig;
use crate::transcript::AssetIndex;
use crate::protocol::ports::{MachinePort, Ports};
use sea_orm::DatabaseConnection;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tethera_common::protocol::capability::CapabilitySet;
use tethera_common::protocol::error::WireError;

pub struct LivePorts {
    machine: LiveMachine,
    conversations: Arc<LiveConversations>,
    terminals: Arc<LiveTerminals>,
    terminals_available: Arc<AtomicBool>,
    assets: LiveAssets,
}

impl LivePorts {
    /// One subprocess call to find out whether the terminal backend is really
    /// there.
    ///
    /// Short: a backend that has not answered in this long is not one a person
    /// waiting on a pairing screen should be waiting for.
    pub const PROBE_DEADLINE: Duration = Duration::from_secs(5);

    /// Builds the four ports, and settles what this machine may advertise.
    ///
    /// The capability set is the union of what each port says it can do, so a
    /// capability arrives with the code that honours it rather than from a list
    /// somebody has to remember to prune. The terminal half is gated on the
    /// backend answering a real call: a machine with no `herdr` on its PATH
    /// would otherwise advertise `pane_open` and refuse every one.
    pub async fn new(
        config: Arc<ApplicationConfig>,
        db: Arc<DatabaseConnection>,
        endpoint_id: String,
    ) -> Self {
        let terminals = LiveTerminals::from_config(&config);
        let assets_index = AssetIndex::new_shared();
        let conversations =
            LiveConversations::new_shared(
                terminals.clone(),
                assets_index.clone(),
                config.data_dir.join("uploads"),
            );
        let terminals_available = Arc::new(AtomicBool::new(false));

        let mut base_capabilities = LiveMachine::own_capabilities();
        base_capabilities.extend(LiveConversations::capabilities());
        base_capabilities.extend(LiveAssets::capabilities());

        let assets = LiveAssets::new(&config, conversations.clone(), assets_index);

        let ports = Self {
            machine: LiveMachine::new(
                config,
                db,
                endpoint_id,
                base_capabilities,
                terminals_available.clone(),
                terminals.clone(),
                conversations.clone(),
            ),
            conversations,
            terminals,
            terminals_available,
            assets,
        };

        ports.reprobe_terminals().await;

        ports
    }

    pub async fn new_shared(
        config: Arc<ApplicationConfig>,
        db: Arc<DatabaseConnection>,
        endpoint_id: String,
    ) -> Arc<Self> {
        Arc::new(Self::new(config, db, endpoint_id).await)
    }

    /// What this machine will tell a client it can do, as of now.
    pub fn capabilities(&self) -> CapabilitySet {
        self.machine.capabilities()
    }

    /// Asks the terminal backend whether it is there, and records the answer.
    ///
    /// Called at construction and again on the runtime's heartbeat, because a
    /// backend can appear or die under a running server. An answer frozen at
    /// startup leaves a machine advertising panes it can no longer open, which
    /// is the failure a capability set exists to prevent.
    pub async fn reprobe_terminals(&self) {
        let answered = Self::terminal_backend_answers(&self.terminals).await;

        if self.terminals_available.swap(answered, Ordering::SeqCst) != answered {
            tracing::info!(
                available = answered,
                "the terminal backend's availability changed; pane capabilities follow it"
            );
        }

        // Reading the whole tree is also what publishes changes to anything
        // watching, because `LiveMachine::tree` diffs each read against the last.
        // Without this a client that opens a machine watch and waits receives its
        // opening frame and then nothing, however the tree changes: every other
        // caller of `tree` is a client RPC, and that client already has the
        // answer in its response.
        //
        // A second read of the backend rather than reusing the probe's, because
        // the probe deliberately treats `Busy` as present and must not publish a
        // tree it did not get. This is the heartbeat, not a hot path.
        if answered {
            let _ = self.machine.tree().await;
        }
    }

    /// Re-reads the tree, but only while somebody is watching it.
    ///
    /// **A machine watch publishes what a tree read diffs**, so how fresh a
    /// conversation's status is on a phone is exactly how often this runs.
    /// Reading it on the address heartbeat meant thirty seconds — far too slow
    /// for a mark that says whether an agent is working right now, and a client
    /// that wanted better had no choice but to poll. A poll costs a full listing
    /// per client per tick, and every client pays it separately.
    ///
    /// This costs one read shared by everybody, and **nothing at all when nobody
    /// is watching** — which is the state a machine spends most of its life in,
    /// and the reason a short interval is affordable here where a timer that ran
    /// regardless would not be.
    ///
    /// Answers whether it read anything, which is the only way to observe the
    /// gate: the runtime ignores it, and a test cannot otherwise tell a machine
    /// that did no work from one whose work produced no events.
    pub async fn refresh_watched_tree(&self) -> bool {
        if self.machine.watchers() == 0 {
            return false;
        }

        let started = std::time::Instant::now();
        let read = self.machine.tree().await;
        let elapsed = started.elapsed();

        // Timed because the interval is only defensible while this stays small,
        // and nothing else would ever say otherwise: a read that quietly grew
        // past its own tick would show up as a machine that felt slow.
        if elapsed >= Self::SLOW_TREE {
            tracing::warn!(
                ms = elapsed.as_millis(),
                watchers = self.machine.watchers(),
                "a tree read is taking longer than the interval it runs on"
            );
        } else {
            tracing::debug!(ms = elapsed.as_millis(), "refreshed the tree for a watcher");
        }

        if let Err(error) = read {
            tracing::debug!(?error, "a watched tree read did not answer");
        }

        true
    }

    /// When a tree read stops being cheap enough to run on a short timer.
    const SLOW_TREE: Duration = Duration::from_secs(2);

    async fn terminal_backend_answers(terminals: &Arc<LiveTerminals>) -> bool {
        match tokio::time::timeout(Self::PROBE_DEADLINE, terminals.tree()).await {
            Ok(Ok(_)) => true,
            // The backend answered and is occupied, which is a backend that is
            // there. Reading Busy as absent would drop every pane capability
            // because one probe happened to arrive while a call held the link.
            Ok(Err(WireError::Busy)) => true,
            Ok(Err(error)) => {
                tracing::debug!(?error, "the terminal backend did not answer");

                false
            }
            Err(_) => {
                tracing::debug!("the terminal backend did not answer within the probe deadline");

                false
            }
        }
    }
}

impl Ports for LivePorts {
    type Machine = LiveMachine;
    type Conversations = LiveConversations;
    type Terminals = LiveTerminals;
    type Assets = LiveAssets;

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
