use crate::error::TransportError;
use crate::frame::FrameCodec;
use iroh::endpoint::{RecvStream, SendStream};

/// Length-prefixed frames over one QUIC stream.
///
/// The protocol's stream discipline is: the first frame declares what the stream
/// is, and after that the stream is typed. These two calls are the whole of the
/// machinery that needs. QUIC is the multiplexer, so there is no correlation
/// table and no request id.
pub struct FrameIo;

impl FrameIo {
    pub async fn write<T: serde::Serialize>(
        send: &mut SendStream,
        codec: &FrameCodec,
        frame: &T,
    ) -> Result<(), TransportError> {
        let bytes = codec.encode(frame)?;

        send.write_all(&bytes)
            .await
            .map_err(|error| TransportError::Connection(error.to_string()))
    }

    /// `Ok(None)` when the peer finished the stream before a header.
    ///
    /// That is a peer that is done, not a failure; treating it as one makes
    /// every orderly close log an error. A header that arrives *partially* is a
    /// different thing entirely - the peer began a frame and vanished - and is
    /// reported, because swallowing it would hide real corruption behind the
    /// same silence as a clean goodbye.
    pub async fn read<T: serde::de::DeserializeOwned>(
        recv: &mut RecvStream,
        codec: &FrameCodec,
    ) -> Result<Option<T>, TransportError> {
        let mut header = [0u8; FrameCodec::HEADER_BYTES];

        match recv.read_exact(&mut header).await {
            Ok(()) => {}
            Err(iroh::endpoint::ReadExactError::FinishedEarly(0)) => return Ok(None),
            Err(error) => return Err(TransportError::Connection(error.to_string())),
        }

        let len = codec.decode_length(header)?;
        let mut body = vec![0u8; len];

        recv.read_exact(&mut body)
            .await
            .map_err(|error| TransportError::Connection(error.to_string()))?;

        codec.decode_body(&body).map(Some)
    }
}

/// Two connected endpoints on the loopback interface.
///
/// Lives here rather than in each test file because the protocol suite and the
/// transport suite both need it, and a second copy would be a second definition
/// of what "connected" means.
#[cfg(any(test, feature = "testing"))]
pub mod testing {
    use crate::endpoint::TetheraEndpoint;
    use crate::error::TransportError;
    use iroh::endpoint::Connection;

    pub struct Loopback {
        pub client: Connection,
        pub server: Connection,
        /// Held so the endpoints outlive the connections. Dropping an endpoint
        /// closes everything it opened, which turns a passing test into a
        /// confusing transport error.
        pub client_endpoint: TetheraEndpoint,
        pub server_endpoint: TetheraEndpoint,
    }

    impl Loopback {
        pub async fn connect() -> Result<Self, TransportError> {
            let server_endpoint = TetheraEndpoint::bind_local().await?;
            let client_endpoint = TetheraEndpoint::bind_local().await?;
            let server_addr = server_endpoint.loopback_addr()?;

            // Accept is spawned first: the dial completes only once the far side
            // has accepted, so calling them in sequence would deadlock.
            let accepting = tokio::spawn(async move {
                let connection = server_endpoint.accept().await;
                (server_endpoint, connection)
            });

            let client = client_endpoint.connect(server_addr).await?;
            let (server_endpoint, server) = accepting
                .await
                .map_err(|error| TransportError::Connection(error.to_string()))?;

            Ok(Self {
                client,
                server: server?,
                client_endpoint,
                server_endpoint,
            })
        }
    }
}
