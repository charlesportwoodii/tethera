use std::io::{Read, Write};
use std::path::PathBuf;

/// One message on the downlink.
///
/// The uplink stays a raw byte stream — it is high volume and carries exactly
/// one kind of thing. The downlink carries two, so it is framed: keystrokes for
/// the shell, and the geometry the pane is to be held at.
///
/// `[tag:1][len:4 big endian][payload]`. Length-prefixed for the same reason the
/// wire codec is: a stream has no message boundaries, and a reader that guessed
/// them from chunk arrival would work in testing and split a resize in half
/// under load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Downlink {
    /// Bytes for the shell, already encoded by the server's key table.
    Input(Vec<u8>),
    /// Hold the pty at this size until told otherwise.
    ///
    /// Not advice. A pane whose geometry a phone has claimed stays claimed after
    /// the phone goes away, because this is a handoff rather than a shared view:
    /// somebody who locked their phone mid-command has not stopped caring what
    /// width it was laid out for.
    Resize { cols: u16, rows: u16 },
}

impl Downlink {
    pub const INPUT: u8 = 0;
    pub const RESIZE: u8 = 1;
    pub const HEADER_BYTES: usize = 5;

    /// Refused rather than truncated, as `FrameCodec` does. The far end of this
    /// channel is another process, and a hostile or confused length must not
    /// make this one allocate for it.
    pub const MAX_PAYLOAD: usize = 64 * 1024;

    pub fn encode(&self) -> Vec<u8> {
        let (tag, payload) = match self {
            Self::Input(bytes) => (Self::INPUT, bytes.clone()),
            Self::Resize { cols, rows } => {
                let mut payload = Vec::with_capacity(4);
                payload.extend_from_slice(&cols.to_be_bytes());
                payload.extend_from_slice(&rows.to_be_bytes());

                (Self::RESIZE, payload)
            }
        };

        let mut out = Vec::with_capacity(Self::HEADER_BYTES + payload.len());
        out.push(tag);
        out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        out.extend_from_slice(&payload);

        out
    }

    /// The message a header and its body describe.
    ///
    /// An unknown tag is `None` rather than an error: this channel is two halves
    /// of one binary today, but a shim that outlived an upgrade should skip a
    /// message it does not know rather than tear down a working pane.
    pub fn decode(tag: u8, payload: &[u8]) -> Option<Self> {
        match tag {
            Self::INPUT => Some(Self::Input(payload.to_vec())),
            Self::RESIZE if payload.len() >= 4 => Some(Self::Resize {
                cols: u16::from_be_bytes([payload[0], payload[1]]),
                rows: u16::from_be_bytes([payload[2], payload[3]]),
            }),
            _ => None,
        }
    }

    /// The payload length a header names, refused if it is past the cap.
    pub fn payload_length(header: [u8; Self::HEADER_BYTES]) -> Option<usize> {
        let len = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;

        (len <= Self::MAX_PAYLOAD).then_some(len)
    }
}

/// One message on the uplink.
///
/// Framed for the same reason the downlink is: this direction carries the pty's
/// bytes *and* the geometry the desk resized the pane to, and a stream has no
/// message boundaries to tell them apart. Without it the server's emulator keeps
/// the geometry from the greeting for the pane's whole life, because the shim
/// resizes its pty when the desk changes and has no way to say so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Uplink {
    /// The pty's output, exactly as the program produced it.
    ///
    /// Unfiltered, unlike what reaches the pane: the server is not a guest in
    /// anybody's terminal, so a resize request in the stream is information
    /// rather than a hazard.
    Output(Vec<u8>),
    /// The pty is now this size, because the terminal the shim sits in is.
    ///
    /// An observation, not a request. A claim travels the other way.
    Resized { cols: u16, rows: u16 },
}

impl Uplink {
    pub const OUTPUT: u8 = 0;
    pub const RESIZED: u8 = 1;
    pub const HEADER_BYTES: usize = 5;

    /// Larger than the downlink's, because this carries screenfuls rather than
    /// keystrokes. Still bounded: the far end is another process.
    pub const MAX_PAYLOAD: usize = 256 * 1024;

    pub fn encode(&self) -> Vec<u8> {
        let (tag, payload) = match self {
            Self::Output(bytes) => (Self::OUTPUT, bytes.clone()),
            Self::Resized { cols, rows } => {
                let mut payload = Vec::with_capacity(4);
                payload.extend_from_slice(&cols.to_be_bytes());
                payload.extend_from_slice(&rows.to_be_bytes());

                (Self::RESIZED, payload)
            }
        };

        let mut out = Vec::with_capacity(Self::HEADER_BYTES + payload.len());
        out.push(tag);
        out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        out.extend_from_slice(&payload);

        out
    }

    /// An unknown tag is `None` rather than an error, so a peer from a newer
    /// build is skipped rather than fatal.
    pub fn decode(tag: u8, payload: &[u8]) -> Option<Self> {
        match tag {
            Self::OUTPUT => Some(Self::Output(payload.to_vec())),
            Self::RESIZED if payload.len() >= 4 => Some(Self::Resized {
                cols: u16::from_be_bytes([payload[0], payload[1]]),
                rows: u16::from_be_bytes([payload[2], payload[3]]),
            }),
            _ => None,
        }
    }

    /// The payload length a header names, refused if it is past the cap.
    pub fn payload_length(header: [u8; Self::HEADER_BYTES]) -> Option<usize> {
        let len = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;

        (len <= Self::MAX_PAYLOAD).then_some(len)
    }
}

/// The local channel a shim reaches this machine's server on.
///
/// A named pipe on Windows and a unix socket elsewhere, rather than a loopback
/// port. Both carry an OS access check, so only this user's processes can
/// connect; a TCP port on 127.0.0.1 is reachable by every process on the machine
/// and would put a writable channel into every pane's shell behind nothing at
/// all.
///
/// The shim **dials out** and this side listens. That direction is deliberate:
/// the server then discovers panes instead of hunting for them, a pane split by
/// hand at the desk announces itself the same way one tethera opened does, and a
/// shim whose dial fails simply runs the shell — so a stopped server is a plain
/// terminal rather than a broken one.
pub struct ShimLink;

impl ShimLink {
    /// The shim reads this to find the server, so it is derived and never
    /// passed: a pane opened at the desk has no tethera argv to carry it.
    pub const ADDRESS_ENV: &'static str = "TETHERA_SHIM_ADDRESS";

    /// How long a shim waits between dial attempts.
    ///
    /// The server may be stopped when a pane opens and started later, and a pane
    /// that only ever tried once would stay unreadable for its whole life.
    pub const REDIAL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

    /// Where the server listens, unless the environment names somewhere else.
    ///
    /// The default is per-user on both platforms. Two operators on one machine
    /// get two servers and must not get one channel.
    pub fn address(data_dir: &PathBuf) -> String {
        if let Ok(named) = std::env::var(Self::ADDRESS_ENV) {
            if !named.is_empty() {
                return named;
            }
        }

        if cfg!(windows) {
            let user = std::env::var("USERNAME").unwrap_or_else(|_| "default".to_string());

            format!(r"\\.\pipe\tethera-shim-{user}")
        } else {
            data_dir.join("shim.sock").to_string_lossy().into_owned()
        }
    }

    /// Dials the server from a shim, synchronously.
    ///
    /// Synchronous because the shim is threads and no runtime — it exists to
    /// move bytes and starting a reactor to do it would be the largest thing in
    /// the process. On Windows a named pipe opens as a file, which is why this
    /// needs no platform crate on either side.
    /// `ERROR_PIPE_BUSY`. Every instance of the pipe is already connected.
    ///
    /// Expected rather than exceptional: a named pipe server holds a fixed
    /// number of instances and creates the next only after the current one is
    /// handed off, so a client that dials while the server is between the two is
    /// refused. A shim opens two channels back to back and loses that race
    /// routinely — measured, the second dial failed with this while the first
    /// had just succeeded.
    #[cfg(windows)]
    const PIPE_BUSY: i32 = 231;

    /// How long a dial keeps retrying a busy pipe, and how often.
    ///
    /// Short, because this covers a server between instances rather than a
    /// server that is down. A shim that cannot get through gives up and runs the
    /// shell, and `REDIAL_INTERVAL` is what brings it back later.
    #[cfg(windows)]
    const BUSY_ATTEMPTS: u32 = 20;
    #[cfg(windows)]
    const BUSY_INTERVAL: std::time::Duration = std::time::Duration::from_millis(25);

    #[cfg(windows)]
    pub fn dial(address: &str) -> std::io::Result<(Box<dyn Read + Send>, Box<dyn Write + Send>)> {
        use std::fs::OpenOptions;

        let mut attempt = 0;

        let pipe = loop {
            match OpenOptions::new().read(true).write(true).open(address) {
                Ok(pipe) => break pipe,
                Err(error)
                    if error.raw_os_error() == Some(Self::PIPE_BUSY)
                        && attempt < Self::BUSY_ATTEMPTS =>
                {
                    attempt += 1;
                    std::thread::sleep(Self::BUSY_INTERVAL);
                }
                Err(error) => return Err(error),
            }
        };

        let write = pipe.try_clone()?;

        Ok((Box::new(pipe), Box::new(write)))
    }

    #[cfg(unix)]
    pub fn dial(address: &str) -> std::io::Result<(Box<dyn Read + Send>, Box<dyn Write + Send>)> {
        use std::os::unix::net::UnixStream;

        let stream = UnixStream::connect(address)?;
        let write = stream.try_clone()?;

        Ok((Box::new(stream), Box::new(write)))
    }
}
