//! Serde inputs of the conversions (GUI commands and MCP tools).
use super::{ImageFormat, PageSize};
use crate::names::default_output;
use crate::output::{check_out_dir, dir_or_beside};
use crate::pages::PagesError;
use crate::security::Unlocked;
use crate::{ProgressSink, Secret};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Makes one PDF from PNG and JPEG images: one page per image, in order.
/// Without `page_size` each page is its image's size at 1 pixel = 1 point;
/// with one, the image is fitted and centred on it, aspect kept.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImagesToPdfInput {
    /// Absolute paths of 1..=500 PNG or JPEG images, in page order.
    pub inputs: Vec<PathBuf>,
    /// Fixed page size in points; omitted: each page is its image's size.
    #[serde(default)]
    pub page_size: Option<PageSize>,
    /// Absolute output path; defaults to `<stem>-images.pdf` beside the
    /// first image. A taken name gets `-1`, `-2`, ...
    #[serde(default)]
    pub out: Option<PathBuf>,
}

impl ImagesToPdfInput {
    /// Runs [`super::images_to_pdf`] (progress and cancel through
    /// `progress`); returns the path written.
    ///
    /// # Errors
    /// As [`super::images_to_pdf`].
    pub fn run(self, progress: &ProgressSink) -> Result<PathBuf, PagesError> {
        let first = self.inputs.first().ok_or_else(no_images)?;
        crate::check_input_file(first)?; // its error, not the default out's
        let out = (self.out.clone()).unwrap_or_else(|| default_output(first, "images"));
        super::images_to_pdf(&self.inputs, &out, self.page_size, progress)
    }
}

pub(super) fn no_images() -> PagesError {
    PagesError::InvalidInput("no images given".into())
}

/// Renders pages of a PDF as displayed (CropBox and rotation applied) to
/// PNG or JPEG files `<stem>-<n>.<png|jpg>`, `n` the 1-based page number.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PdfToImagesInput {
    /// Absolute path of the source PDF.
    pub input: PathBuf,
    /// Absolute path of an existing output directory; defaults to the
    /// input's directory. A taken name gets `-1`, `-2`, ...
    #[serde(default)]
    pub out_dir: Option<PathBuf>,
    /// Resolution, 72..=600 dots per inch; default 150.
    #[serde(default = "dpi")]
    pub dpi: u32,
    /// `png` (default) or `jpeg`.
    #[serde(default)]
    pub format: ImageFormat,
    /// JPEG quality 1..=100, default 85; used only for `jpeg`.
    #[serde(default = "quality")]
    pub jpeg_quality: u8,
    /// 0-based pages to render, in output order; every page when omitted.
    #[serde(default)]
    pub pages: Option<Vec<usize>>,
    /// Secret: password (user or owner) that opens an encrypted `input`.
    /// Never logged or echoed.
    #[serde(default)]
    pub password: Option<Secret>,
}

fn dpi() -> u32 {
    150
}

fn quality() -> u8 {
    85
}

impl PdfToImagesInput {
    /// Runs [`super::pdf_to_images`] (progress and cancel through
    /// `progress`) on the input, opened with its password; returns the
    /// files written, in order, and the job's notes.
    ///
    /// # Errors
    /// As [`super::pdf_to_images`], plus `WrongPassword`.
    pub fn run(self, progress: &ProgressSink) -> Result<(Vec<PathBuf>, Vec<String>), PagesError> {
        let dir = dir_or_beside::<PagesError>(&self.out_dir, &self.input)?;
        check_out_dir(&dir)?;
        Unlocked::single(&self.input, self.password, None, |input| {
            let pages = self.pages.as_deref();
            let (dpi, q) = (self.dpi, self.jpeg_quality);
            super::pdf_to_images(input, &dir, dpi, self.format, q, pages, progress)
        })
    }
}

/// Converts a document (docx, doc, odt, rtf, txt, html, htm, md) to PDF
/// with the bundled WordCraft sidecar (`wordcraft` of `pdf_capabilities`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OfficeToPdfInput {
    /// Absolute path of the document (docx, doc, odt, rtf, txt, html, htm,
    /// md).
    pub input: PathBuf,
    /// Absolute path of an existing output directory; defaults to the
    /// input's directory. The output is `<stem>.pdf`; a taken name gets
    /// `-1`, `-2`, ...
    #[serde(default)]
    pub out_dir: Option<PathBuf>,
}

impl OfficeToPdfInput {
    /// Runs [`super::office_to_pdf`] (cancel through `progress`); returns
    /// the path written.
    ///
    /// # Errors
    /// As [`super::office_to_pdf`].
    pub fn run(self, progress: &ProgressSink) -> Result<PathBuf, PagesError> {
        let dir = dir_or_beside::<PagesError>(&self.out_dir, &self.input)?;
        super::office_to_pdf(&self.input, &dir, progress)
    }
}
