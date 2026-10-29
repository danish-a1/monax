//! Typed failures of pdfium-backed operations.
use crate::output::OutputError;
use crate::paths::PathError;

/// Failures of pdfium-backed operations.
#[derive(Debug, thiserror::Error)]
pub enum PdfiumError {
    #[error("pdfium library unavailable: {0}")]
    Library(String),
    #[error("damaged file")]
    Damaged,
    #[error("encrypted input: a password is required")]
    EncryptedInput,
    /// A password was given and the file rejected it (never echoed).
    #[error("wrong password")]
    WrongPassword,
    #[error("{}", crate::pdfium::out_of_range(*.page, *.count))]
    PageOutOfRange { page: usize, count: usize },
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// R113: the form's field tree reaches one field twice (a DAG or a
    /// cycle), which pdfium walks once per path when it draws the form.
    #[error("damaged file: form field tree reaches a field twice")]
    FieldRevisit,
    /// A field's /Parent chain is longer than pdfium nests fields (32),
    /// which pdfium joins into the full name of every field below it.
    #[error("damaged file: form field nesting is deeper than 32")]
    FieldDepth,
    /// qpdf warned reading a document with a form (e.g. it rebuilt the
    /// xref), so pdfium may walk a field tree the check did not see.
    #[error("damaged file: repair it first")]
    DamagedRepairFirst,
    /// The form field check before a render could not read the document;
    /// carries the reason.
    #[error("form field check failed: {0}")]
    FieldCheck(String),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<PathError> for PdfiumError {
    fn from(e: PathError) -> Self {
        Self::InvalidInput(e.to_string())
    }
}

impl From<OutputError> for PdfiumError {
    fn from(e: OutputError) -> Self {
        match e {
            OutputError::InvalidInput(s) => Self::InvalidInput(s),
            OutputError::Io(e) => Self::Io(e),
        }
    }
}

/// The `PageOutOfRange` message: `page` is a 0-based index.
pub(crate) fn out_of_range(page: usize, count: usize) -> String {
    match count.checked_sub(1) {
        Some(last) => {
            format!("page index {page} out of range (document has {count} pages, indices 0-{last})")
        }
        None => format!("page index {page} out of range (document has no pages)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn out_of_range_names_the_indices_or_says_no_pages() {
        let e = PdfiumError::PageOutOfRange { page: 5, count: 3 };
        assert_eq!(
            e.to_string(),
            "page index 5 out of range (document has 3 pages, indices 0-2)"
        );
        let e = PdfiumError::PageOutOfRange { page: 0, count: 0 };
        assert_eq!(
            e.to_string(),
            "page index 0 out of range (document has no pages)"
        );
    }
}
