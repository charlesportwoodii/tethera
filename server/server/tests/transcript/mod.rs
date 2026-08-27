mod assets;
mod catalog;
mod grouping;
mod index;
mod mapper;
mod reader;
mod sent;

use std::path::PathBuf;
use tethera_common::structs::agent::Agent;
use tethera_common::structs::transcript::{Part, Turn};
use tethera_server_lib::transcript::TranscriptReader;

/// A fixture reader, opened on a committed redacted transcript.
pub struct Fixture;

impl Fixture {
    pub fn path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("transcripts")
            .join(name)
    }

    pub fn reader(name: &str) -> TranscriptReader {
        TranscriptReader::open(Self::path(name), Agent::Claude)
    }

    /// Every turn in a fixture, oldest first.
    pub fn turns(name: &str) -> Vec<Turn> {
        let mut reader = Self::reader(name);

        reader.page(None, u16::MAX).expect("page").items
    }

    pub fn texts(turn: &Turn) -> Vec<String> {
        turn.parts
            .iter()
            .filter_map(|part| match part {
                Part::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn kinds(turn: &Turn) -> Vec<&'static str> {
        turn.parts.iter().map(Part::kind).collect()
    }

    /// Every part of every turn, flattened, for the filtering cases where which
    /// turn a part landed in is not the point.
    pub fn parts(name: &str) -> Vec<Part> {
        Self::turns(name)
            .into_iter()
            .flat_map(|turn| turn.parts)
            .collect()
    }
}
