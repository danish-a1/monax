//! Signature placement, Review Focus cases: encrypted and damaged input,
//! output collisions with the input kept, forms kept.
mod common;
use common::sign::{RED, at, png};
use common::{copy_fixture, names, setup};
use monax_core::forms::{SignImageInput, SignTextInput, list_fields, sign_text};
use monax_core::pages::PagesError;
use serde_json::json;

const RECT: (f32, f32, f32, f32) = (60.0, 20.0, 160.0, 60.0);

#[test]
fn encrypted_input_needs_its_password() {
    require_qpdf!();
    let (dir, enc) = copy_fixture("encrypted.pdf", "enc.pdf");
    let img = png(dir.path(), "s.png", (10, 10), |_, _| RED);
    let v = |pw: Option<&str>| {
        json!({ "input": enc, "image": img, "password": pw,
        "placement": { "page": 0, "rect": { "left": 10, "bottom": 10, "right": 50, "top": 30 } } })
    };
    let r = run_input!(SignImageInput, v(None));
    assert!(matches!(r, Err(PagesError::EncryptedInput)), "{r:?}");
    let r = run_input!(SignImageInput, v(Some("bad")));
    assert!(matches!(r, Err(PagesError::WrongPassword)), "{r:?}");
    let (out, notes) = run_input!(SignImageInput, v(Some("user"))).unwrap();
    assert!(out.ends_with("enc-signed.pdf"), "{out:?}");
    assert!(
        notes.iter().any(|n| n == monax_core::forms::SIGN_NOTE),
        "{notes:?}"
    );
    assert!(
        notes
            .iter()
            .any(|n| n == "output is not password-protected"),
        "{notes:?}"
    );
}

#[test]
fn damaged_input_reports_damaged_file() {
    require_pdfium!();
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.pdf");
    std::fs::write(&bad, b"%PDF-1.7\n1 0 obj << /Type /Catalog").unwrap();
    let r = sign_text(&bad, &dir.path().join("o.pdf"), &at(0, RECT), "A", 10.0);
    assert!(matches!(r, Err(PagesError::Damaged)), "{r:?}");
    assert_eq!(names(dir.path()), ["bad.pdf"]);
}

#[test]
fn outputs_never_overwrite_and_the_input_is_kept() {
    require_pdfium!();
    let (_dir, three) = setup();
    let before = std::fs::read(&three).unwrap();
    let v = json!({ "input": three, "text": "Ann", "font_size": 12,
        "placement": { "page": 0, "rect": { "left": 60, "bottom": 20, "right": 160, "top": 60 } } });
    let (a, notes) = run_input!(SignTextInput, v.clone()).unwrap();
    assert_eq!(notes, [monax_core::forms::SIGN_NOTE]);
    let (b, _) = run_input!(SignTextInput, v).unwrap();
    assert_eq!(
        (a.file_name().unwrap(), b.file_name().unwrap()),
        ("three-signed.pdf".as_ref(), "three-signed-1.pdf".as_ref())
    );
    assert_eq!(std::fs::read(&three).unwrap(), before);
    let r = sign_text(&three, &three, &at(0, RECT), "A", 10.0);
    assert!(matches!(r, Err(PagesError::InvalidInput(_))), "{r:?}");
}

#[test]
fn form_fields_survive_signing() {
    require_qpdf!();
    let (dir, form) = copy_fixture("form.pdf", "form.pdf");
    let out = sign_text(
        &form,
        &dir.path().join("o.pdf"),
        &at(0, (200.0, 10.0, 290.0, 40.0)),
        "Ann",
        12.0,
    )
    .unwrap();
    assert_eq!(list_fields(&out).unwrap(), list_fields(&form).unwrap());
}
