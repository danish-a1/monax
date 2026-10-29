//! The serde input of [`super::run`] (GUI command and MCP tool).
use crate::pages::PagesError;
use crate::security::Unlocked;
use crate::{ProgressSink, Secret};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Makes a scanned PDF searchable: every page is rendered as displayed and
/// read by tesseract, and the output's pages are those images with an
/// invisible text layer over them, at the same displayed size. Links, form
/// fields, annotations and bookmarks are not carried over.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OcrInput {
    /// Absolute path of the source PDF.
    pub input: PathBuf,
    /// Languages of the text: tesseract codes (`eng`, `deu`, `chi_sim`),
    /// read together. Installed ones: `ocr_languages` of
    /// `pdf_capabilities`. Default `["eng"]`.
    #[serde(default = "eng")]
    pub lang: Vec<String>,
    /// Resolution pages are rendered at for reading, 72..=600; default 300.
    #[serde(default = "dpi")]
    pub dpi: u32,
    /// Secret: password (user or owner) that opens an encrypted `input`.
    /// Never logged or echoed; the output is not encrypted.
    #[serde(default)]
    pub password: Option<Secret>,
    /// Absolute output path; defaults to `<stem>-ocr.pdf` beside the
    /// input. A taken name gets `-1`, `-2`, ...
    #[serde(default)]
    pub out: Option<PathBuf>,
}

fn eng() -> Vec<String> {
    vec!["eng".into()]
}

fn dpi() -> u32 {
    300
}

impl OcrInput {
    /// Runs [`super::run`] (progress and cancel through `progress`) on the
    /// input, opened with its password; returns the path written and the
    /// notes of opening it.
    ///
    /// # Errors
    /// As [`super::run`], plus `WrongPassword`.
    pub fn run(self, progress: &ProgressSink) -> Result<(PathBuf, Vec<String>), PagesError> {
        let out =
            (self.out.clone()).unwrap_or_else(|| crate::names::default_output(&self.input, "ocr"));
        Unlocked::single(&self.input, self.password, Some(&out), |p| {
            super::run(p, &out, &self.lang, self.dpi, progress)
        })
    }
}
