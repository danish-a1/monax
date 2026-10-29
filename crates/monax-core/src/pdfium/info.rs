//! Page count, size and rotation.
use super::geometry::degrees;
use super::{PdfiumError, count_of, geometry, page_of, with_doc, with_doc_opened};
use crate::Secret;
use serde::Serialize;
use std::path::Path;

/// Page count and each page's boxes, as the page-info query reports them.
#[derive(Debug, Clone, Serialize)]
pub struct PageInfo {
    pub page_count: usize,
    pub pages: Vec<PageBoxes>,
}

/// One page's boxes: `[left, bottom, right, top]` in PDF user space points
/// (origin bottom-left, y-up, unrotated MediaBox).
#[derive(Debug, Clone, Serialize)]
pub struct PageBoxes {
    pub media_box: [f32; 4],
    /// CropBox clipped to the MediaBox: the visible area.
    pub crop_box: [f32; 4],
    /// /Rotate, clockwise degrees: 0, 90, 180 or 270.
    pub rotation: u16,
}

/// [`PageInfo`] of the document at `path`, opened with `password` (a secret,
/// user or owner; handed to pdfium in memory) when given.
///
/// # Errors
/// As [`page_count_with_password`].
pub fn page_info(path: &Path, password: Option<&Secret>) -> Result<PageInfo, PdfiumError> {
    with_doc_opened(path, password, |engine, doc| {
        let pages = (0..count_of(doc))
            .map(|i| {
                let g = geometry(engine, doc, i)?;
                Ok(PageBoxes {
                    media_box: g.media.into(),
                    crop_box: g.visible.into(),
                    rotation: g.rotation,
                })
            })
            .collect::<Result<Vec<_>, PdfiumError>>()?;
        Ok(PageInfo {
            page_count: pages.len(),
            pages,
        })
    })
}

/// Number of pages in the document at `path` (absolute, regular file).
///
/// # Errors
/// `InvalidInput` (bad path), `Damaged`, `EncryptedInput`, `Library`.
pub fn page_count(path: &Path) -> Result<usize, PdfiumError> {
    page_count_with_password(path, None)
}

/// [`page_count`] of a document opened with `password` (a secret, user or
/// owner; handed to pdfium in memory) when given.
///
/// # Errors
/// As [`page_count`], plus `WrongPassword`.
pub fn page_count_with_password(
    path: &Path,
    password: Option<&Secret>,
) -> Result<usize, PdfiumError> {
    with_doc_opened(path, password, |_, doc| Ok(count_of(doc)))
}

/// Whether the document at `path`, which opens without a password, has a
/// security handler (owner restrictions). In process: no qpdf needed.
///
/// # Errors
/// As [`page_count`] (`EncryptedInput` when it needs a password).
pub(crate) fn has_security(path: &Path) -> Result<bool, PdfiumError> {
    use pdfium_render::prelude::PdfSecurityHandlerRevision as Rev;
    with_doc(path, |_, doc| {
        // AES-256 (revision 6) is unknown to pdfium-render: an error, not Unprotected.
        let rev = doc.permissions().security_handler_revision();
        Ok(!matches!(rev, Ok(Rev::Unprotected)))
    })
}

/// Unrotated MediaBox `(width, height)` of 0-based `page`, in points; also
/// when the box is inherited from the /Pages tree.
///
/// # Errors
/// As [`page_count`], plus `PageOutOfRange`.
pub fn page_size(path: &Path, page: usize) -> Result<(f32, f32), PdfiumError> {
    with_doc(path, |engine, doc| {
        let (l, b, r, t) = geometry(engine, doc, page)?.media;
        Ok((r - l, t - b))
    })
}

/// Displayed `(width, height)` of 0-based `page` in points: the CropBox
/// clipped to the MediaBox (inherited boxes honoured), swapped for /Rotate 90
/// or 270.
///
/// # Errors
/// As [`page_size`].
pub fn displayed_size(path: &Path, page: usize) -> Result<(f32, f32), PdfiumError> {
    with_doc(path, |engine, doc| {
        Ok(geometry(engine, doc, page)?.displayed())
    })
}

/// /Rotate of 0-based `page`, normalized to 0, 90, 180 or 270 degrees clockwise.
///
/// # Errors
/// As [`page_size`].
pub fn rotation(path: &Path, page: usize) -> Result<u16, PdfiumError> {
    with_doc(path, |_, doc| degrees(&page_of(doc, page)?))
}

/// Effective CropBox `(left, bottom, right, top)` of 0-based `page` in PDF user
/// space points (unrotated): CropBox clipped to the MediaBox, inheritance
/// honoured; the MediaBox when the page has no CropBox.
///
/// # Errors
/// As [`page_size`].
pub fn crop_box(path: &Path, page: usize) -> Result<(f32, f32, f32, f32), PdfiumError> {
    with_doc(path, |engine, doc| Ok(geometry(engine, doc, page)?.visible))
}
