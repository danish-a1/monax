//! PDF to images: pages render as displayed (rotation + CropBox) to PNG or
//! JPEG files, one per page, with the Review Focus cases (encrypted,
//! damaged, form check, taken names, bad options) and all-or-nothing output.
mod common;
use common::convert::{decode, invalid, to_images};
use common::ocr::job;
use common::{copy_fixture, names, setup};
use monax_core::convert::{ImageFormat, PdfToImagesInput, pdf_to_images};
use monax_core::pages::PagesError;
use monax_core::pdfium::render;
use serde_json::{Value, json};
use std::path::PathBuf;

/// [`PdfToImagesInput::run`] of `v` on a registry job.
fn run_input(v: Value) -> Result<(Vec<PathBuf>, Vec<String>), PagesError> {
    let input: PdfToImagesInput = serde_json::from_value(v).unwrap();
    job(move |s| input.run(s)).0
}

#[test]
fn every_page_becomes_a_png_as_displayed_at_the_dpi() {
    require_pdfium!();
    let (dir, three) = setup();
    let got = to_images(&three, 150, ImageFormat::Png, None).unwrap();
    let want: Vec<PathBuf> = (1..=3)
        .map(|n| dir.path().join(format!("three-{n}.png")))
        .collect();
    assert_eq!(got, want);
    for (i, p) in got.iter().enumerate() {
        let r = render(&three, i, 150).unwrap();
        assert_eq!(decode(p), (417, 417, r.rgba), "page {i}");
    }
    // /Rotate 90 and CropBox [50 20 250 120]: 100 x 200 points as shown.
    let (dir, rc) = copy_fixture("rotated-crop.pdf", "rc.pdf");
    let got = to_images(&rc, 150, ImageFormat::Png, None).unwrap();
    assert_eq!(got, [dir.path().join("rc-1.png")]);
    let r = render(&rc, 0, 150).unwrap();
    assert_eq!(decode(&got[0]), (209, 417, r.rgba));
}

#[test]
fn jpeg_of_chosen_pages_in_order_and_taken_names_get_a_suffix() {
    require_pdfium!();
    let (dir, three) = setup();
    std::fs::write(dir.path().join("three-1.jpg"), b"keep").unwrap();
    let got = to_images(&three, 72, ImageFormat::Jpeg, Some(&[2, 0])).unwrap();
    let want = ["three-3.jpg", "three-1-1.jpg"].map(|n| dir.path().join(n));
    assert_eq!(got, want);
    for p in &got {
        let b = std::fs::read(p).unwrap();
        assert!(b.starts_with(&[0xFF, 0xD8, 0xFF]), "{}", p.display());
        let sof = b.windows(2).position(|w| w == [0xFF, 0xC0]).unwrap();
        assert_eq!(b[sof + 5..sof + 9], [0, 200, 0, 200], "200 x 200 px");
    }
    assert_eq!(
        std::fs::read(dir.path().join("three-1.jpg")).unwrap(),
        b"keep"
    );
}

#[test]
fn input_struct_defaults_and_password() {
    require_qpdf!();
    let (dir, enc) = copy_fixture("encrypted.pdf", "enc.pdf");
    let r = to_images(&enc, 72, ImageFormat::Png, None);
    assert!(matches!(r, Err(PagesError::EncryptedInput)), "{r:?}");
    let (got, notes) = run_input(json!({ "input": enc, "password": "user" })).unwrap();
    assert_eq!(notes, ["output is not password-protected"]);
    assert_eq!(got[0], dir.path().join("enc-1.png"));
    let r = run_input(json!({ "input": enc, "password": "nope" }));
    assert!(matches!(r, Err(PagesError::WrongPassword)), "{r:?}");
    // Defaults: 150 dpi PNG of every page into the input's directory.
    let (dir, three) = setup();
    let out = tempfile::tempdir().unwrap();
    let v = json!({ "input": three, "out_dir": out.path(), "format": "jpeg", "pages": [1] });
    let (got, notes) = run_input(v).unwrap();
    assert!(notes.is_empty());
    assert_eq!(got, [out.path().join("three-2.jpg")]);
    let (got, _) = run_input(json!({ "input": three })).unwrap();
    assert_eq!(got.len(), 3);
    assert_eq!(decode(&got[0]).0, 417);
    let v = json!({ "input": three, "out_dir": "rel" });
    let m = run_input(v).unwrap_err().to_string();
    assert!(
        m.contains("must be an absolute path to an existing directory"),
        "{m}"
    );
    assert_eq!(names(dir.path()).len(), 4);
}

#[test]
fn bad_options_and_inputs_fail_without_output() {
    require_pdfium!();
    let (dir, three) = setup();
    let png = ImageFormat::Png;
    assert_eq!(
        invalid(to_images(&three, 71, png, None)),
        "dpi 71 not in 72..=600"
    );
    assert_eq!(
        invalid(to_images(&three, 601, png, None)),
        "dpi 601 not in 72..=600"
    );
    assert_eq!(
        invalid(to_images(&three, 72, png, Some(&[]))),
        "no pages given"
    );
    let r = to_images(&three, 72, png, Some(&[0, 3]));
    assert!(
        matches!(r, Err(PagesError::PageOutOfRange { page: 3, count: 3 })),
        "{r:?}"
    );
    for q in [0, 101] {
        let (i, d) = (three.clone(), dir.path().to_path_buf());
        let r = job(move |s| pdf_to_images(&i, &d, 72, ImageFormat::Jpeg, q, None, s)).0;
        assert_eq!(invalid(r), format!("jpeg_quality {q} not in 1..=100"));
    }
    let cut = dir.path().join("cut.pdf");
    std::fs::write(&cut, &std::fs::read(&three).unwrap()[..40]).unwrap();
    let r = to_images(&cut, 72, png, None);
    assert!(matches!(r, Err(PagesError::Damaged)), "{r:?}");
    assert_eq!(names(dir.path()), ["cut.pdf", "three.pdf"]);
}

#[test]
fn forms_render_through_the_field_check() {
    require_qpdf!();
    let (dir, form) = copy_fixture("form.pdf", "form.pdf");
    let got = to_images(&form, 72, ImageFormat::Png, None).unwrap();
    assert_eq!(decode(&got[0]).2, render(&form, 0, 72).unwrap().rgba);
    let (dir2, dag) = copy_fixture("field-dag.pdf", "dag.pdf");
    let e = to_images(&dag, 72, ImageFormat::Png, None).unwrap_err();
    assert_eq!(
        e.to_string(),
        "damaged file: form field tree reaches a field twice"
    );
    assert_eq!(names(dir.path()), ["form-1.png", "form.pdf"]);
    assert_eq!(names(dir2.path()), ["dag.pdf"]);
}
