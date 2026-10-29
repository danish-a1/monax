//! Blank single-page documents.
use super::{PdfiumError, lib, save, with_engine};
use pdfium_render::prelude::{PdfPagePaperSize, PdfPoints};

/// Bytes of a new one-page PDF whose empty page is `width` x `height` points
/// (MediaBox at the origin, no /Rotate).
///
/// # Errors
/// `Library` when pdfium is unavailable or fails to build or save the document.
pub(crate) fn blank_pdf(width: f32, height: f32) -> Result<Vec<u8>, PdfiumError> {
    with_engine(|engine| {
        let mut doc = engine.create_new_pdf().map_err(lib)?;
        let size = PdfPagePaperSize::from_points(PdfPoints::new(width), PdfPoints::new(height));
        doc.pages_mut().create_page_at_end(size).map_err(lib)?;
        save(&doc)
    })
}
