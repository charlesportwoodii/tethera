use curia::{LineFormatter, LogEvent};
use serde_json::{json, Value};
use std::sync::Arc;

/// One log line as a JSON object, for the file sink.
///
/// Fields are nested under `fields` rather than flattened into the object, so a
/// field called `level` or `message` cannot overwrite the record's own.
pub struct JsonFormatter;

impl JsonFormatter {
    pub fn formatter() -> LineFormatter {
        Arc::new(|event: &LogEvent| {
            let mut record = json!({
                "ts": event.timestamp.to_rfc3339(),
                "level": event.level.as_str(),
                "target": event.target,
                "message": event.message,
            });

            if !event.fields.is_empty() {
                record["fields"] = Value::Object(event.fields.as_map().clone());
            }

            // A record that cannot be rendered still has to reach the file, or
            // the one line explaining a failure is the line that goes missing.
            if let (Some(file), Some(line)) = (&event.file, event.line) {
                record["at"] = json!(format!("{file}:{line}"));
            }

            record.to_string()
        })
    }
}
