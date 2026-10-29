//! Small helpers shared by the pdfium page edits.
use super::PdfiumError;
use pdfium_render::prelude::{PdfDocument, PdfiumError as Lib};

pub(crate) fn lib(e: Lib) -> PdfiumError {
    PdfiumError::Library(e.to_string())
}

/// Serialises `doc` to PDF bytes.
pub(crate) fn save(doc: &PdfDocument<'_>) -> Result<Vec<u8>, PdfiumError> {
    doc.save_to_bytes().map_err(lib)
}

/// 0-based page index as pdfium's `i32`; `InvalidInput` when it does not fit.
pub(crate) fn idx(i: usize) -> Result<i32, PdfiumError> {
    i32::try_from(i).map_err(|_| PdfiumError::InvalidInput("too many pages".into()))
}
