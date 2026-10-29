//! pdfium page edits that change single pages: crop and insert blank.
use super::{PagesError, check_pages, write_bytes};
use crate::pdfium::{
    self, PdfiumError, count_of, idx, lib, pin_boxes, save, set_visible, with_doc,
};
use pdfium_render::prelude::{PdfPagePaperSize, PdfPoints};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A rectangle in PDF user space: points, origin bottom-left, y up, relative
/// to the unrotated MediaBox.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Rect {
    /// Left edge x, points.
    pub left: f32,
    /// Bottom edge y, points (y grows upward).
    pub bottom: f32,
    /// Right edge x, points (> `left`).
    pub right: f32,
    /// Top edge y, points (> `bottom`).
    pub top: f32,
}

/// Sets the CropBox of the 0-based `pages` (duplicates ignored) to `rect`
/// (user space points, unrotated MediaBox space; replaces any existing
/// CropBox). Other pages are untouched. Returns the path written.
///
/// # Errors
/// `InvalidInput` (bad paths, empty `pages`, `rect` empty, not finite or not
/// inside a target page's MediaBox), `PageOutOfRange`, `EncryptedInput`,
/// `Damaged`, `Pdfium`, `Io`.
pub fn crop(input: &Path, pages: &[usize], rect: Rect, out: &Path) -> Result<PathBuf, PagesError> {
    let bytes = with_doc(input, |engine, doc| {
        check_pages(pages, count_of(doc))?;
        for &p in pages {
            let (l, b, r, t) = pin_boxes(engine, doc, p)?.media;
            let finite = [rect.left, rect.bottom, rect.right, rect.top]
                .iter()
                .all(|v| v.is_finite());
            if !finite || rect.left >= rect.right || rect.bottom >= rect.top {
                return Err(PdfiumError::InvalidInput("crop rectangle is empty".into()));
            }
            if rect.left < l || rect.bottom < b || rect.right > r || rect.top > t {
                return Err(PdfiumError::InvalidInput(format!(
                    "crop rectangle is outside the MediaBox of page {p}"
                )));
            }
            let mut page = pdfium::page_of(doc, p)?;
            set_visible(&mut page, (rect.left, rect.bottom, rect.right, rect.top))?;
        }
        save(doc)
    })?;
    write_bytes(out, input, &bytes)
}

/// Inserts a blank page after each 0-based index in `after` (duplicates
/// ignored; inserting before page 0 is not supported). Each blank page, with
/// no /Rotate, has the displayed size of the page it follows, in points: its
/// visible box (CropBox clipped to the MediaBox), swapped for /Rotate 90 or
/// 270. Returns the path written.
///
/// # Errors
/// `InvalidInput` (bad paths, empty `after`), `PageOutOfRange`,
/// `EncryptedInput`, `Damaged`, `Pdfium`, `Io`.
pub fn insert_blank(input: &Path, after: &[usize], out: &Path) -> Result<PathBuf, PagesError> {
    let mut at = after.to_vec();
    at.sort_unstable_by(|a, b| b.cmp(a)); // back to front keeps earlier indices valid
    at.dedup();
    let bytes = with_doc(input, |engine, doc| {
        check_pages(after, count_of(doc))?;
        for p in at {
            let (w, h) = pin_boxes(engine, doc, p)?.displayed();
            let size = PdfPagePaperSize::from_points(PdfPoints::new(w), PdfPoints::new(h));
            doc.pages_mut()
                .create_page_at_index(size, idx(p + 1)?)
                .map_err(lib)?;
        }
        save(doc)
    })?;
    write_bytes(out, input, &bytes)
}
