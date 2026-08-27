//! A client, driven by hand.
//!
//! Deliberately not a reusable client library: every frame this writes is
//! written explicitly so a test reads as the protocol, not as a wrapper around
//! it. If a step here is awkward, that is a fact about the protocol worth
//! seeing.

use std::sync::Arc;

use iroh::endpoint::{Connection, RecvStream, SendStream};
use tethera_common::protocol::handshake::{
    ClientHello, ClientInfo, EnrollCode, EnrollResult, Intent, Platform, ServerHello,
};
use tethera_common::protocol::request::Request;
use tethera_common::protocol::response::{Payload, Response};
use tethera_common::protocol::stream::StreamOpen;
use tethera_common::protocol::terminal::AttachSpec;
use tethera_common::protocol::transfer::{FetchHead, FetchSpec, PutReady, PutResult, PutSpec};
use tethera_common::protocol::watch::{WatchOpen, WatchSpec};
use tethera_common::protocol::WireVersion;
use tethera_common::structs::ids::RequestId;
use tethera_transport::frame::FrameCodec;
use tethera_transport::stream::testing::Loopback;
use tethera_transport::stream::FrameIo;
use tethera_server_lib::protocol::Dispatcher;

use super::fakes::FakePorts;

pub struct Harness {
    pub ports: Arc<FakePorts>,
    pub connection: Connection,
    codec: FrameCodec,
    /// Held for the whole test. Dropping an endpoint closes everything it
    /// opened, and the serve task finishes long before a test does - so parking
    /// these in the task would kill the client's connection mid-assertion.
    _endpoints: (
        tethera_transport::endpoint::TetheraEndpoint,
        tethera_transport::endpoint::TetheraEndpoint,
    ),
    _server: tokio::task::JoinHandle<()>,
}

impl Harness {
    /// A dispatcher serving one loopback connection, with fresh fake ports.
    pub async fn start() -> Self {
        let ports = Arc::new(FakePorts::new());

        Self::start_with(ports).await
    }

    /// A dispatcher serving one loopback connection over ports the caller keeps
    /// a handle to, so a test can observe what a handler did.
    pub async fn start_with(ports: Arc<FakePorts>) -> Self {
        let pair = Loopback::connect().await.expect("loopback");
        let dispatcher = Dispatcher::new(ports.clone());

        let Loopback {
            client,
            server,
            client_endpoint,
            server_endpoint,
        } = pair;

        let handle = tokio::spawn(async move {
            let _ = dispatcher.serve_connection(server).await;
        });

        Self {
            ports,
            connection: client,
            codec: FrameCodec::default(),
            _endpoints: (client_endpoint, server_endpoint),
            _server: handle,
        }
    }

    /// Opens the mandatory first stream and reads the answer.
    ///
    /// The stream is returned because enrolment continues on it.
    pub async fn hello(&self, intent: Intent) -> (ServerHello, SendStream, RecvStream) {
        let (mut send, mut recv) = self.connection.open_bi().await.expect("open");

        let hello = StreamOpen::Hello(ClientHello {
            versions: WireVersion::SUPPORTED.to_vec(),
            client: ClientInfo {
                app_version: "0.1.0".into(),
                platform: Platform::Ios,
                install_id: "3f9a2c".into(),
            },
            intent,
        });

        FrameIo::write(&mut send, &self.codec, &hello)
            .await
            .expect("write hello");

        let answer: ServerHello = FrameIo::read(&mut recv, &self.codec)
            .await
            .expect("read")
            .expect("a server hello");

        (answer, send, recv)
    }

    /// Offers a hello whose version list shares nothing with the server's.
    pub async fn hello_with_versions(&self, versions: Vec<WireVersion>) -> ServerHello {
        let (mut send, mut recv) = self.connection.open_bi().await.expect("open");

        let hello = StreamOpen::Hello(ClientHello {
            versions,
            client: ClientInfo {
                app_version: "0.1.0".into(),
                platform: Platform::Ios,
                install_id: "3f9a2c".into(),
            },
            intent: Intent::Session,
        });

        FrameIo::write(&mut send, &self.codec, &hello)
            .await
            .expect("write hello");

        FrameIo::read(&mut recv, &self.codec)
            .await
            .expect("read")
            .expect("a server hello")
    }

    pub async fn type_code(
        &self,
        send: &mut SendStream,
        recv: &mut RecvStream,
        code: &str,
    ) -> EnrollResult {
        let typed = EnrollCode {
            request_id: RequestId("req".into()),
            code: code.to_string(),
            device_name: "phone".to_string(),
        };

        FrameIo::write(send, &self.codec, &typed)
            .await
            .expect("write code");

        FrameIo::read(recv, &self.codec)
            .await
            .expect("read")
            .expect("an enrol result")
    }

    /// Every frame the server wrote for one request, in order.
    ///
    /// Returned whole rather than just the terminal frame, so a test can assert
    /// on the progress that preceded it.
    pub async fn rpc_frames(&self, request: Request) -> Vec<Response> {
        let (mut send, mut recv) = self.connection.open_bi().await.expect("open");

        FrameIo::write(&mut send, &self.codec, &StreamOpen::Rpc(request))
            .await
            .expect("write request");
        send.finish().ok();

        let mut frames = Vec::new();

        while let Some(frame) = FrameIo::read::<Response>(&mut recv, &self.codec)
            .await
            .expect("read")
        {
            let terminal = frame.is_terminal();
            frames.push(frame);

            if terminal {
                break;
            }
        }

        frames
    }

    /// The one terminal frame of a request, with any progress discarded.
    pub async fn rpc(&self, request: Request) -> Response {
        self.rpc_frames(request)
            .await
            .pop()
            .expect("a terminal frame")
    }

    pub async fn payload(&self, request: Request) -> Payload {
        match self.rpc(request).await {
            Response::Ok(payload) => payload,
            other => panic!("expected a payload, got {other:?}"),
        }
    }

    pub async fn watch(&self, spec: WatchSpec) -> (WatchOpen, RecvStream) {
        let (mut send, mut recv) = self.connection.open_bi().await.expect("open");

        FrameIo::write(&mut send, &self.codec, &StreamOpen::Watch(spec))
            .await
            .expect("write watch");

        let open: WatchOpen = FrameIo::read(&mut recv, &self.codec)
            .await
            .expect("read")
            .expect("a watch open");

        (open, recv)
    }

    pub async fn attach(&self, spec: AttachSpec) -> (SendStream, RecvStream) {
        let (mut send, recv) = self.connection.open_bi().await.expect("open");

        FrameIo::write(&mut send, &self.codec, &StreamOpen::Attach(spec))
            .await
            .expect("write attach");

        (send, recv)
    }

    /// The head frame plus every byte after it, to FIN.
    pub async fn fetch(&self, spec: FetchSpec) -> (FetchHead, Vec<u8>) {
        let (mut send, mut recv) = self.connection.open_bi().await.expect("open");

        FrameIo::write(&mut send, &self.codec, &StreamOpen::Fetch(spec))
            .await
            .expect("write fetch");
        send.finish().ok();

        let head: FetchHead = FrameIo::read(&mut recv, &self.codec)
            .await
            .expect("read")
            .expect("a fetch head");

        let body = recv.read_to_end(1024 * 1024).await.expect("read body");

        (head, body)
    }

    /// Writes the bytes the server asks for, starting at the offset it names.
    pub async fn put(&self, spec: PutSpec, whole: &[u8]) -> (PutReady, PutResult) {
        let (mut send, mut recv) = self.connection.open_bi().await.expect("open");

        FrameIo::write(&mut send, &self.codec, &StreamOpen::Put(spec))
            .await
            .expect("write put");

        let ready: PutReady = FrameIo::read(&mut recv, &self.codec)
            .await
            .expect("read")
            .expect("a put ready");

        send.write_all(&whole[ready.offset as usize..])
            .await
            .expect("write body");
        send.finish().ok();

        let result: PutResult = FrameIo::read(&mut recv, &self.codec)
            .await
            .expect("read")
            .expect("a put result");

        (ready, result)
    }

    pub fn codec(&self) -> &FrameCodec {
        &self.codec
    }
}
