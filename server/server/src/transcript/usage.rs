use serde::Deserialize;

/// What one model request cost, as the agent recorded it.
///
/// Every field is `#[serde(default)]`, because a record from a harness release
/// that stopped writing one must not fail the whole parse — a transcript that
/// will not read is a conversation a person cannot open, which is far worse than
/// a token count of zero.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(rename = "cache_creation_input_tokens", default)]
    pub cache_written: u64,
    #[serde(rename = "cache_read_input_tokens", default)]
    pub cache_read: u64,
}

impl Usage {
    /// Everything the model was sent for this request.
    ///
    /// Cached and uncached both. What a person wants from a context figure is
    /// how full the window is, and the window does not care which part of it
    /// came from a cache.
    pub fn context_used(&self) -> u64 {
        self.input_tokens
            .saturating_add(self.cache_written)
            .saturating_add(self.cache_read)
    }

    /// Whether this record carried a usage block at all.
    ///
    /// A record with nothing in it is one the harness wrote without usage —
    /// a tool result, a meta line — and it must not overwrite the figures from
    /// the request that is actually in flight.
    pub fn is_empty(&self) -> bool {
        self.input_tokens == 0
            && self.output_tokens == 0
            && self.cache_written == 0
            && self.cache_read == 0
    }
}
