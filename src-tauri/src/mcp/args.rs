//! Arguments of the app-level MCP tools (job status, cancel, open); op
//! tools take the `monax-core` input structs.
use monax_core::JobId;
use serde::Deserialize;
use std::path::PathBuf;

/// No arguments.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct NoArgs {}

/// Identifies a job.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct JobArgs {
    /// `job_id` returned by a `pdf_*` tool (decimal string).
    pub job_id: JobId,
}

/// A document to show in the Monax window.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct OpenArgs {
    /// Absolute path of an existing PDF.
    pub path: PathBuf,
}
