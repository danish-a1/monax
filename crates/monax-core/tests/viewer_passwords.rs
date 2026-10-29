//! The viewer's queries open a password-protected PDF through pdfium's
//! in-memory password: page count and thumbnails.
mod common;
use common::fixture;
use monax_core::Secret;
use monax_core::pdfium::{self, PdfiumError};

#[test]
fn viewer_queries_take_a_password() {
    require_pdfium!();
    let enc = fixture("encrypted.pdf");
    let (user, wrong) = (Secret::from("user"), Secret::from("x-Sesame"));
    assert_eq!(
        pdfium::page_count_with_password(&enc, Some(&user)).unwrap(),
        3
    );
    let r = pdfium::page_count_with_password(&enc, None);
    assert!(matches!(r, Err(PdfiumError::EncryptedInput)), "{r:?}");
    let r = pdfium::page_count_with_password(&enc, Some(&wrong));
    assert!(matches!(r, Err(PdfiumError::WrongPassword)), "{r:?}");
    assert_eq!(r.unwrap_err().to_string(), "wrong password");
    let png = pdfium::thumbnail_png_with_password(&enc, 1, 64, Some(&user)).unwrap();
    assert_eq!(&png[1..4], b"PNG");
    let r = pdfium::thumbnail_png_with_password(&enc, 1, 64, Some(&wrong));
    assert!(matches!(r, Err(PdfiumError::WrongPassword)), "{r:?}");
    let own = fixture("owner-only.pdf");
    assert_eq!(pdfium::page_count_with_password(&own, None).unwrap(), 3);
}
