//! The viewer's queries: page geometry and page images at device-pixel size.
mod common;
use common::fixture;
use monax_core::Secret;
use monax_core::pdfium::{self, PdfiumError};

fn dims(png: &[u8]) -> (u32, u32) {
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let r = |o: usize| u32::from_be_bytes(png[o..o + 4].try_into().unwrap());
    (r(16), r(20))
}

#[test]
fn page_info_matches_the_geometry_queries() {
    require_pdfium!();
    let f = fixture("rotated-crop.pdf");
    let info = pdfium::page_info(&f, None).unwrap();
    let n = pdfium::page_count(&f).unwrap();
    assert_eq!((info.page_count, info.pages.len()), (n, n));
    for (i, pg) in info.pages.iter().enumerate() {
        let (l, b, r, t) = pdfium::crop_box(&f, i).unwrap();
        assert_eq!(pg.crop_box, [l, b, r, t], "page {i}");
        assert_eq!(pg.rotation, pdfium::rotation(&f, i).unwrap(), "page {i}");
        let [ml, mb, mr, mt] = pg.media_box;
        assert_eq!((mr - ml, mt - mb), pdfium::page_size(&f, i).unwrap());
    }
}

#[test]
fn page_info_opens_with_a_password() {
    require_pdfium!();
    let f = fixture("encrypted.pdf");
    let pw = |s: &str| Secret::from(s);
    assert_eq!(
        pdfium::page_info(&f, Some(&pw("user")))
            .unwrap()
            .pages
            .len(),
        3
    );
    let r = pdfium::page_info(&f, None);
    assert!(matches!(r, Err(PdfiumError::EncryptedInput)), "{r:?}");
    let r = pdfium::page_info(&f, Some(&pw("nope")));
    assert!(matches!(r, Err(PdfiumError::WrongPassword)), "{r:?}");
}

#[test]
fn view_png_renders_up_to_view_max_px() {
    require_pdfium!();
    let f = fixture("three-pages.pdf");
    let png = pdfium::view_png(&f, 0, 2048, None).unwrap();
    assert_eq!(dims(&png).0.max(dims(&png).1), 2048);
    for bad in [15, pdfium::VIEW_MAX_PX + 1] {
        let r = pdfium::view_png(&f, 0, bad, None);
        assert!(matches!(r, Err(PdfiumError::InvalidInput(_))), "{bad}");
    }
}
