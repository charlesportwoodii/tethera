use curia::{LineFormatter, LogEvent};
use std::sync::Arc;

/// One log line for a person reading a terminal.
///
/// `HH:MM:SS LEVEL target: message key=value`
///
/// The time is local and carries no date: this is read live, beside the command
/// that produced it, and a full timestamp on every line pushes the message past
/// the width anybody reads to.
pub struct HumanFormatter;

impl HumanFormatter {
    pub fn formatter() -> LineFormatter {
        Arc::new(|event: &LogEvent| {
            let mut line = format!(
                "{} {:<5} {}: {}",
                event.timestamp.format("%H:%M:%S"),
                event.level.as_str().to_uppercase(),
                event.target,
                event.message
            );

            for (key, value) in event.fields.as_map() {
                line.push_str(&format!(" {key}={}", Self::render(value)));
            }

            line
        })
    }

    // A string field is printed bare. Quoting every value turns `endpoint_id=0ba4`
    // into `endpoint_id="0ba4"`, which is noise on the one field type that is
    // almost always a string.
    fn render(value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::String(text) => text.clone(),
            other => other.to_string(),
        }
    }
}
