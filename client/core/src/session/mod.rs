use crate::error::ClientError;
use iroh::endpoint::Connection;
use tethera_common::protocol::handshake::{ClientHello, ClientInfo, Intent, ServerHello};
use tethera_common::protocol::stream::StreamOpen;
use tethera_common::protocol::WireVersion;
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::FrameIo;

/// The mandatory first stream on a connection.
///
/// Every connection begins this way, so it lives here rather than being written
/// out at each dial. A second copy is a second place for the intent or the
/// version list to drift.
pub struct Session;

impl Session {
    /// Says hello with `Intent::Session` and returns the machine's answer.
    ///
    /// Session rather than Enroll: this is for a machine already paired, and a
    /// machine that does not recognise the endpoint id refuses rather than
    /// offering a pairing window. Enrolment has its own path in `pairing`,
    /// because it holds the stream open afterwards and this does not.
    pub async fn open(
        connection: &Connection,
        client: ClientInfo,
    ) -> Result<ServerHello, ClientError> {
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))?;

        let codec = FrameCodec::default();
        let hello = StreamOpen::Hello(ClientHello {
            versions: WireVersion::SUPPORTED.to_vec(),
            client,
            intent: Intent::Session,
        });

        FrameIo::write(&mut send, &codec, &hello)
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))?;

        FrameIo::read(&mut recv, &codec)
            .await
            .map_err(|error| ClientError::Rpc(error.to_string()))?
            .ok_or_else(|| {
                ClientError::Rpc("the machine closed the stream without a hello".to_string())
            })
    }
}
