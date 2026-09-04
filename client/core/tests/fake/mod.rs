//! A machine, scripted by hand.
//!
//! Deliberately not a reusable server: every frame it writes is written
//! explicitly so a test reads as the protocol rather than as a wrapper around
//! it. It is the mirror image of the server suite's hand-driven client.

use tethera_common::protocol::capability::CapabilitySet;
use tethera_common::protocol::handshake::{
    CodeFormat, DeviceRecord, EnrollCode, EnrollResult, Handshake, RefuseReason, ServerHello,
    ServerInfo,
};
use tethera_common::protocol::response::{Page, Payload, Response};
use tethera_common::protocol::stream::StreamOpen;
use tethera_common::protocol::{Request, WireVersion};
use tethera_common::structs::conversation::{Conversation, ConversationFilter};
use tethera_common::structs::ids::{DeviceId, RequestId, ServerId};
use tethera_common::structs::pairing::PairingOffer;
use tethera_common::structs::primitives::Timestamp;
use tethera_transport::endpoint::TetheraEndpoint;
use tethera_transport::error::TransportError;
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::FrameIo;

use std::sync::{Arc, Mutex};

/// What this machine does when a client says hello.
#[derive(Debug, Clone)]
pub enum Answer {
    /// Offers enrolment, then compares typed codes against `code` until
    /// `attempts` is spent.
    Enroll {
        code: String,
        attempts: u8,
        expires_in_ms: u32,
    },
    /// Already knows this device.
    Session,
    /// Already knows this device, but its conversation port is broken: it
    /// completes the handshake and then refuses every request.
    SessionRefusingRequests,
    Refuse(RefuseReason),
    /// Closes at the transport level without writing a frame, the way a machine
    /// already serving its connection limit does.
    Close { code: u32, reason: &'static [u8] },
}

pub struct FakeMachine {
    endpoint_id: String,
    addr: iroh::EndpointAddr,
    /// What this machine claims in its `ServerHello`.
    info: ServerInfo,
    /// What its printed offer says, which for an honest machine is the same id.
    /// Kept apart so a test can print one machine's QR and have another answer.
    offer_id: ServerId,
    /// Which filters this machine has been asked for, in order.
    ///
    /// The sweep's choice of filter decides whether a long-running blocked agent
    /// can reach a list row at all, and nothing else on the client observes it.
    asked: Arc<Mutex<Vec<ConversationFilter>>>,
    _serve: tokio::task::JoinHandle<()>,
}

impl FakeMachine {
    pub const LABEL: &'static str = "atlas";

    pub async fn start(answer: Answer) -> Self {
        Self::start_with(answer, None, Vec::new()).await
    }

    /// A machine that reports work in progress when asked.
    pub async fn start_running(answer: Answer, conversations: Vec<Conversation>) -> Self {
        Self::start_with(answer, None, conversations).await
    }

    pub async fn start_as(answer: Answer, server_id: Option<&str>) -> Self {
        Self::start_with(answer, server_id, Vec::new()).await
    }

    /// `server_id` overrides the id this machine claims in its `ServerHello`
    /// while leaving its printed offer honest, so a test can scan one machine's
    /// QR and have a different machine answer.
    pub async fn start_with(
        answer: Answer,
        server_id: Option<&str>,
        conversations: Vec<Conversation>,
    ) -> Self {
        let endpoint = TetheraEndpoint::bind_local().await.expect("bind");
        let endpoint_id = endpoint.id().to_string();
        let addr = endpoint.loopback_addr().expect("loopback addr");

        let offer_id = ServerId::parse(&format!("sv_{endpoint_id}")).expect("a valid server id");
        let claimed = match server_id {
            Some(id) => id.to_string(),
            None => offer_id.as_str().to_string(),
        };

        let info = ServerInfo {
            id: ServerId::parse(&claimed).expect("a valid server id"),
            label: Self::LABEL.to_string(),
            app_version: "0.1.0".to_string(),
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
        };

        let serving = info.clone();
        let running = conversations.clone();
        let asked: Arc<Mutex<Vec<ConversationFilter>>> = Arc::new(Mutex::new(Vec::new()));
        let recording = Arc::clone(&asked);
        let handle = tokio::spawn(async move {
            let conversations = running;
            while let Ok(connection) = endpoint.accept().await {
                let answer = answer.clone();
                let info = serving.clone();

                if let Answer::Close { code, reason } = answer {
                    connection.close(code.into(), reason);
                    continue;
                }

                // Built per connection rather than cloned: FrameCodec carries no
                // derives, and adding one to the transport crate to suit a test
                // fixture would be the fixture dictating to the library.
                let _ = Self::serve(
                    connection,
                    answer,
                    info,
                    conversations.clone(),
                    Arc::clone(&recording),
                    FrameCodec::default(),
                )
                .await;
            }
        });

        Self {
            endpoint_id,
            addr,
            info,
            offer_id,
            asked,
            _serve: handle,
        }
    }

    /// Which filters this machine was asked for, in order.
    pub fn filters_asked(&self) -> Vec<ConversationFilter> {
        self.asked.lock().expect("the filter record").clone()
    }

    pub fn endpoint_id(&self) -> String {
        self.endpoint_id.clone()
    }

    pub fn server_info(&self) -> ServerInfo {
        self.info.clone()
    }

    pub fn direct_addrs(&self) -> Vec<String> {
        self.addr.ip_addrs().map(|addr| addr.to_string()).collect()
    }

    /// The offer a client would have scanned off this machine's screen.
    pub fn offer_uri(&self) -> String {
        PairingOffer::new(
            self.offer_id.as_str().to_string(),
            Some(self.endpoint_id()),
            None,
            self.direct_addrs(),
            Some(Self::LABEL.to_string()),
        )
        .to_uri()
    }

    fn device() -> DeviceRecord {
        DeviceRecord {
            id: DeviceId::parse("dv_phone").expect("a valid device id"),
            name: "phone".to_string(),
            paired_at: Timestamp(1),
        }
    }

    async fn serve(
        connection: iroh::endpoint::Connection,
        answer: Answer,
        info: ServerInfo,
        conversations: Vec<Conversation>,
        asked: Arc<Mutex<Vec<ConversationFilter>>>,
        codec: FrameCodec,
    ) -> Result<(), TransportError> {
        let (mut send, mut recv) = connection
            .accept_bi()
            .await
            .map_err(|error| TransportError::Connection(error.to_string()))?;

        let open: Option<StreamOpen> = FrameIo::read(&mut recv, &codec).await?;

        let Some(StreamOpen::Hello(_)) = open else {
            return Ok(());
        };

        match answer {
            Answer::Session | Answer::SessionRefusingRequests => {
                let refusing = matches!(answer, Answer::SessionRefusingRequests);

                FrameIo::write(
                    &mut send,
                    &codec,
                    &ServerHello::Session {
                        version: WireVersion(1),
                        server: info,
                        capabilities: CapabilitySet::new(),
                        device: Self::device(),
                    },
                )
                .await?;
                send.finish().ok();

                // The handshake is one stream of many. A real machine keeps
                // serving the connection afterwards, and a fixture that stopped
                // here would make every later request look like a dead machine.
                Self::serve_requests(&connection, &conversations, &asked, refusing, &codec).await?;
            }
            Answer::Refuse(reason) => {
                FrameIo::write(&mut send, &codec, &ServerHello::Refuse(reason)).await?;
                send.finish().ok();
                connection.closed().await;
            }
            Answer::Enroll {
                code,
                mut attempts,
                expires_in_ms,
            } => {
                FrameIo::write(
                    &mut send,
                    &codec,
                    &ServerHello::EnrollPending {
                        request_id: RequestId("req".to_string()),
                        expires_in_ms,
                        server: info.clone(),
                        code_length: code.len() as u8,
                        code_format: CodeFormat::Digits,
                    },
                )
                .await?;

                Self::redeem(&mut send, &mut recv, &codec, &info, &code, &mut attempts).await?;

                // Wait for the peer rather than returning, which would drop the
                // connection and close it before the client has read the frame
                // just written. The real dispatcher keeps serving the connection
                // after enrolment for the same reason.
                connection.closed().await;
            }
            Answer::Close { .. } => {}
        }

        Ok(())
    }

    /// Answers requests until the peer hangs up.
    async fn serve_requests(
        connection: &iroh::endpoint::Connection,
        conversations: &[Conversation],
        asked: &Mutex<Vec<ConversationFilter>>,
        refusing: bool,
        codec: &FrameCodec,
    ) -> Result<(), TransportError> {
        loop {
            let Ok((mut send, mut recv)) = connection.accept_bi().await else {
                return Ok(());
            };

            let open: Option<StreamOpen> = FrameIo::read(&mut recv, codec).await?;

            let Some(StreamOpen::Rpc(request)) = open else {
                continue;
            };

            let response = match request {
                _ if refusing => Response::Err(tethera_common::protocol::WireError::Backend {
                    message: "this machine cannot read agent transcripts".to_string(),
                }),
                Request::ListConversations { filter, limit, .. } => {
                    asked.lock().expect("the filter record").push(filter);

                    let items: Vec<Conversation> =
                        conversations.iter().take(limit as usize).cloned().collect();
                    let has_earlier = items.len() < conversations.len();

                    Response::Ok(Payload::Conversations(Page {
                        items,
                        next_before: None,
                        has_earlier,
                    }))
                }
                _ => Response::Err(tethera_common::protocol::WireError::Backend {
                    message: "this fixture answers only ListConversations".to_string(),
                }),
            };

            FrameIo::write(&mut send, codec, &response).await?;
            send.finish().ok();
        }
    }

    // The retry loop the real dispatcher runs: a wrong code spends one attempt
    // and the stream stays open, because somebody mistyping six digits is the
    // common case rather than the exceptional one.
    async fn redeem(
        send: &mut iroh::endpoint::SendStream,
        recv: &mut iroh::endpoint::RecvStream,
        codec: &FrameCodec,
        info: &ServerInfo,
        code: &str,
        attempts: &mut u8,
    ) -> Result<(), TransportError> {
        loop {
            let typed: Option<EnrollCode> = FrameIo::read(recv, codec).await?;

            let Some(typed) = typed else {
                return Ok(());
            };

            if Handshake::normalize_code(&typed.code) == Handshake::normalize_code(code) {
                FrameIo::write(
                    send,
                    codec,
                    &EnrollResult::Accepted {
                        device: Self::device(),
                        version: WireVersion(1),
                        server: info.clone(),
                        capabilities: CapabilitySet::new(),
                    },
                )
                .await?;
                send.finish().ok();

                return Ok(());
            }

            *attempts = attempts.saturating_sub(1);

            FrameIo::write(
                send,
                codec,
                &EnrollResult::Refused {
                    reason: RefuseReason::NotEnrolled,
                    attempts_left: *attempts,
                },
            )
            .await?;

            // Out of attempts: the window is spent, and a stream left open would
            // invite an unbounded guessing loop against a six-digit code.
            if *attempts == 0 {
                send.finish().ok();

                return Ok(());
            }
        }
    }
}
