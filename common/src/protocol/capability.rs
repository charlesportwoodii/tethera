use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One named thing a machine can do.
///
/// A string rather than an enum variant, and a set rather than a struct of
/// booleans, so adding a capability needs no wire change at any version. This is
/// the shape every growing list in this protocol takes, for that reason.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export, export_to = "./../../client/src/js/bindings/")]
pub struct CapabilityId(pub String);

impl From<&str> for CapabilityId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

pub type CapabilitySet = BTreeSet<CapabilityId>;

/// Asking a set whether it holds a capability by name.
///
/// An extension trait because `CapabilitySet` is a plain `BTreeSet`, which is
/// what keeps it additive-safe on the wire.
pub trait HasCapability {
    fn has(&self, name: &str) -> bool;
}

impl HasCapability for CapabilitySet {
    fn has(&self, name: &str) -> bool {
        self.contains(&CapabilityId(name.to_owned()))
    }
}

pub const AGENT_CATALOG: &str = "agent_catalog";
pub const CONVERSATION_START: &str = "conversation_start";
pub const CONVERSATION_RESUME: &str = "conversation_resume";
pub const CONVERSATION_STOP: &str = "conversation_stop";
pub const CONVERSATION_PREVIEW: &str = "conversation_preview";
pub const TRANSCRIPT_PAGING: &str = "transcript_paging";
pub const PROMPT_SEND: &str = "prompt_send";
pub const INTERRUPT: &str = "interrupt";
pub const QUESTIONS: &str = "questions";
pub const QUESTIONS_PERMISSION: &str = "questions_permission";
pub const RECENT_CWDS: &str = "recent_cwds";
pub const TERMINAL_ATTACH: &str = "terminal_attach";
pub const TERMINAL_INPUT: &str = "terminal_input";
pub const TERMINAL_SCROLLBACK: &str = "terminal_scrollback";
/// Whether this machine will say where a tab's panes sit.
///
/// A backend that owns no geometry cannot answer it, and a client that asked
/// anyway would draw a map of a layout nobody vouched for.
pub const PANE_LAYOUT: &str = "pane_layout";
/// Whether a client may move the machine's own focus to a tab.
pub const TAB_FOCUS: &str = "tab_focus";
pub const PANE_OPEN: &str = "pane_open";
pub const PANE_SPLIT: &str = "pane_split";
pub const PANE_CLOSE: &str = "pane_close";
pub const ASSETS_READ: &str = "assets_read";
pub const ASSETS_WRITE: &str = "assets_write";
pub const PUSH_FCM: &str = "push_fcm";
pub const NOTIFY_POLICY: &str = "notify_policy";
pub const DEVICE_SELF_REVOKE: &str = "device_self_revoke";
