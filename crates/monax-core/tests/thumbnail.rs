mod common;
use common::fixture;
use monax_core::pdfium::{self, PdfiumError};

fn dims(png: &[u8]) -> (u32, u32) {
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let r = |o: usize| u32::from_be_bytes(png[o..o + 4].try_into().unwrap());
    (r(16), r(20))
}

#[test]
fn longer_side_is_max_px() {
    require_pdfium!();
    let png = pdfium::thumbnail_png(&fixture("three-pages.pdf"), 0, 200).unwrap();
    let (w, h) = dims(&png);
    assert_eq!(w.max(h), 200);
}

#[test]
fn rotated_page_is_oriented_as_displayed() {
    require_pdfium!();
    let f = fixture("rotated-crop.pdf");
    let (dw, dh) = pdfium::displayed_size(&f, 0).unwrap();
    let (w, h) = dims(&pdfium::thumbnail_png(&f, 0, 300).unwrap());
    assert_eq!(w > h, dw > dh);
    assert_eq!(w.max(h), 300);
}

#[test]
fn max_px_out_of_range_is_invalid() {
    require_pdfium!();
    let f = fixture("three-pages.pdf");
    for bad in [0, 15, 1025] {
        let r = pdfium::thumbnail_png(&f, 0, bad);
        assert!(matches!(r, Err(PdfiumError::InvalidInput(_))), "{bad}");
    }
    assert!(pdfium::thumbnail_png(&f, 0, 16).is_ok());
    assert!(pdfium::thumbnail_png(&f, 0, 1024).is_ok());
}

#[test]
fn page_out_of_range() {
    require_pdfium!();
    let r = pdfium::thumbnail_png(&fixture("three-pages.pdf"), 3, 64);
    assert!(matches!(r, Err(PdfiumError::PageOutOfRange { .. })));
}
