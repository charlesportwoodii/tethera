/// Where one record's line sits in the file.
///
/// The index holds these rather than the lines themselves. One session on the
/// development machine is 57.5 MB, and holding bodies would make every read of
/// it an allocation of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordSpan {
    pub offset: u64,
    pub len: usize,
}
