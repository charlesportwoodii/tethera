mod budget;

pub use budget::SweepBudget;

use crate::endpoint::ClientEndpoint;
use crate::link::Measure;
use crate::rpc::Rpc;
use crate::session::Session;
use iroh::endpoint::Connection;
use std::sync::Arc;
use std::time::Duration;
use tethera_common::protocol::handshake::{ClientInfo, ServerHello};
use tethera_common::protocol::response::Payload;
use tethera_common::protocol::Request;
use tethera_common::structs::client::{ServerEntry, ServerRow};
use tethera_common::structs::conversation::{Conversation, ConversationFilter};
use tethera_common::structs::link::{Link, LinkKind};
use tethera_common::structs::primitives::Timestamp;
use tokio::sync::Semaphore;

/// One reachability pass over every remembered machine.
///
/// Dial, handshake, measure, close. Nothing is held open: a phone keeping a QUIC
/// connection per paired machine burns radio for state the operating system
/// suspends anyway, and a suspended connection still wearing a live dot is worse
/// than no dot at all.
///
/// The handshake alone answers the list. `ServerHello::Session` carries the
/// server info and the capabilities, and the device record carries `paired_at`,
/// so a row costs one connection and one round trip with no follow-up request.
pub struct Sweep;

impl Sweep {
    /// Each dial is a QUIC handshake and possibly a hole punch. A phone radio
    /// does not reward more, and a person owns few machines.
    pub const MAX_CONCURRENT: usize = 4;

    /// How many conversations a list row carries.
    ///
    /// The list is a glance across every machine, not a full index. The rest are
    /// a page away, where they can be paged and sectioned properly.
    ///
    /// Twelve rather than five because this is a `Live` listing. Five was a cap
    /// on the newest sessions on disk, where the sixth was almost always
    /// finished work; it is now a cap on what is actually running, where the
    /// twelfth may be the one somebody is waiting on.
    pub const ROW_CONVERSATIONS: u16 = 12;

    pub async fn run(
        endpoint: &ClientEndpoint,
        entries: Vec<ServerEntry>,
        client: ClientInfo,
        budget: SweepBudget,
    ) -> Vec<ServerRow> {
        let permits = Arc::new(Semaphore::new(Self::MAX_CONCURRENT));

        let probes = entries.into_iter().map(|entry| {
            let permits = permits.clone();
            let client = client.clone();

            async move {
                let _permit = permits.acquire().await;

                Self::probe(endpoint, entry, client, budget).await
            }
        });

        // Ordered, so the list does not reshuffle as machines answer.
        let all = futures_util::future::join_all(probes);

        match tokio::time::timeout(budget.total, all).await {
            Ok(rows) => rows,
            // Nothing partial survives a whole-sweep timeout. That is a bug
            // worth seeing rather than a state worth rendering.
            Err(_) => Vec::new(),
        }
    }

    async fn probe(
        endpoint: &ClientEndpoint,
        entry: ServerEntry,
        client: ClientInfo,
        budget: SweepBudget,
    ) -> ServerRow {
        let asking = Self::ask(endpoint, &entry, client, budget.settle);

        match tokio::time::timeout(budget.dial, asking).await {
            Ok(Some(row)) => row,
            _ => Self::offline(entry),
        }
    }

    async fn ask(
        endpoint: &ClientEndpoint,
        entry: &ServerEntry,
        client: ClientInfo,
        settle: Duration,
    ) -> Option<ServerRow> {
        let connection = endpoint
            .dial(
                &entry.endpoint_id,
                entry.relay.as_deref(),
                &entry.direct_addrs,
            )
            .await
            .ok()?;

        let answer = Session::open(&connection, client).await.ok()?;
        let link = Measure::settled(&connection, settle).await;

        let row = match answer {
            ServerHello::Session {
                server,
                capabilities,
                device,
                ..
            } => ServerRow {
                entry: ServerEntry {
                    server,
                    capabilities,
                    device,
                    last_seen_at: Some(Timestamp(Self::now())),
                    conversations: Self::conversations(&connection, &entry.conversations).await,
                    ..entry.clone()
                },
                link,
                refusal: None,
            },
            // The machine is up and said no. That is a different row from
            // silence, and the person should not be sent to debug a network that
            // is working.
            ServerHello::Refuse(reason) => ServerRow {
                entry: entry.clone(),
                link,
                refusal: Some(reason),
            },
            // A machine offering enrolment to a device that asked for a session
            // is confused. Nothing here can be reported honestly, so it is left
            // to the offline path.
            ServerHello::EnrollPending { .. } => return None,
        };

        connection.close(0u32.into(), b"swept");

        Some(row)
    }

    /// What is running on a machine that has just answered.
    ///
    /// A second round trip on a connection already open, which is cheap next to
    /// the dial that preceded it. A machine with no transcript reader answers
    /// with an empty page and the row simply draws no conversations.
    ///
    /// A failure keeps what was last seen rather than blanking the row: losing
    /// this call is not evidence that the work stopped.
    async fn conversations(
        connection: &Connection,
        held: &[Conversation],
    ) -> Vec<Conversation> {
        let request = Request::ListConversations {
            // What is running, rather than what is newest. An unbound
            // conversation is reported Done whatever its records say, so an
            // unfiltered listing spends the row's whole budget on finished work
            // and hides the agent that is blocked behind it.
            filter: ConversationFilter::Live,
            before: None,
            limit: Self::ROW_CONVERSATIONS,
        };

        match Rpc::request(connection, request).await {
            Ok(Payload::Conversations(page)) => page.items,
            Ok(_) => held.to_vec(),
            Err(error) => {
                log::debug!("could not list conversations: {error}");

                held.to_vec()
            }
        }
    }

    fn offline(entry: ServerEntry) -> ServerRow {
        ServerRow {
            entry,
            link: Link {
                kind: LinkKind::Offline,
                rtt_ms: None,
            },
            refusal: None,
        }
    }

    // Milliseconds. `Timestamp` is epoch millis everywhere on this wire, and
    // seconds written into it read as 1970 on one screen and as the year 58621
    // on another.
    fn now() -> i64 {
        chrono::Utc::now().timestamp_millis()
    }
}
