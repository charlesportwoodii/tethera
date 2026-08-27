use super::Rect;
use serde::Deserialize;

/// One divider in a tab's split tree.
///
/// `direction` is a `String` because nothing here reads it. How a tab is laid
/// out is the desk's business; this protocol has no operation that changes it.
#[derive(Debug, Clone, Deserialize)]
pub struct LayoutSplit {
    pub id: String,
    pub direction: String,
    pub ratio: f32,
    pub rect: Rect,
}
