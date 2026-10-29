//! Typed failures of page operations (and the ops built on them).
use crate::output::OutputError;
use crate::paths::PathError;
use crate::pdfium::PdfiumError;
use monax_sidecar::SidecarError;

/// Failures of page operations.
#[derive(Debug, thiserror::Error)]
pub enum PagesError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("encrypted input: a password is required")]
    EncryptedInput,
    /// A password was given and the file rejected it (never echoed).
    #[error("wrong password")]
    WrongPassword,
    /// Encrypted, but too damaged for qpdf to find the encryption (its
    /// trailer is gone): never processed as plain.
    #[error("damaged encrypted file: repair needs the trailer to decrypt")]
    DamagedEncrypted,
    #[error("damaged file")]
    Damaged,
    /// The input's qpdf JSON nests past the reader's limit (128).
    #[error("damaged file: object nesting too deep to redact safely")]
    DamagedNesting,
    /// R112: qpdf warned or failed reading a redaction's input.
    #[error("damaged file: repair it first, then redact")]
    DamagedRepairFirst,
    /// The output of a redaction still held text of a redacted page (never
    /// echoed); nothing was written.
    #[error(
        "redaction check failed: text of a redacted page is still in the output; nothing was written"
    )]
    RedactionLeak,
    /// R115: a kept page of a redaction's output would not draw exactly as
    /// in the input; nothing was written.
    #[error("redaction check failed: a kept page would change")]
    KeptPageChanged,
    /// R131: pdfium drew no appearance for a filled field's value, so it
    /// would not show; nothing was written.
    #[error("form field {0:?} could not be drawn with its new value; nothing was written")]
    FieldNotDrawn(String),
    /// No `<code>.traineddata` for this OCR language in tesseract's data.
    #[error("OCR language not installed: {0}")]
    MissingLanguage(String),
    /// tesseract ran but failed; carries its last stderr line.
    #[error("tesseract failed: {0}")]
    Tesseract(String),
    /// R152: the bundled `wordcraft-cli` is missing; nothing was run.
    #[error("WordCraft not found - the bundled converter is missing")]
    OfficeUnavailable,
    /// WordCraft ran but failed; carries its last stderr line (at most 200
    /// chars).
    #[error("WordCraft failed: {0}")]
    Office(String),
    /// WordCraft exited 0 without writing the output.
    #[error("WordCraft produced no output")]
    OfficeNoOutput,
    /// The job was cancelled; files it had written are removed.
    #[error("cancelled")]
    Cancelled,
    #[error("{}", crate::pdfium::out_of_range(*.page, *.count))]
    PageOutOfRange { page: usize, count: usize },
    /// qpdf ran but failed (any exit but 0 or 3); carries its last stderr line.
    #[error("qpdf failed: {0}")]
    Qpdf(String),
    #[error(transparent)]
    Sidecar(#[from] SidecarError),
    #[error(transparent)]
    Pdfium(PdfiumError),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<PathError> for PagesError {
    fn from(e: PathError) -> Self {
        Self::InvalidInput(e.to_string())
    }
}

impl From<OutputError> for PagesError {
    fn from(e: OutputError) -> Self {
        match e {
            OutputError::InvalidInput(s) => Self::InvalidInput(s),
            OutputError::Io(e) => Self::Io(e),
        }
    }
}

impl From<PdfiumError> for PagesError {
    fn from(e: PdfiumError) -> Self {
        match e {
            PdfiumError::Damaged => Self::Damaged,
            PdfiumError::EncryptedInput => Self::EncryptedInput,
            PdfiumError::WrongPassword => Self::WrongPassword,
            PdfiumError::PageOutOfRange { page, count } => Self::PageOutOfRange { page, count },
            PdfiumError::InvalidInput(s) => Self::InvalidInput(s),
            e => Self::Pdfium(e),
        }
    }
}
