//! Serializable job snapshot types.
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::PathBuf;

/// Opaque job identifier. Serializes as a decimal string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(pub u64);

impl Serialize for JobId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for JobId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map(JobId).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for JobId {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "JobId".into()
    }

    fn json_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
        String::json_schema(g)
    }
}

/// Lifecycle of a job. `Done`, `Failed` and `Cancelled` are terminal. JSON:
/// `{"kind":"queued"}`, ..., `{"kind":"failed","message":"..."}`.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    Done,
    Failed(String),
    Cancelled,
}

/// Snapshot of a job. `progress` is 0.0..=1.0. `notes` are things the user
/// should know about a finished result (e.g. images compress left as they
/// were), not errors; omitted from JSON when empty.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Job {
    pub id: JobId,
    pub state: JobState,
    pub progress: f32,
    pub outputs: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}
