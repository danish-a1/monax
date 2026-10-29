//! Read-only queries whose result is written to a new file, so an agent gets
//! it as a job output: page info (`.json`), text (`.txt`), page image (`.png`).
//! The input is never modified; outputs follow the copy-on-save rules of
//! `pages` (`out` defaults to `<stem>-<tool>.<ext>` beside the input, a taken
//! name gets `-1`, `-2`, ..., a failed query leaves no file).
use crate::pages::{PagesError, check_pages, write_bytes};
use crate::pdfium::{self, PdfiumError};
use crate::security::Unlocked;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// `out`, else `<stem>-<tool>.<ext>` beside `input`.
pub(crate) fn dest(out: &Option<PathBuf>, input: &Path, tool: &str, ext: &str) -> PathBuf {
    out.clone()
        .unwrap_or_else(|| crate::names::default_output(input, tool).with_extension(ext))
}

/// Writes a .json with `page_count` and each page's `media_box` and `crop_box`
/// (`[left, bottom, right, top]`, PDF user space points, origin bottom-left,
/// y-up, unrotated MediaBox) and clockwise `rotation`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PageInfoInput {
    /// Absolute path of the PDF to inspect.
    pub input: PathBuf,
    /// Absolute output path of the `.json`; defaults beside the input.
    #[serde(default)]
    pub out: Option<PathBuf>,
    /// Secret: password (user or owner) that opens an encrypted `input`.
    /// Never logged or echoed.
    #[serde(default)]
    pub password: Option<crate::Secret>,
}

impl PageInfoInput {
    /// Writes `{"page_count": n, "pages": [{"media_box", "crop_box",
    /// "rotation"}, ...]}` (boxes as described on the fields, in points).
    ///
    /// # Errors
    /// `InvalidInput` (bad path or `out`), `Damaged`, `EncryptedInput`,
    /// `WrongPassword`, `Pdfium` (library), `Io`.
    pub fn run(self) -> Result<(PathBuf, Vec<String>), PagesError> {
        let out = dest(&self.out, &self.input, "page-info", "json");
        Unlocked::single(&self.input, self.password, Some(&out), |input| {
            Ok(page_info(input, &out)?)
        })
    }
}

fn page_info(input: &Path, out: &Path) -> Result<PathBuf, PdfiumError> {
    let info = pdfium::page_info(input, None)?;
    let json = serde_json::to_vec(&info).map_err(|e| PdfiumError::Io(e.into()))?;
    write_bytes(out, input, &json)
}

/// Writes the text of the chosen pages (0-based) to a UTF-8 .txt, pages
/// separated by a form feed.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReadTextInput {
    /// Absolute path of the PDF to read.
    pub input: PathBuf,
    /// 0-based page indices, in output order; every page when omitted.
    #[serde(default)]
    pub pages: Option<Vec<usize>>,
    /// Absolute output path of the `.txt`; defaults beside the input.
    #[serde(default)]
    pub out: Option<PathBuf>,
    /// Secret: password (user or owner) that opens an encrypted `input`.
    /// Never logged or echoed.
    #[serde(default)]
    pub password: Option<crate::Secret>,
}

impl ReadTextInput {
    /// Writes each chosen page's text in content-stream order, pages
    /// separated by a form feed (`\f`).
    ///
    /// # Errors
    /// `InvalidInput` (bad path, `out`, or empty `pages`), `PageOutOfRange`,
    /// `Damaged`, `EncryptedInput`, `WrongPassword`, `Pdfium` (library), `Io`.
    pub fn run(self) -> Result<(PathBuf, Vec<String>), PagesError> {
        let out = dest(&self.out, &self.input, "read-text", "txt");
        Unlocked::single(&self.input, self.password, Some(&out), |input| {
            Ok(read_text(input, self.pages.as_deref(), &out)?)
        })
    }
}

fn read_text(input: &Path, pages: Option<&[usize]>, out: &Path) -> Result<PathBuf, PdfiumError> {
    let all = pdfium::page_texts(input)?;
    let text = match pages {
        None => all.join("\u{c}"),
        Some(pages) => {
            check_pages(pages, all.len())?;
            let picked: Vec<&str> = pages.iter().map(|&i| all[i].as_str()).collect();
            picked.join("\u{c}")
        }
    };
    write_bytes(out, input, text.as_bytes())
}

fn default_max_px() -> u32 {
    1024
}

/// Renders one page (0-based) as the user sees it (CropBox and rotation
/// applied) to a .png whose longer side is `max_px`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderPageInput {
    /// Absolute path of the PDF to render.
    pub input: PathBuf,
    /// 0-based page index.
    pub page: usize,
    /// Longer side of the image in pixels, 16..=1024.
    #[serde(default = "default_max_px")]
    pub max_px: u32,
    /// Absolute output path of the `.png`; defaults beside the input.
    #[serde(default)]
    pub out: Option<PathBuf>,
    /// Secret: password (user or owner) that opens an encrypted `input`.
    /// Never logged or echoed.
    #[serde(default)]
    pub password: Option<crate::Secret>,
}

impl RenderPageInput {
    /// Renders `page` with CropBox and /Rotate applied, longer side `max_px`.
    ///
    /// # Errors
    /// `InvalidInput` (bad path, `out` or `max_px`), `PageOutOfRange`,
    /// `Damaged`, `EncryptedInput`, `WrongPassword`, `Pdfium` (library), `Io`.
    pub fn run(self) -> Result<(PathBuf, Vec<String>), PagesError> {
        let out = dest(&self.out, &self.input, "render-page", "png");
        Unlocked::single(&self.input, self.password, Some(&out), |input| {
            let png = pdfium::thumbnail_png(input, self.page, self.max_px)?;
            write_bytes(&out, input, &png)
        })
    }
}
