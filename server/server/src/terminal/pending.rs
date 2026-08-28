use tethera_common::structs::transcript::Question;

/// A question, and what its rows were showing when it was read.
///
/// **The ticks stop at this server.** `Question` is a wire type and postcard
/// encodes struct fields positionally, so a field on `QuestionOption` would mean
/// a new `WireVersion` and every phone on the old one refused at the handshake.
/// Nothing on the client can draw a pre-ticked box today, so the field would buy
/// no behaviour and cost a forced update. It belongs there the day the client can
/// render it, in the same bump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingQuestion {
    pub question: Question,
    /// One entry per ask, aligned to that ask's options.
    ///
    /// `None` where the set did not come off a screen. The records cannot know
    /// what is ticked, and saying so is honest where reporting all-clear would be
    /// a guess the driver then acts on.
    pub ticks: Option<Vec<Vec<bool>>>,
}

impl PendingQuestion {
    pub fn new(question: Question, ticks: Option<Vec<Vec<bool>>>) -> Self {
        Self { question, ticks }
    }

    /// What the nth ask's rows were showing, where that is known at all.
    pub fn ticks_for(&self, ask: usize) -> Option<&[bool]> {
        self.ticks.as_ref()?.get(ask).map(Vec::as_slice)
    }
}
