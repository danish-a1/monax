//! Failing or missing qpdf: images to PDF reports the exact reason and
//! leaves no file; PDF to images needs no qpdf for a document without a form.
mod common;
use common::convert::idle;
use common::ocr::job;
use common::sign::{RED, png};
use common::{fixture, names};
use monax_core::convert::{ImageFormat, images_to_pdf, pdf_to_images};
use monax_core::pages::PagesError;
use monax_sidecar::SidecarError;

// Every scenario mutates process env, so they share one test to avoid races.
#[test]
fn failing_or_missing_qpdf_leaves_no_output() {
    require_pdfium!();
    let dir = tempfile::tempdir().unwrap();
    let img = png(dir.path(), "a.png", (4, 4), |_, _| RED);
    let out = dir.path().join("o.pdf");
    let only_input = || assert_eq!(names(dir.path()), ["a.png"]);

    // SAFETY: the only test in this binary.
    unsafe { std::env::set_var("MONAX_BIN_DIR", fixture("bin-partial")) };
    let r = images_to_pdf(std::slice::from_ref(&img), &out, None, &idle());
    assert!(
        matches!(&r, Err(PagesError::Qpdf(m)) if m == "qpdf: boom"),
        "{r:?}"
    );
    only_input();

    // SAFETY: as above.
    unsafe {
        std::env::set_var("MONAX_BIN_DIR", "/nonexistent");
        std::env::remove_var("PATH");
    }
    let r = images_to_pdf(std::slice::from_ref(&img), &out, None, &idle());
    let missing = matches!(r, Err(PagesError::Sidecar(SidecarError::NotFound("qpdf"))));
    assert!(missing, "{r:?}");
    only_input();
    let pdf = dir.path().join("three.pdf");
    std::fs::copy(fixture("three-pages.pdf"), &pdf).unwrap();
    let d = dir.path().to_path_buf();
    let r = job(move |s| pdf_to_images(&pdf, &d, 72, ImageFormat::Png, 85, Some(&[0]), s)).0;
    assert_eq!(r.unwrap(), [dir.path().join("three-1.png")]);
}
