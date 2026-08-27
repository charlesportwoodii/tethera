use super::wire::{Envelope, Failure};
use crate::backend::error::BackendError;
use crate::process::Windowless;
use serde::de::DeserializeOwned;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// The only place this backend spawns a process.
///
/// Keeping the subprocess in one struct is what leaves every parsing type a
/// pure function over a string, and it is why the parsing is tested against
/// committed real output rather than against a mock of this.
pub struct HerdrCommand {
    binary: String,
    deadline: Duration,
}

impl HerdrCommand {
    /// Longer than any healthy herdr call — the slowest observed, a full
    /// `api snapshot` against a seven-workspace session, is a few tens of
    /// milliseconds.
    pub const DEFAULT_DEADLINE: Duration = Duration::from_secs(5);

    pub fn new(binary: String) -> Self {
        Self {
            binary,
            deadline: Self::DEFAULT_DEADLINE,
        }
    }

    pub fn with_deadline(binary: String, deadline: Duration) -> Self {
        Self { binary, deadline }
    }

    pub fn binary(&self) -> &str {
        &self.binary
    }

    /// Runs herdr and returns its stdout.
    ///
    /// The deadline is enforced here, where the child can be killed, rather
    /// than only above in the async layer — a task that gives up on a process
    /// it cannot stop leaves the process running and its admission permit held,
    /// which turns one wedged call into a terminal surface that answers `Busy`
    /// for the life of the server.
    ///
    /// herdr is not consistent about which stream carries a failure: `pane get`
    /// and `pane read` write the error envelope to stdout, and `pane close`
    /// writes it to stderr. Both are searched, or a missing pane reads as an
    /// opaque backend failure rather than as `NotFound`.
    pub fn run(&self, args: &[&str]) -> Result<String, BackendError> {
        let mut command = Command::new(&self.binary);

        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // This is the call that runs on every tree read and every heartbeat, so
        // it is the one that would flash a console window across the operator's
        // desktop several times a minute.
        let mut child = Windowless::apply(&mut command).spawn().map_err(|error| {
            BackendError::message(format!("could not run {}: {error}", self.binary))
        })?;

        let (stdout, stderr) = Self::drain(&mut child)?;
        let status = Self::wait(&mut child, self.deadline, args)?;

        if status {
            return Ok(stdout);
        }

        Err(Self::failure(&stdout, &stderr, args))
    }

    pub fn run_json<T: DeserializeOwned>(&self, args: &[&str]) -> Result<T, BackendError> {
        let raw = self.run(args)?;

        Envelope::<T>::decode(&raw)?.into_result()
    }

    /// Reads both pipes to completion, each on its own thread.
    ///
    /// Draining is not optional and it cannot be interleaved with polling for
    /// exit: a child that fills a pipe buffer blocks writing, and a parent that
    /// is polling rather than reading blocks waiting. `pane read` on a deep
    /// buffer is easily large enough to reach that.
    fn drain(child: &mut Child) -> Result<(String, Vec<u8>), BackendError> {
        let mut out = child
            .stdout
            .take()
            .ok_or_else(|| BackendError::message("herdr was spawned with no stdout"))?;
        let mut err = child
            .stderr
            .take()
            .ok_or_else(|| BackendError::message("herdr was spawned with no stderr"))?;

        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut buffer = Vec::new();
            let _ = err.read_to_end(&mut buffer);
            let _ = tx.send(buffer);
        });

        let mut stdout = Vec::new();
        out.read_to_end(&mut stdout).map_err(|error| {
            BackendError::message(format!("could not read from herdr: {error}"))
        })?;

        Ok((
            String::from_utf8_lossy(&stdout).into_owned(),
            rx.recv().unwrap_or_default(),
        ))
    }

    /// Waits for exit, killing the child when the deadline passes.
    ///
    /// Both pipes are already closed by the time this runs, so the child is
    /// either exiting or genuinely stuck; polling here cannot deadlock.
    fn wait(
        child: &mut Child,
        deadline: Duration,
        args: &[&str],
    ) -> Result<bool, BackendError> {
        const TICK: Duration = Duration::from_millis(10);
        let mut waited = Duration::ZERO;

        loop {
            match child.try_wait() {
                Ok(Some(status)) => return Ok(status.success()),
                Ok(None) => {}
                Err(error) => {
                    return Err(BackendError::message(format!(
                        "could not wait for herdr: {error}"
                    )))
                }
            }

            if waited >= deadline {
                let _ = child.kill();
                let _ = child.wait();

                tracing::warn!(
                    command = args.join(" "),
                    ?deadline,
                    "herdr did not answer before its deadline and was killed"
                );

                return Err(BackendError::Busy);
            }

            std::thread::sleep(TICK);
            waited += TICK;
        }
    }

    fn failure(stdout: &str, stderr: &[u8], args: &[&str]) -> BackendError {
        let stderr = String::from_utf8_lossy(stderr);

        for stream in [stdout, stderr.as_ref()] {
            if let Some(failure) = Self::envelope_error(stream) {
                return failure;
            }
        }

        BackendError::message(format!(
            "herdr {} failed: {}",
            args.join(" "),
            stderr.trim()
        ))
    }

    /// The error out of one stream, when that stream is a herdr envelope.
    fn envelope_error(stream: &str) -> Option<BackendError> {
        let trimmed = stream.trim();

        if trimmed.is_empty() {
            return None;
        }

        serde_json::from_str::<Envelope<serde_json::Value>>(trimmed)
            .ok()?
            .error
            .map(Failure::into_backend)
    }
}
