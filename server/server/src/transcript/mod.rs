//! Reading an agent's own record of a session.
//!
//! The file is JSONL, appended to while the session runs, and one file is one
//! conversation for its whole life - a resume days later writes the same file
//! under the same id. So history is complete regardless of what any screen
//! retained, and the index below pages it by byte offset rather than re-reading
//! it.
//!
//! Nothing here names an agent. Where the records live, what the harness injects
//! under the person's role, and which tool call means what are all read off
//! `AgentTrait`, so a second harness is another arm rather than a branch here.

mod assets;
mod attachment;
mod body;
mod budget;
mod catalog;
mod content;
mod grouping;
mod index;
mod index_of_assets;
mod mapper;
mod message;
mod origin;
mod reader;
mod record;
mod results;
mod sent;
mod span;
mod stats;
mod status;
mod summary;
mod turn_span;
mod usage;
mod watcher;

pub use assets::AssetNaming;
pub use budget::PageBudget;
pub use index_of_assets::AssetIndex;
pub use attachment::Attachment;
pub use body::MessageBody;
pub use catalog::SessionCatalog;
pub use content::ContentBlock;
pub use grouping::TurnGrouping;
pub use index::TranscriptIndex;
pub use mapper::TurnMapper;
pub use message::Message;
pub use origin::AttachmentOrigin;
pub use reader::TranscriptReader;
pub use record::Record;
pub use results::ToolOutcome;
pub use sent::SentFiles;
pub use span::RecordSpan;
pub use stats::StatsRule;
pub use status::StatusRule;
pub use summary::SessionSummary;
pub use turn_span::TurnSpan;
pub use usage::Usage;
pub use watcher::TranscriptWatcher;
