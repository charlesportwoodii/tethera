mod herdr_config;
mod terminal_kind;

pub use herdr_config::HerdrConfig;
pub use terminal_kind::TerminalKind;

use directories::ProjectDirs;
use std::fmt;
use std::path::PathBuf;
use tethera_common::structs::terminal::Size;

#[derive(Clone)]
pub struct ApplicationConfig {
    pub data_dir: PathBuf,
    pub relay_url: Option<String>,
    pub relay_token: Option<String>,
    // What a pairing screen calls this machine. Absent rather than defaulted:
    // a guessed name shown on a phone beside two other machines is worse than
    // no name, because the person cannot tell which one they are looking at.
    pub label: Option<String>,
    // What to run to reach the terminal backend, and the geometry a pane gets
    // when the backend's own layout does not describe one. Neither is a secret.
    pub herdr_binary: String,
    pub terminal_size: Size,
    // Which backend drives panes. Only the pty backend can be attached to,
    // because herdr publishes no per-pane byte stream.
    pub terminal_backend: TerminalKind,
    pub max_connections: usize,
    /// The UDP port this machine's endpoint binds.
    ///
    /// Fixed rather than ephemeral so a router forward can name it. A forward is
    /// what lets a phone on a mobile network punch a direct path in; without one
    /// every connection from outside the network is carried by a relay.
    pub bind_port: u16,
}

impl ApplicationConfig {
    /// A memory bound, not an admission policy.
    ///
    /// Each connection costs a task, a five-second hello timeout and two
    /// database reads before it can be refused, so an unbounded accept loop is
    /// a denial of service against the operator's own machine. It is set high
    /// on purpose: a refused peer holds its slot until it hangs up, and the
    /// enrolment loop has no read deadline, so a low bound would let a handful
    /// of stalled strangers lock the operator out of pairing their own phone.
    /// Tightening it needs those two deadlines in the dispatcher first.
    pub const DEFAULT_MAX_CONNECTIONS: usize = 256;

    /// The port to forward. Carried over from the predecessor so an operator who
    /// already has a rule for it does not have to write a second one.
    pub const DEFAULT_BIND_PORT: u16 = 23848;

    pub const QUALIFIER: &'static str = "com";
    pub const ORGANIZATION: &'static str = "alaydriem";
    pub const APPLICATION: &'static str = "tethera";
    pub const DEFAULT_HERDR_BINARY: &'static str = "herdr";
    pub const DEFAULT_TERMINAL_SIZE: Size = Size {
        cols: 120,
        rows: 40,
    };

    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            relay_url: None,
            relay_token: None,
            label: None,
            herdr_binary: Self::DEFAULT_HERDR_BINARY.to_string(),
            terminal_size: Self::DEFAULT_TERMINAL_SIZE,
            terminal_backend: TerminalKind::default(),
            max_connections: Self::DEFAULT_MAX_CONNECTIONS,
            bind_port: Self::DEFAULT_BIND_PORT,
        }
    }

    /// A relay URL with its own credentials in it, stripped.
    ///
    /// `iroh-relay` accepts its shared secret as a `?token=` query parameter, so
    /// an operator following that convention would hand this value to
    /// `PairingOffer` — which is rendered as a QR, printed to stdout, written to
    /// `endpoint.json` and logged. `CLAUDE.md` states the rule outright: the
    /// relay URL must never be written down with the secret in it. Nothing
    /// downstream can tell a URL that carries a credential from one that does
    /// not, so it is removed here, once, at the boundary. Returns whether
    /// anything was taken out, so the caller can say so.
    pub fn sanitise_relay_url(raw: &str) -> (String, bool) {
        let without_fragment = raw.split('#').next().unwrap_or(raw);
        let without_query = without_fragment.split('?').next().unwrap_or(without_fragment);

        // Userinfo is the other place a secret hides: https://user:pass@host.
        let cleaned = match without_query.split_once("://") {
            Some((scheme, rest)) => match rest.split_once('/') {
                Some((authority, path)) => match authority.rsplit_once('@') {
                    Some((_, host)) => format!("{scheme}://{host}/{path}"),
                    None => without_query.to_string(),
                },
                None => match rest.rsplit_once('@') {
                    Some((_, host)) => format!("{scheme}://{host}"),
                    None => without_query.to_string(),
                },
            },
            None => without_query.to_string(),
        };

        let removed = cleaned != raw;

        (cleaned, removed)
    }

    pub fn default_data_dir() -> PathBuf {
        ProjectDirs::from(Self::QUALIFIER, Self::ORGANIZATION, Self::APPLICATION)
            .map(|dirs| dirs.data_local_dir().to_path_buf())
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(".tethera")
            })
    }

    pub fn identity_path(&self) -> PathBuf {
        self.data_dir.join("identity.key")
    }

    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join("tethera.sqlite3")
    }

    pub fn pid_path(&self) -> PathBuf {
        self.data_dir.join("tethera.pid")
    }

    // Where the running server records the addresses it is reachable on, for
    // `tethera pair` to read. A separate file rather than a database row: it is
    // rewritten on a timer, and it belongs to the process rather than to the
    // operator's data.
    pub fn endpoint_path(&self) -> PathBuf {
        self.data_dir.join("endpoint.json")
    }

    // mode=rwc so a first run creates the file rather than failing on an
    // absent one. Zero config to start is a requirement, not a convenience.
    pub fn database_url(&self) -> String {
        format!(
            "sqlite://{}?mode=rwc",
            self.database_path().to_string_lossy().replace('\\', "/")
        )
    }

    /// Where rotated JSON logs are written.
    ///
    /// Beside the database rather than in a platform log directory, so an
    /// operator copying `data_dir` for support gets the logs with it.
    pub fn log_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

    pub fn ensure_data_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)
    }
}

// Hand written so a `tracing::debug!(?config)` cannot dump a live credential
// into the log.
impl fmt::Debug for ApplicationConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationConfig")
            .field("data_dir", &self.data_dir)
            .field("label", &self.label)
            .field("relay_url", &self.relay_url)
            .field("relay_token", &self.relay_token.as_ref().map(|_| "<redacted>"))
            .field("herdr_binary", &self.herdr_binary)
            .field("terminal_size", &self.terminal_size)
            .field("terminal_backend", &self.terminal_backend)
            .field("max_connections", &self.max_connections)
            .field("bind_port", &self.bind_port)
            // Non-exhaustive so a field that is deliberately not printed reads
            // as a decision rather than as one somebody forgot to add.
            .finish_non_exhaustive()
    }
}

impl Default for ApplicationConfig {
    fn default() -> Self {
        Self::with_data_dir(Self::default_data_dir())
    }
}
