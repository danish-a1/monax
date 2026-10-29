mod common;
use common::fixture;
use monax_core::pdfium::{self, PdfiumError};
use std::path::Path;

#[test]
fn page_out_of_range() {
    require_pdfium!();
    let p = fixture("three-pages.pdf");
    assert!(matches!(
        pdfium::render(&p, 3, 72),
        Err(PdfiumError::PageOutOfRange { .. })
    ));
    assert!(matches!(
        pdfium::page_size(&p, 3),
        Err(PdfiumError::PageOutOfRange { .. })
    ));
    assert!(matches!(
        pdfium::rotation(&p, 9),
        Err(PdfiumError::PageOutOfRange { .. })
    ));
    assert!(matches!(
        pdfium::extract_text(&p, 1..=3),
        Err(PdfiumError::PageOutOfRange { .. })
    ));
    #[allow(clippy::reversed_empty_ranges)]
    let backwards = 2..=1;
    assert!(matches!(
        pdfium::extract_text(&p, backwards),
        Err(PdfiumError::InvalidInput(_))
    ));
}

#[test]
fn damaged_file() {
    require_pdfium!();
    let dir = tempfile::tempdir().unwrap();
    let bytes = std::fs::read(fixture("three-pages.pdf")).unwrap();
    let cut = dir.path().join("cut.pdf");
    std::fs::write(&cut, &bytes[..40]).unwrap();
    assert!(matches!(
        pdfium::page_count(&cut),
        Err(PdfiumError::Damaged)
    ));
    let junk = dir.path().join("junk.pdf");
    std::fs::write(&junk, b"not a pdf at all").unwrap();
    assert!(matches!(
        pdfium::render(&junk, 0, 72),
        Err(PdfiumError::Damaged)
    ));
}

#[test]
fn encrypted_input() {
    require_pdfium!();
    let p = fixture("encrypted.pdf");
    assert!(matches!(
        pdfium::page_count(&p),
        Err(PdfiumError::EncryptedInput)
    ));
}

#[test]
fn invalid_dpi() {
    require_pdfium!();
    let p = fixture("three-pages.pdf");
    for dpi in [0, 1201, u32::MAX] {
        assert!(matches!(
            pdfium::render(&p, 0, dpi),
            Err(PdfiumError::InvalidInput(_))
        ));
    }
    assert!(pdfium::render(&p, 0, 1).is_ok());
}

#[test]
fn pixel_budget_enforced() {
    require_pdfium!();
    // 14400pt (200in) square at 1200 dpi is 240000 px per side: far over budget.
    let dir = tempfile::tempdir().unwrap();
    let big = dir.path().join("big.pdf");
    let bytes = std::fs::read(fixture("three-pages.pdf")).unwrap();
    let s = String::from_utf8_lossy(&bytes).replace("[ 0 0 200 200 ]", "[ 0 0 14400 14400 ]");
    std::fs::write(&big, s.as_bytes()).unwrap();
    assert!(matches!(
        pdfium::render(&big, 0, 1200),
        Err(PdfiumError::InvalidInput(_))
    ));
}

#[test]
fn bad_paths() {
    require_pdfium!();
    assert!(matches!(
        pdfium::page_count(Path::new("relative.pdf")),
        Err(PdfiumError::InvalidInput(_))
    ));
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        pdfium::page_count(&dir.path().join("nope.pdf")),
        Err(PdfiumError::InvalidInput(_))
    ));
    assert!(matches!(
        pdfium::page_count(dir.path()),
        Err(PdfiumError::InvalidInput(_))
    ));
}
