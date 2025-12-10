use crate::error::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

/// Enumerates the terminal states for operation envelopes.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    /// Operation is still running.
    InProgress,
    /// Operation completed successfully.
    Succeeded,
    /// Operation failed.
    Failed,
}

/// Serializable telemetry event emitted for every MCP tool invocation.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct OperationEnvelope {
    pub session_id: String,
    pub capability_id: String,
    pub handle: Option<String>,
    #[serde(
        serialize_with = "serialize_timestamp",
        deserialize_with = "deserialize_timestamp"
    )]
    #[schemars(with = "String")]
    pub started_at: OffsetDateTime,
    #[serde(
        serialize_with = "serialize_timestamp_option",
        deserialize_with = "deserialize_timestamp_option"
    )]
    #[schemars(with = "Option<String>")]
    pub completed_at: Option<OffsetDateTime>,
    pub status: OperationStatus,
    pub error: Option<String>,
    pub notes: Vec<String>,
}

impl OperationEnvelope {
    /// Creates a new envelope tied to a session/capability pair.
    pub fn new(session_id: impl Into<String>, capability_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            capability_id: capability_id.into(),
            handle: None,
            started_at: OffsetDateTime::now_utc(),
            completed_at: None,
            status: OperationStatus::InProgress,
            error: None,
            notes: Vec::new(),
        }
    }

    /// Sets the resource handle associated with this operation.
    pub fn with_handle(mut self, handle: impl Into<String>) -> Self {
        self.handle = Some(handle.into());
        self
    }

    /// Records a free-form diagnostic note.
    pub fn push_note(&mut self, note: impl Into<String>) {
        self.notes.push(note.into());
    }

    /// Marks the operation as successful and captures the completion timestamp.
    pub fn mark_succeeded(mut self) -> Self {
        self.status = OperationStatus::Succeeded;
        self.completed_at = Some(OffsetDateTime::now_utc());
        self
    }

    /// Marks the operation as failed and records the supplied error message.
    pub fn mark_failed(mut self, error: impl Into<String>) -> Self {
        self.status = OperationStatus::Failed;
        self.error = Some(error.into());
        self.completed_at = Some(OffsetDateTime::now_utc());
        self
    }
}

/// Append-only telemetry sink that serializes envelopes as NDJSON.
pub struct OperationEnvelopeSink {
    path: PathBuf,
    file: Mutex<File>,
}

impl OperationEnvelopeSink {
    /// Opens (or creates) the sink at the desired path.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        Ok(Self {
            path,
            file: Mutex::new(file),
        })
    }

    /// Returns the filesystem path backing this sink.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Appends the serialized envelope to the sink.
    pub fn append(&self, envelope: &OperationEnvelope) -> Result<()> {
        let mut file = self.file.lock().expect("poisoned telemetry mutex");
        serde_json::to_writer(&mut *file, envelope)?;
        file.write_all(b"\n")?;
        file.flush()?;
        Ok(())
    }
}

fn serialize_timestamp<S>(value: &OffsetDateTime, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let rendered = value
        .format(&Rfc3339)
        .map_err(serde::ser::Error::custom)?;
    serializer.serialize_str(&rendered)
}

fn deserialize_timestamp<'de, D>(deserializer: D) -> std::result::Result<OffsetDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    OffsetDateTime::parse(&raw, &Rfc3339).map_err(serde::de::Error::custom)
}

fn serialize_timestamp_option<S>(
    value: &Option<OffsetDateTime>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(ts) => serializer.serialize_some(&ts.format(&Rfc3339).map_err(serde::ser::Error::custom)?),
        None => serializer.serialize_none(),
    }
}

fn deserialize_timestamp_option<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<OffsetDateTime>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw: Option<String> = Option::<String>::deserialize(deserializer)?;
    match raw {
        Some(raw) => Ok(Some(
            OffsetDateTime::parse(&raw, &Rfc3339).map_err(serde::de::Error::custom)?,
        )),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn sink_appends_json_lines() {
        let dir = tempdir().unwrap();
        let sink = OperationEnvelopeSink::new(dir.path().join("telemetry.jsonl")).unwrap();
        let envelope = OperationEnvelope::new("session", "capability").with_handle("spec://foo");
        sink.append(&envelope).expect("append succeeds");
        let contents = std::fs::read_to_string(sink.path()).unwrap();
        assert!(contents.contains("\"session_id\":\"session\""));
    }
}
