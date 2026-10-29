//! Text extraction.
use super::{PdfiumError, count_of, page_of, with_doc};
use pdfium_render::prelude::PdfDocument;
use std::ops::RangeInclusive;
use std::path::Path;

/// Every character of page `i` in content-stream order. Not `PdfPageText::all`:
/// that bounds by the displayed size at the user space origin, which cuts
/// text on rotated or cropped pages.
fn text_of(doc: &PdfDocument<'_>, i: usize) -> Result<String, PdfiumError> {
    let p = page_of(doc, i)?;
    let t = p.text().map_err(|_| PdfiumError::Damaged)?;
    Ok(t.chars().iter().filter_map(|c| c.unicode_char()).collect())
}

/// Text of the 0-based inclusive `pages` range, one page after another joined
/// by `\n`, in content-stream order.
///
/// # Errors
/// `InvalidInput` (bad path or empty range), `PageOutOfRange`, `Damaged`,
/// `EncryptedInput`, `Library`.
pub fn extract_text(path: &Path, pages: RangeInclusive<usize>) -> Result<String, PdfiumError> {
    if pages.start() > pages.end() {
        return Err(PdfiumError::InvalidInput("empty page range".into()));
    }
    with_doc(path, |_, doc| {
        page_of(doc, *pages.end())?;
        let out: Result<Vec<String>, _> = pages.map(|i| text_of(doc, i)).collect();
        Ok(out?.join("\n"))
    })
}

/// Text of every page, in page order, from one load of the document.
///
/// # Errors
/// As [`extract_text`].
pub(crate) fn page_texts(path: &Path) -> Result<Vec<String>, PdfiumError> {
    with_doc(path, |_, doc| {
        (0..count_of(doc)).map(|i| text_of(doc, i)).collect()
    })
}
