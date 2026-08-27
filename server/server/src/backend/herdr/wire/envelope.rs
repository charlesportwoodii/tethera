use super::Failure;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use crate::backend::error::BackendError;

/// Every herdr socket-API answer, success or failure.
///
/// Both bodies are optional in one type rather than an untagged enum: postcard
/// is not involved here, but an untagged enum reports a mismatch as "did not
/// match any variant", which throws away the parse error that says which field
/// was wrong.
#[derive(Debug, Clone, Deserialize)]
pub struct Envelope<T> {
    pub id: Option<String>,
    pub result: Option<T>,
    pub error: Option<Failure>,
}

impl<T: DeserializeOwned> Envelope<T> {
    pub fn decode(raw: &str) -> Result<Self, BackendError> {
        serde_json::from_str(raw).map_err(|error| BackendError::Backend {
            message: format!("herdr answered unreadable json: {error}"),
        })
    }

    /// The error is checked first: a failure envelope carries no `result`, and
    /// reporting "no result" for it would lose the code that says why.
    pub fn into_result(self) -> Result<T, BackendError> {
        if let Some(failure) = self.error {
            return Err(failure.into_backend());
        }

        self.result.ok_or(BackendError::Backend {
            message: "herdr answered with neither a result nor an error".to_string(),
        })
    }
}
