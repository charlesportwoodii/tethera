use crate::protocol::attach::Attach;
use crate::protocol::ports::{EnrollOffer, Enrolment, MachinePort, Ports};
use crate::protocol::rpc::Rpc;
use crate::protocol::transfer::Transfer;
use crate::protocol::watch::Watch;
use iroh::endpoint::Connection;
use std::sync::Arc;
use std::time::Duration;
use tethera_common::protocol::handshake::{
    ClientHello, CodeFormat, DeviceRecord, EnrollCode, EnrollResult, Intent, RefuseReason,
    ServerHello,
};
use tethera_common::protocol::stream::StreamOpen;
use tethera_common::protocol::WireVersion;
use tethera_common::structs::ids::RequestId;
use tethera_transport::error::TransportError;
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::FrameIo;

/// What the handshake decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandshakeOutcome {
    Session {
        device: DeviceRecord,
        version: WireVersion,
    },
    EnrollPending {
        offer: EnrollOffer,
        version: WireVersion,
    },
    Refuse(RefuseReason),
}

/// A connection that has completed its handshake.
#[derive(Debug, Clone)]
pub struct Session {
    pub device: DeviceRecord,
    pub version: WireVersion,
}

pub struct Dispatcher<P: Ports> {
    ports: Arc<P>,
    codec: FrameCodec,
}

impl<P: Ports> Dispatcher<P> {
    /// A connection that never declares itself is not a connection.
    pub const HELLO_TIMEOUT: Duration = Duration::from_secs(5);

    pub fn new(ports: Arc<P>) -> Self {
        Self {
            ports,
            codec: FrameCodec::default(),
        }
    }

    /// The whole handshake decision, as a function of its inputs.
    ///
    /// An associated function taking plain values rather than a method that
    /// reads the database, so every case below is testable with no fixture and
    /// every branch is visible in one signature.
    ///
    /// **Version is checked first.** A version problem must never read as an
    /// authorisation one: the client can then tell a person to update rather
    /// than to re-pair, which is the opposite instruction.
    pub fn decide(
        enrolment: &Enrolment,
        window: Option<EnrollOffer>,
        intent: Intent,
        client_versions: &[WireVersion],
    ) -> HandshakeOutcome {
        let Some(version) = WireVersion::negotiate(WireVersion::SUPPORTED, client_versions) else {
            return HandshakeOutcome::Refuse(RefuseReason::NoCommonVersion);
        };

        match (enrolment, intent) {
            // Revoked is distinct from unknown. A revoked device that could
            // re-enrol by presenting itself as a stranger would make revocation
            // cosmetic.
            (Enrolment::Revoked, _) => HandshakeOutcome::Refuse(RefuseReason::Revoked),
            (Enrolment::Known(device), _) => HandshakeOutcome::Session {
                device: device.clone(),
                version,
            },
            (Enrolment::Unknown, Intent::Session) => {
                HandshakeOutcome::Refuse(RefuseReason::NotEnrolled)
            }
            (Enrolment::Unknown, Intent::Enroll) => match window {
                Some(offer) => HandshakeOutcome::EnrollPending { offer, version },
                None => HandshakeOutcome::Refuse(RefuseReason::PairingWindowClosed),
            },
        }
    }

    /// Accepts, handshakes, then serves every stream the peer opens.
    pub async fn serve_connection(&self, connection: Connection) -> Result<(), TransportError> {
        // Straight from the peer's TLS certificate, so this is the
        // authentication rather than a claim the peer made about itself.
        let endpoint_id = connection.remote_id().to_string();

        let session = match self.handshake(&connection, &endpoint_id).await? {
            Some(session) => session,
            // Refused. The answer has been written, but dropping the connection
            // here would close it before the peer has read the frame, and the
            // client would see a lost connection instead of the reason it was
            // turned away. Waiting for the peer to hang up is what makes a
            // refusal legible.
            None => {
                connection.closed().await;

                return Ok(());
            }
        };

        let session = Arc::new(session);

        loop {
            let (send, recv) = match connection.accept_bi().await {
                Ok(streams) => streams,
                // The peer went away. That is how a connection ends, not a
                // failure to report.
                Err(_) => return Ok(()),
            };

            // Spawned, never awaited. A `Watch` or an `Attach` is long-lived by
            // design, so serving it inline would park the accept loop and every
            // later stream on this connection would stall behind it. That is the
            // predecessor's one-connection-at-a-time failure reproduced one
            // level down, and it looks like a hang rather than an error.
            let ports = self.ports.clone();
            let session = session.clone();

            tokio::spawn(async move {
                let codec = FrameCodec::default();
                let mut recv = recv;

                let open: Option<StreamOpen> = match FrameIo::read(&mut recv, &codec).await {
                    Ok(open) => open,
                    Err(_) => return,
                };

                let Some(open) = open else {
                    return;
                };

                let _ = Self::serve_stream(&ports, &codec, open, send, recv, &session).await;
            });
        }
    }

    async fn serve_stream(
        ports: &Arc<P>,
        codec: &FrameCodec,
        open: StreamOpen,
        send: iroh::endpoint::SendStream,
        recv: iroh::endpoint::RecvStream,
        session: &Session,
    ) -> Result<(), TransportError> {
        match open {
            // A second hello on an established connection is a confused peer.
            // Refusing it beats re-running a handshake that already succeeded.
            StreamOpen::Hello(_) => Ok(()),
            StreamOpen::Rpc(request) => {
                Rpc::serve(ports.as_ref(), codec, request, send, session).await
            }
            StreamOpen::Watch(spec) => {
                Watch::serve(ports.as_ref(), codec, spec, send, recv).await
            }
            StreamOpen::Attach(spec) => {
                Attach::serve(ports.as_ref(), codec, spec, send, recv).await
            }
            StreamOpen::Fetch(spec) => {
                Transfer::fetch(ports.as_ref(), codec, spec, send).await
            }
            StreamOpen::Put(spec) => {
                Transfer::put(ports.as_ref(), codec, spec, send, recv).await
            }
        }
    }

    /// `None` when the connection was refused and nothing further should be
    /// served on it.
    async fn handshake(
        &self,
        connection: &Connection,
        endpoint_id: &str,
    ) -> Result<Option<Session>, TransportError> {
        let (mut send, mut recv) = tokio::time::timeout(Self::HELLO_TIMEOUT, connection.accept_bi())
            .await
            .map_err(|_| TransportError::Connection("hello timed out".to_string()))?
            .map_err(|error| TransportError::Connection(error.to_string()))?;

        let open: Option<StreamOpen> = FrameIo::read(&mut recv, &self.codec).await?;

        let Some(StreamOpen::Hello(hello)) = open else {
            return Err(TransportError::Connection(
                "the first stream must be a hello".to_string(),
            ));
        };

        let enrolment = self.ports.machine().enrolment(endpoint_id).await;
        let window = self.ports.machine().pairing_window().await;
        let outcome = Self::decide(&enrolment, window, hello.intent, &hello.versions);

        match outcome {
            HandshakeOutcome::Session { device, version } => {
                self.write_session(&mut send, &device, version).await?;
                send.finish().ok();

                Ok(Some(Session { device, version }))
            }
            HandshakeOutcome::Refuse(reason) => {
                FrameIo::write(&mut send, &self.codec, &ServerHello::Refuse(reason)).await?;
                send.finish().ok();

                Ok(None)
            }
            HandshakeOutcome::EnrollPending { offer, version } => {
                self.enroll(&mut send, &mut recv, endpoint_id, offer, version, &hello)
                    .await
            }
        }
    }

    async fn write_session(
        &self,
        send: &mut iroh::endpoint::SendStream,
        device: &DeviceRecord,
        version: WireVersion,
    ) -> Result<(), TransportError> {
        let describe = self.ports.machine().describe().await;

        FrameIo::write(
            send,
            &self.codec,
            &ServerHello::Session {
                version,
                server: describe.server,
                capabilities: describe.capabilities,
                device: device.clone(),
            },
        )
        .await
    }

    async fn enroll(
        &self,
        send: &mut iroh::endpoint::SendStream,
        recv: &mut iroh::endpoint::RecvStream,
        endpoint_id: &str,
        offer: EnrollOffer,
        version: WireVersion,
        _hello: &ClientHello,
    ) -> Result<Option<Session>, TransportError> {
        FrameIo::write(
            send,
            &self.codec,
            &ServerHello::EnrollPending {
                request_id: RequestId(endpoint_id.to_string()),
                expires_in_ms: offer.expires_in_ms,
                server: offer.server.clone(),
                code_length: offer.code_length,
                code_format: CodeFormat::Digits,
            },
        )
        .await?;

        // A retry loop, because `attempts_left` is otherwise a lie: the client
        // is told it may try again, so the stream has to stay open long enough
        // for it to. A person mistyping a six-digit code is the common case, not
        // the exceptional one.
        loop {
            let typed: Option<EnrollCode> = FrameIo::read(recv, &self.codec).await?;

            let Some(typed) = typed else {
                return Ok(None);
            };

            let describe = self.ports.machine().describe().await;

            let result = self
                .ports
                .machine()
                .redeem_code(
                    endpoint_id,
                    &tethera_common::protocol::handshake::Handshake::normalize_code(&typed.code),
                    &typed.device_name,
                )
                .await;

            match result {
                Ok(device) => {
                    FrameIo::write(
                        send,
                        &self.codec,
                        &EnrollResult::Accepted {
                            device: device.clone(),
                            version,
                            server: describe.server,
                            capabilities: describe.capabilities,
                        },
                    )
                    .await?;
                    send.finish().ok();

                    return Ok(Some(Session { device, version }));
                }
                Err(attempts_left) => {
                    FrameIo::write(
                        send,
                        &self.codec,
                        &EnrollResult::Refused {
                            reason: RefuseReason::NotEnrolled,
                            attempts_left,
                        },
                    )
                    .await?;

                    // Out of attempts: the offer is spent, and a stream that
                    // stayed open would invite an unbounded guessing loop
                    // against a six-digit code.
                    if attempts_left == 0 {
                        send.finish().ok();

                        return Ok(None);
                    }
                }
            }
        }
    }
}
