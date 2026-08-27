use crate::state::AppState;
use tauri::{AppHandle, State};
use tethera_client_core::rpc::Rpc;
use tethera_common::protocol::capability::{self, HasCapability};
use tethera_common::protocol::response::{Page, Payload};
use tethera_common::protocol::Request;
use tethera_client_core::error::ClientError;
use tethera_common::protocol::WireError;
use tethera_common::structs::client::{ConversationControls, SendOutcome};
use tethera_common::structs::conversation::Conversation;
use tethera_common::structs::ids::{AssetId, ConversationId, QuestionId, ServerId};
use tethera_common::structs::primitives::{Cursor, Fingerprint};
use tethera_common::structs::transcript::{Answer, Turn};



pub struct Conversations;

impl Conversations {
    /// Parses at the boundary rather than passing the string inwards.
    ///
    /// The prefix is part of the value, so a pane id handed in where a
    /// conversation id belongs fails here by name instead of reaching the
    /// machine and resolving to nothing.
    fn id(value: &str) -> Result<ConversationId, String> {
        ConversationId::parse(value).ok_or_else(|| format!("{value} is not a conversation id"))
    }

}

/// One page of a conversation's history, newest page first.
///
/// `before` is the cursor of the oldest turn already held. Absent asks for the
/// most recent page, which is what opening a conversation wants.
#[tauri::command]
pub(crate) async fn conversation_transcript(
    state: State<'_, AppState>,
    server: String,
    conversation: String,
    before: Option<String>,
    limit: u16,
) -> Result<Page<Turn>, String> {
    let server = ServerId::parse(&server).ok_or_else(|| format!("{server} is not a server id"))?;
    let connection = state.connect(&server).await?;

    let answer = Rpc::request(
        &connection,
        Request::Transcript {
            conversation: Conversations::id(&conversation)?,
            before: before.map(Cursor),
            limit,
        },
    )
    .await;

    match answer {
        Ok(Payload::Transcript(page)) => Ok(page),
        Ok(other) => Err(format!("the machine answered with {other:?}")),
        // A conversation that has begun no turn has no records on disk, so its
        // transcript is genuinely absent rather than lost. An agent announces
        // its session before it writes anything, which is exactly the window
        // somebody sees after pressing start.
        Err(ClientError::Wire(WireError::NotFound { .. })) => Ok(Page {
            items: Vec::new(),
            next_before: None,
            has_earlier: false,
        }),
        Err(error) => Err(error.to_string()),
    }
}

/// One conversation as the machine has it.
///
/// Used when the live tail cannot be opened. A conversation that has begun no
/// turn has no records to watch, so the machine ends the watch — but it can
/// still describe the conversation, which is the whole header.
#[tauri::command]
pub(crate) async fn get_conversation(
    state: State<'_, AppState>,
    server: String,
    conversation: String,
) -> Result<Conversation, String> {
    let server = ServerId::parse(&server).ok_or_else(|| format!("{server} is not a server id"))?;
    let connection = state.connect(&server).await?;

    let payload = Rpc::request(
        &connection,
        Request::GetConversation {
            conversation: Conversations::id(&conversation)?,
        },
    )
    .await
    .map_err(|error| error.to_string())?;

    match payload {
        Payload::Conversation(conversation) => Ok(conversation),
        other => Err(format!("the machine answered with {other:?}")),
    }
}

/// Opens the live tail and answers with the conversation as the machine has it.
///
/// `after` is the cursor of the newest turn already held, so a reconnect resumes
/// rather than replaying. The machine answers with where the stream *actually*
/// starts, which is not always what was asked for: later means the held cursor
/// predates the earliest surviving record, and the gap has to be refetched
/// rather than rendered as continuous.
#[tauri::command]
pub(crate) async fn watch_conversation(
    app: AppHandle,
    state: State<'_, AppState>,
    server: String,
    conversation: String,
    after: Option<String>,
) -> Result<(Conversation, Cursor), String> {
    let server = ServerId::parse(&server).ok_or_else(|| format!("{server} is not a server id"))?;
    let connection = state.connect(&server).await?;

    state
        .watches()
        .start(
            app,
            connection,
            Conversations::id(&conversation)?,
            after.map(Cursor),
        )
        .await
}

#[tauri::command]
pub(crate) async fn unwatch_conversation(
    state: State<'_, AppState>,
    conversation: String,
) -> Result<(), String> {
    state
        .watches()
        .stop(&Conversations::id(&conversation)?)
        .await;

    Ok(())
}

/// What this machine will let a conversation screen do, and how much it will
/// answer at a time.
///
/// Asked rather than read from the book. The capabilities recorded at the last
/// handshake can be a build old, and the page ceiling is not recorded at all —
/// both come from `Describe`, which is one request on the connection this screen
/// is about to use anyway. A machine that will not answer is treated as allowing
/// nothing: drawing a control that then fails on press teaches somebody the app
/// is unreliable.
#[tauri::command]
pub(crate) async fn conversation_controls(
    state: State<'_, AppState>,
    server: String,
) -> Result<ConversationControls, String> {
    let server = ServerId::parse(&server).ok_or_else(|| format!("{server} is not a server id"))?;
    let connection = state.connect(&server).await?;

    let payload = Rpc::request(&connection, Request::Describe)
        .await
        .map_err(|error| error.to_string())?;

    let Payload::Describe(describe) = payload else {
        return Err("the machine did not describe itself".to_string());
    };

    let has = |name: &str| describe.capabilities.has(name);

    Ok(ConversationControls {
        transcript_page: describe.limits.transcript_page,
        send: has(capability::PROMPT_SEND),
        answer: has(capability::QUESTIONS),
        interrupt: has(capability::INTERRUPT),
        resume: has(capability::CONVERSATION_RESUME),
        stop: has(capability::CONVERSATION_STOP),
        read_files: has(capability::ASSETS_READ),
        attach_files: has(capability::ASSETS_WRITE),
    })
}

/// Starts an agent again on a conversation that has stopped.
///
/// Idempotent by the machine's design: asking to resume one that is already
/// running gives that conversation back untouched, because two agents appending
/// to one set of records would corrupt the history this whole screen reads.
#[tauri::command]
pub(crate) async fn resume_conversation(
    state: State<'_, AppState>,
    server: String,
    conversation: String,
) -> Result<Conversation, String> {
    let server = ServerId::parse(&server).ok_or_else(|| format!("{server} is not a server id"))?;
    let connection = state.connect(&server).await?;

    let payload = Rpc::request(
        &connection,
        Request::ResumeConversation {
            conversation: Conversations::id(&conversation)?,
            cwd: None,
        },
    )
    .await
    .map_err(|error| error.to_string())?;

    match payload {
        Payload::Conversation(conversation) => Ok(conversation),
        other => Err(format!("the machine answered with {other:?}")),
    }
}

/// Sends a message to a running agent.
#[tauri::command]
pub(crate) async fn send_prompt(
    state: State<'_, AppState>,
    server: String,
    conversation: String,
    text: String,
    attachments: Vec<String>,
) -> Result<SendOutcome, String> {
    let server = ServerId::parse(&server).ok_or_else(|| format!("{server} is not a server id"))?;
    let connection = state.connect(&server).await?;

    // Staged uploads become real here, and only here. Sending is what commits
    // them, which is why a picked file that is never sent needs no undo.
    let attachments = attachments
        .iter()
        .map(|value| {
            AssetId::parse(value).ok_or_else(|| format!("{value} is not an asset id"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let answer = Rpc::request(
        &connection,
        Request::SendPrompt {
            conversation: Conversations::id(&conversation)?,
            text,
            attachments,
        },
    )
    .await;

    match answer {
        Ok(Payload::Ack) => Ok(SendOutcome::Sent),
        Ok(other) => Err(format!("the machine answered with {other:?}")),
        // Not a failure to report as one. There is no pane to type at, so the
        // answer is to offer a resume rather than a send that did not work.
        Err(ClientError::Wire(WireError::NotRunning { .. })) => Ok(SendOutcome::NotRunning),
        Err(error) => Err(error.to_string()),
    }
}

/// Answers a question the agent is blocked on.
///
/// The fingerprint is echoed back exactly as it arrived. The machine refuses a
/// stale one rather than answering a question that has since changed, which is
/// the whole point of carrying it: answering the wrong question is worse than
/// not answering.
#[tauri::command]
pub(crate) async fn answer_question(
    state: State<'_, AppState>,
    server: String,
    conversation: String,
    question: String,
    fingerprint: String,
    answers: Vec<Answer>,
) -> Result<(), String> {
    // On arrival, before anything can fail. Until this existed there was no way
    // to tell an answer that never left the screen from one the machine refused,
    // and the two have entirely different causes.
    log::info!(
        "answering {question} on {conversation} with {} answer(s)",
        answers.len()
    );

    let server = ServerId::parse(&server).ok_or_else(|| format!("{server} is not a server id"))?;
    let connection = state.connect(&server).await?;

    let request = Request::AnswerQuestion {
        conversation: Conversations::id(&conversation)?,
        question: QuestionId::parse(&question)
            .ok_or_else(|| format!("{question} is not a question id"))?,
        fingerprint: Fingerprint(fingerprint),
        answers,
    };

    // The same retry the transfers carry, and for the same reason: a QUIC path
    // closed for being idle leaves a cached connection that looks usable, and
    // the first request on it is the one that discovers otherwise. An answer is
    // the worst thing to lose that way - somebody chose it, and the agent is
    // stopped until it arrives.
    let payload = match Rpc::request(&connection, request.clone()).await {
        Ok(payload) => payload,
        Err(first) => {
            log::warn!("answer could not be carried, dialling again: {first}");

            let connection = state.reconnect(&server).await?;

            Rpc::request(&connection, request)
                .await
                .map_err(|error| error.to_string())?
        }
    };

    match payload {
        Payload::Ack => Ok(()),
        other => {
            // To the log as well as to the screen. An answer that does not land
            // is the one failure a person cannot work around by pressing the
            // control again, and the line on screen is gone as soon as they
            // scroll. The log is what survives long enough to read afterwards.
            log::warn!("answer refused for {conversation}: the machine answered with {other:?}");

            Err(format!("the machine answered with {other:?}"))
        }
    }
}

/// Stops what the agent is doing without ending the conversation.
#[tauri::command]
pub(crate) async fn interrupt_conversation(
    state: State<'_, AppState>,
    server: String,
    conversation: String,
) -> Result<(), String> {
    let server = ServerId::parse(&server).ok_or_else(|| format!("{server} is not a server id"))?;
    let connection = state.connect(&server).await?;

    let payload = Rpc::request(
        &connection,
        Request::Interrupt {
            conversation: Conversations::id(&conversation)?,
        },
    )
    .await
    .map_err(|error| error.to_string())?;

    match payload {
        Payload::Ack => Ok(()),
        other => Err(format!("the machine answered with {other:?}")),
    }
}
