/// Why a connection was closed at the transport level.
///
/// Distinct from `RefuseReason`, which travels in a `ServerHello` and needs a
/// stream to carry it. These are the refusals that happen *before* there is
/// anything to write a frame on — a machine already serving its limit has no
/// handshake to answer with.
///
/// QUIC delivers both the code and the reason bytes to the peer, so a client
/// sees exactly what it was told. It reaches the wire as a QUIC integer rather
/// than inside a frame, which is why nothing here derives `Serialize`: postcard
/// never sees it, and a version bump does not apply.
///
/// An enum rather than a constant on the server, because a bare integer has no
/// owner: a second close code added later could silently reuse the same value,
/// and the client would misreport it with nothing failing to compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseCode {
    /// This machine is already serving as many connections as it will.
    AtCapacity,
}

impl CloseCode {
    pub fn as_u32(&self) -> u32 {
        match self {
            Self::AtCapacity => 1,
        }
    }

    /// `None` for a code this build does not know.
    ///
    /// An unknown code is not a default. A client that mapped one onto
    /// `AtCapacity` would tell a person to try again later about a refusal that
    /// means something else entirely.
    pub fn from_u32(code: u32) -> Option<Self> {
        match code {
            1 => Some(Self::AtCapacity),
            _ => None,
        }
    }

    /// The bytes sent alongside the code, so both halves spell it one way.
    pub fn reason(&self) -> &'static [u8] {
        match self {
            Self::AtCapacity => b"at capacity",
        }
    }
}
