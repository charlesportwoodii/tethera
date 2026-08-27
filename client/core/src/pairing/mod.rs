use crate::endpoint::ClientEndpoint;
use crate::error::ClientError;
use iroh::endpoint::{Connection, ConnectionError, RecvStream, SendStream};
use std::time::Duration;
use tethera_common::protocol::close::CloseCode;
use tethera_common::protocol::handshake::{
    ClientHello, ClientInfo, EnrollCode, EnrollResult, Handshake, Intent, RefuseReason, ServerHello,
};
use tethera_common::protocol::stream::StreamOpen;
use tethera_common::protocol::WireVersion;
use tethera_common::structs::client::{BeginOutcome, FoundServer, PairOutcome, ServerEntry};
use tethera_common::structs::ids::{RequestId, ServerId};
use tethera_common::structs::pairing::PairingOffer;
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::FrameIo;

/// What `begin` decided, and the live stream when there is one.
pub struct Begun {
    pub outcome: BeginOutcome,
    /// `Some` only for `BeginOutcome::Found`. Every other outcome has already
    /// closed its connection: there is no code to type, and a parked stream
    /// would leave the machine waiting on a frame that never comes.
    pub session: Option<PairingSession>,
}

/// A connection that has been opened and had a hello written on it.
///
/// Bundled so the judging below takes one argument rather than eight.
struct Dialed {
    connection: Connection,
    send: SendStream,
    recv: RecvStream,
    codec: FrameCodec,
    offer: PairingOffer,
    scanned: ServerId,
    endpoint_id: String,
    device_name: String,
}

/// One enrolment attempt, holding its stream open.
///
/// The stream stays open across a person typing six digits because the machine
/// counts attempts against a pairing window. A client that re-dialled for each
/// guess would defeat that budget and turn a six-digit code into an unbounded
/// guessing surface.
pub struct PairingSession {
    send: SendStream,
    recv: RecvStream,
    codec: FrameCodec,
    found: FoundServer,
    request_id: RequestId,
    device_name: String,
    offer: PairingOffer,
    /// Held so the connection outlives the streams. Dropping it resets them,
    /// which is also how cancellation is expressed.
    _connection: Connection,
}

impl PairingSession {
    /// How long to wait for a hung-up connection to report why.
    ///
    /// Bounded because `closed()` on a healthy connection never returns, and a
    /// read failing for some other reason must not park the pairing screen.
    const CLOSE_REASON_TIMEOUT: Duration = Duration::from_secs(2);

    pub async fn begin(
        endpoint: &ClientEndpoint,
        uri: &str,
        client: ClientInfo,
        device_name: String,
    ) -> Result<Begun, ClientError> {
        let offer =
            PairingOffer::from_uri(uri).map_err(|error| ClientError::BadOffer(error.to_string()))?;

        let scanned = ServerId::parse(&offer.server_id).ok_or_else(|| {
            ClientError::BadOffer(format!("{} is not a server id", offer.server_id))
        })?;

        let endpoint_id = offer
            .endpoint_id
            .clone()
            .ok_or(ClientError::OfferHasNoEndpoint)?;

        let connection = match endpoint
            .dial(&endpoint_id, offer.relay.as_deref(), &offer.direct_addrs)
            .await
        {
            Ok(connection) => connection,
            Err(error) => {
                log::debug!("pairing dial failed: {error}");

                return Ok(Self::done(BeginOutcome::Unreachable));
            }
        };

        let Ok((mut send, recv)) = connection.open_bi().await else {
            return Ok(Self::done(Self::closed_as(&connection).await));
        };

        let codec = FrameCodec::default();
        let hello = StreamOpen::Hello(ClientHello {
            versions: WireVersion::SUPPORTED.to_vec(),
            client,
            intent: Intent::Enroll,
        });

        if FrameIo::write(&mut send, &codec, &hello).await.is_err() {
            return Ok(Self::done(Self::closed_as(&connection).await));
        }

        Self::judge(Dialed {
            connection,
            send,
            recv,
            codec,
            offer,
            scanned,
            endpoint_id,
            device_name,
        })
        .await
    }

    pub fn found(&self) -> &FoundServer {
        &self.found
    }

    /// Writes a typed code on the held stream and reads the machine's verdict.
    ///
    /// `WrongCode` leaves this session usable. Everything else finishes it, and
    /// the caller drops it.
    pub async fn submit(&mut self, code: &str) -> Result<PairOutcome, ClientError> {
        let typed = EnrollCode {
            request_id: self.request_id.clone(),
            // Normalised here as well as on the machine. The code is read off a
            // screen and typed on a phone, which adds spaces and changes case.
            code: Handshake::normalize_code(code),
            device_name: self.device_name.clone(),
        };

        if FrameIo::write(&mut self.send, &self.codec, &typed)
            .await
            .is_err()
        {
            return Ok(PairOutcome::LinkLost);
        }

        let result: Option<EnrollResult> =
            match FrameIo::read(&mut self.recv, &self.codec).await {
                Ok(result) => result,
                Err(_) => return Ok(PairOutcome::LinkLost),
            };

        let Some(result) = result else {
            return Ok(PairOutcome::LinkLost);
        };

        Ok(match result {
            EnrollResult::Accepted {
                device,
                server,
                capabilities,
                ..
            } => PairOutcome::Paired(ServerEntry {
                server,
                endpoint_id: self.found.endpoint_id.clone(),
                relay: self.offer.relay.clone(),
                direct_addrs: self.offer.direct_addrs.clone(),
                device,
                capabilities,
                last_seen_at: None,
                // Nothing is known about this machine's work yet. The first
                // sweep after pairing fills it.
                conversations: Vec::new(),
            }),
            // Zero covers two causes the machine cannot separate for us: the
            // attempts are spent, or no window was open when the code arrived.
            // Both are fixed by opening a new window, so both report the same
            // thing and neither claims a number of wrong guesses.
            EnrollResult::Refused {
                attempts_left: 0, ..
            } => PairOutcome::WindowSpent,
            EnrollResult::Refused { attempts_left, .. } => PairOutcome::WrongCode { attempts_left },
        })
    }

    async fn judge(dialed: Dialed) -> Result<Begun, ClientError> {
        let Dialed {
            connection,
            send,
            mut recv,
            codec,
            offer,
            scanned,
            endpoint_id,
            device_name,
        } = dialed;

        let answer: Option<ServerHello> = match FrameIo::read(&mut recv, &codec).await {
            Ok(answer) => answer,
            Err(_) => return Ok(Self::done(Self::closed_as(&connection).await)),
        };

        let Some(answer) = answer else {
            return Ok(Self::done(Self::closed_as(&connection).await));
        };

        match answer {
            ServerHello::EnrollPending {
                request_id,
                expires_in_ms,
                server,
                code_length,
                code_format,
            } => {
                // The scanned id is a claim by whoever printed the offer; this
                // one is proved by the peer's TLS certificate. Disagreement
                // means the offer and the answering machine are not the same
                // machine.
                if server.id != scanned {
                    return Ok(Self::done(BeginOutcome::IdMismatch {
                        scanned: scanned.as_str().to_string(),
                        answered: server.id,
                    }));
                }

                let found = FoundServer {
                    server,
                    endpoint_id,
                    relay: offer.relay.clone(),
                    direct_addr_count: offer.direct_addrs.len() as u16,
                    code_length,
                    code_format,
                    expires_in_ms,
                };

                Ok(Begun {
                    outcome: BeginOutcome::Found(found.clone()),
                    session: Some(Self {
                        send,
                        recv,
                        codec,
                        found,
                        request_id,
                        device_name,
                        offer,
                        _connection: connection,
                    }),
                })
            }
            ServerHello::Session {
                server,
                capabilities,
                device,
                ..
            } => Ok(Self::done(BeginOutcome::AlreadyPaired(ServerEntry {
                server,
                endpoint_id,
                relay: offer.relay,
                direct_addrs: offer.direct_addrs,
                device,
                capabilities,
                last_seen_at: None,
                // Nothing is known about this machine's work yet. The first
                // sweep after pairing fills it.
                conversations: Vec::new(),
            }))),
            ServerHello::Refuse(reason) => Ok(Self::done(match reason {
                RefuseReason::PairingWindowClosed => BeginOutcome::WindowClosed,
                RefuseReason::Revoked => BeginOutcome::Revoked,
                RefuseReason::NoCommonVersion => BeginOutcome::NoCommonVersion,
                // Cannot occur under Intent::Enroll: the dispatcher answers an
                // unknown endpoint id with a window, or with PairingWindowClosed
                // when none is open.
                RefuseReason::NotEnrolled => BeginOutcome::WindowClosed,
            })),
        }
    }

    /// Why the machine hung up, when it did so without writing a frame.
    ///
    /// An unknown code is never folded into a known one. `CloseCode::from_u32`
    /// returns `None` rather than a default for exactly this reason: reporting
    /// one refusal as another sends a person somewhere useless.
    async fn closed_as(connection: &Connection) -> BeginOutcome {
        let closed = tokio::time::timeout(Self::CLOSE_REASON_TIMEOUT, connection.closed()).await;

        let Ok(ConnectionError::ApplicationClosed(close)) = closed else {
            return BeginOutcome::Unreachable;
        };

        let code = u64::from(close.error_code) as u32;

        match CloseCode::from_u32(code) {
            Some(CloseCode::AtCapacity) => BeginOutcome::AtCapacity,
            None => BeginOutcome::ClosedByMachine { code },
        }
    }

    fn done(outcome: BeginOutcome) -> Begun {
        Begun {
            outcome,
            session: None,
        }
    }
}
