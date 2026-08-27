use crate::backend::error::BackendError;
use tethera_common::protocol::error::EntityKind;
use tethera_common::structs::ids::{PaneId, TabId, WorkspaceId};

/// Translation between tethera's prefixed ids and herdr's own.
///
/// herdr names a pane `w62:p1`. tethera's ids carry a prefix as part of the
/// value, so the same pane is `pn_w62:p1`. The herdr id rides verbatim in the
/// suffix, colons included, which makes the mapping total and reversible with
/// no table to keep.
///
/// Inbound, the suffix is **validated, not just unwrapped**. It becomes an argv
/// token, and herdr's clap has no `--` escape — `herdr pane get -- w62:p1`
/// consumes the separator as the positional. So `pn_--help` would run
/// `herdr pane close --help`, which prints help and **exits 0**, and a caller
/// would be told a pane was closed when nothing was. Anything that is not
/// shaped like a herdr id is refused here, which is the refusal the prefix rule
/// already exists to make.
pub struct HerdrIds;

impl HerdrIds {
    pub fn workspace(native: &str) -> WorkspaceId {
        WorkspaceId::mint(native)
    }

    pub fn tab(native: &str) -> TabId {
        TabId::mint(native)
    }

    pub fn pane(native: &str) -> PaneId {
        PaneId::mint(native)
    }

    pub fn native_workspace(id: &WorkspaceId) -> Result<&str, BackendError> {
        Self::strip(id.as_str(), WorkspaceId::PREFIX, None, EntityKind::Workspace)
    }

    pub fn native_tab(id: &TabId) -> Result<&str, BackendError> {
        Self::strip(id.as_str(), TabId::PREFIX, Some('t'), EntityKind::Tab)
    }

    pub fn native_pane(id: &PaneId) -> Result<&str, BackendError> {
        Self::strip(id.as_str(), PaneId::PREFIX, Some('p'), EntityKind::Pane)
    }

    /// A path this backend is willing to hand to `--cwd`.
    ///
    /// A value opening with `-` is parsed as a flag rather than as the option's
    /// argument, so it is refused. No real directory starts with one, and a
    /// silent misparse on a create is worse than a refusal a caller can read.
    pub fn cwd(value: &str) -> Result<&str, BackendError> {
        if value.starts_with('-') || value.is_empty() {
            return Err(BackendError::message(format!(
                "a working directory must not be empty or begin with '-': {value:?}"
            )));
        }

        Ok(value)
    }

    /// A workspace label this backend is willing to hand to `--label`.
    pub fn label(value: &str) -> Result<&str, BackendError> {
        if value.starts_with('-') || value.trim().is_empty() {
            return Err(BackendError::message(format!(
                "a workspace name must not be blank or begin with '-': {value:?}"
            )));
        }

        Ok(value)
    }

    /// herdr's own id shape: `w<counter>` for a workspace, and that plus
    /// `:t<counter>` or `:p<counter>` for a tab or a pane.
    ///
    /// The counter is alphanumeric, and no narrower check is safe. It was
    /// written as hexadecimal on the evidence of a session that had reached
    /// `w6A`; a longer-lived one reaches `w6G`, `w6H` and `w6J`, and a hex check
    /// refuses every id in those workspaces — which is not a parse failure a
    /// person can see, but a workspace that quietly stops existing.
    ///
    /// Alphanumeric is enough for what this refusal is for. herdr's clap honours
    /// no `--` separator, so what must never get through is a value that reads
    /// as a flag, and none of those is alphanumeric.
    fn strip<'a>(
        value: &'a str,
        prefix: &str,
        rank: Option<char>,
        kind: EntityKind,
    ) -> Result<&'a str, BackendError> {
        let native = value
            .strip_prefix(prefix)
            .ok_or(BackendError::NotFound { kind })?;

        let (workspace, child) = match native.split_once(':') {
            Some((workspace, child)) => (workspace, Some(child)),
            None => (native, None),
        };

        let shaped = Self::is_workspace(workspace)
            && match (rank, child) {
                (None, None) => true,
                (Some(rank), Some(child)) => Self::is_child(child, rank),
                _ => false,
            };

        if !shaped {
            return Err(BackendError::NotFound { kind });
        }

        Ok(native)
    }

    fn is_workspace(value: &str) -> bool {
        Self::is_counter(value.strip_prefix('w'))
    }

    fn is_child(value: &str, rank: char) -> bool {
        Self::is_counter(value.strip_prefix(rank))
    }

    fn is_counter(value: Option<&str>) -> bool {
        matches!(value, Some(counter)
            if !counter.is_empty() && counter.bytes().all(|b| b.is_ascii_alphanumeric()))
    }
}
