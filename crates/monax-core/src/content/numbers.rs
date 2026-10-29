//! Sequential numbers: page numbers and Bates numbers.
use super::overlay::{Stamp, stamp};
use super::text::expand;
use super::{Anchor, PagesError};
use crate::pdfium::PdfiumError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Options of [`page_numbers`]. Sizes and margins in points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct NumberOpts {
    /// Text per page; `%[page]` is the page's number, `%[pages]` the
    /// document's page count.
    pub format: String,
    /// Anchor on the page as displayed.
    pub position: Anchor,
    /// Points from the displayed page edges.
    pub margin: f32,
    /// Font size in points.
    pub size: f32,
    /// Number shown on the first selected page.
    pub start_at: u32,
    /// 0-based pages to number; `None` = all.
    pub pages: Option<Vec<usize>>,
}

impl Default for NumberOpts {
    fn default() -> Self {
        Self {
            format: "%[page]".into(),
            position: Anchor::BottomCenter,
            margin: 24.0,
            size: 10.0,
            start_at: 1,
            pages: None,
        }
    }
}

/// Options of [`bates`]. Sizes and margins in points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct BatesOpts {
    /// Text before the number.
    pub prefix: String,
    /// Text after the number.
    pub suffix: String,
    /// Number on the first selected page.
    pub start: u64,
    /// Minimum digits, zero-padded: 1..=12.
    pub digits: u8,
    /// Anchor on the page as displayed.
    pub position: Anchor,
    /// Points from the displayed page edges.
    pub margin: f32,
    /// Font size in points.
    pub size: f32,
    /// 0-based pages to number; `None` = all.
    pub pages: Option<Vec<usize>>,
}

impl Default for BatesOpts {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            suffix: String::new(),
            start: 1,
            digits: 6,
            position: Anchor::BottomRight,
            margin: 24.0,
            size: 10.0,
            pages: None,
        }
    }
}

/// Stamps page numbers onto the selected pages: the k-th selected page (in
/// page order, from 0) shows `format` with `%[page]` = `start_at + k`.
///
/// # Errors
/// `InvalidInput` (bad paths, options, unsupported characters, text that
/// does not fit), `PageOutOfRange`, `EncryptedInput`, `Damaged`, `Pdfium`,
/// `Io`.
pub fn page_numbers(input: &Path, out: &Path, opts: NumberOpts) -> Result<PathBuf, PagesError> {
    let o = &opts;
    stamp(input, out, o.pages.as_deref(), |k, _, count| {
        let n = u64::from(o.start_at) + k as u64;
        let t = expand(&o.format, n, count);
        Ok(vec![Stamp::plain(t, o.position, o.margin, o.size)])
    })
}

/// Stamps Bates numbers onto the selected pages: the k-th selected page (in
/// page order, from 0) shows `prefix`, `start + k` zero-padded to `digits`,
/// then `suffix`.
///
/// # Errors
/// As [`page_numbers`]; `InvalidInput` when `digits` is not 1..=12 or a
/// number needs more than `digits` digits.
pub fn bates(input: &Path, out: &Path, opts: BatesOpts) -> Result<PathBuf, PagesError> {
    let o = &opts;
    let digits = usize::from(o.digits);
    if !(1..=12).contains(&digits) {
        return Err(PagesError::InvalidInput(format!(
            "digits must be 1..=12, got {digits}"
        )));
    }
    stamp(input, out, o.pages.as_deref(), |k, _, _| {
        let n = o
            .start
            .checked_add(k as u64)
            .map(|n| format!("{n:0digits$}"));
        let Some(n) = n.filter(|n| n.len() <= digits) else {
            return Err(PdfiumError::InvalidInput(format!(
                "Bates number {} + {k} needs more than {digits} digits",
                o.start
            )));
        };
        let t = format!("{}{n}{}", o.prefix, o.suffix);
        Ok(vec![Stamp::plain(t, o.position, o.margin, o.size)])
    })
}
