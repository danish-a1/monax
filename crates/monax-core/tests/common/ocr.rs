//! OCR test helpers: tesseract presence and running an op as a registry job.
use monax_core::{Job, ProgressSink, Registry};

/// The note every OCR output gets (R125).
pub const NOTE: &str = "pages were replaced by images with a searchable text layer; \
    links, form fields, annotations and bookmarks were not carried over";

/// True when tesseract is found (strict: `MONAX_REQUIRE_SIDECARS`).
pub fn have_tesseract() -> bool {
    let ok = monax_sidecar::find(monax_sidecar::Tool::Tesseract).is_some();
    super::available(ok, "tesseract not found", "MONAX_REQUIRE_SIDECARS")
}

/// Runs `f` with a job's progress sink on a fresh registry; returns what
/// `f` returned and the finished job (its notes).
pub fn job<T: Send + 'static>(f: impl FnOnce(&ProgressSink) -> T + Send + 'static) -> (T, Job) {
    let reg = Registry::new();
    let (tx, rx) = std::sync::mpsc::channel();
    let id = reg.submit(move |s| {
        tx.send(f(&s)).unwrap();
        Ok::<_, String>(vec![])
    });
    let job = reg.wait(id).unwrap();
    (rx.recv().unwrap(), job)
}

/// Early-returns from the test (with a notice) when pdfium, qpdf or
/// tesseract is missing.
#[macro_export]
macro_rules! require_tesseract {
    () => {
        if !common::have_pdfium() || !common::have_qpdf() || !common::ocr::have_tesseract() {
            return;
        }
    };
}
