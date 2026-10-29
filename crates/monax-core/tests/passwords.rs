//! Passwords for every op (R88): an input struct's `password` opens an
//! encrypted input for qpdf ops, pdfium ops, splits, reads and merges; the
//! guards still apply to the original; owner-only files need none.
mod common;
use common::docs::requires_password;
use common::{copy_fixture, fixture, names, words};
use monax_core::pages::PagesError;
use serde_json::{Value, json};
use std::path::PathBuf;

const NOTE: &str = "output is not password-protected";
const WRONG: &str = "o-Sesame wrong";

/// `encrypted.pdf` (user password "user") copied as `enc.pdf`.
fn enc() -> (tempfile::TempDir, PathBuf) {
    copy_fixture("encrypted.pdf", "enc.pdf")
}

#[test]
fn a_password_opens_the_input_of_every_kind_of_op() {
    require_qpdf!();
    use monax_core::pages::{NupInput, RotateInput, SplitPagesInput};
    let (dir, p) = enc();
    let d = dir.path();
    let pw = |extra: Value| {
        let mut v = json!({ "input": p, "password": "user" });
        v.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        v
    };
    let (out, notes) = run_input!(RotateInput, pw(json!({ "pages": [0], "degrees": 90 }))).unwrap();
    assert_eq!(
        (out.clone(), notes),
        (d.join("enc-rotate.pdf"), vec![NOTE.to_string()])
    );
    assert_eq!(requires_password(&out), Some(2));
    let (out, _) = run_input!(NupInput, pw(json!({ "per_sheet": 2 }))).unwrap();
    assert_eq!(
        (out.clone(), requires_password(&out)),
        (d.join("enc-nup.pdf"), Some(2))
    );
    let ranges = json!({ "ranges": [{ "start": 0, "end": 0 }, { "start": 1, "end": 2 }] });
    let (outs, notes) = run_input!(SplitPagesInput, pw(ranges)).unwrap();
    assert_eq!(outs, [d.join("enc-1.pdf"), d.join("enc-2.pdf")]);
    assert_eq!(notes, [NOTE]);
    assert_eq!(words(&outs[1]), ["two", "three"]);
    use monax_core::content::{CompressInput, WatermarkInput};
    let (out, notes) = run_input!(WatermarkInput, pw(json!({ "opts": { "text": "W" } }))).unwrap();
    assert_eq!(
        (out, notes),
        (d.join("enc-watermark.pdf"), vec![NOTE.to_string()])
    );
    let (_, notes) = run_input!(CompressInput, pw(json!({}))).unwrap();
    assert_eq!(notes, [NOTE]);
    use monax_core::read::ReadTextInput;
    let (out, notes) = run_input!(ReadTextInput, pw(json!({ "pages": [1] }))).unwrap();
    assert_eq!(
        (
            std::fs::read_to_string(&out).unwrap().ends_with("two"),
            notes
        ),
        (true, vec![NOTE.to_string()])
    );
    let mut want = vec!["enc-1.pdf", "enc-2.pdf", "enc-compress.pdf", "enc-nup.pdf"];
    want.extend([
        "enc-read-text.txt",
        "enc-rotate.pdf",
        "enc-watermark.pdf",
        "enc.pdf",
    ]);
    assert_eq!(names(d), want);
}

#[test]
fn extract_images_opens_an_encrypted_input() {
    require_qpdf!();
    use monax_core::pdfium::ExtractImagesInput;
    use monax_core::security::{EncryptOpts, encrypt};
    let (dir, img) = copy_fixture("image.pdf", "src.pdf");
    let o = EncryptOpts {
        user_pw: "user".into(),
        owner_pw: "owner".into(),
        permissions: Default::default(),
        aes256: true,
    };
    let locked = encrypt(&img, &dir.path().join("img.pdf"), o).unwrap();
    let (outs, notes) = run_input!(
        ExtractImagesInput,
        json!({ "input": locked, "password": "user" })
    )
    .unwrap();
    assert_eq!(
        (outs, notes),
        (
            vec![dir.path().join("img-p1-1.png")],
            vec![NOTE.to_string()]
        )
    );
}

#[test]
fn a_wrong_or_missing_password_fails_with_its_own_error_and_no_output() {
    require_qpdf!();
    use monax_core::pages::{MergeInput, NupInput, RotateInput};
    use monax_core::read::RenderPageInput;
    let (dir, p) = enc();
    let three = dir.path().join("three.pdf");
    std::fs::copy(fixture("three-pages.pdf"), &three).unwrap();
    let r = run_input!(
        RotateInput,
        json!({ "input": p, "pages": [0], "degrees": 90, "password": WRONG })
    );
    assert_eq!(r.unwrap_err().to_string(), "wrong password");
    let r = run_input!(
        NupInput,
        json!({ "input": p, "per_sheet": 2, "password": WRONG })
    );
    assert!(matches!(r, Err(PagesError::WrongPassword)), "{r:?}");
    let r = run_input!(
        RenderPageInput,
        json!({ "input": p, "page": 0, "password": WRONG })
    );
    assert!(matches!(r, Err(PagesError::WrongPassword)), "{r:?}");
    let r = run_input!(NupInput, json!({ "input": p, "per_sheet": 2 }));
    assert!(matches!(r, Err(PagesError::EncryptedInput)), "{r:?}");
    let merge =
        |pw: Value| run_input!(MergeInput, json!({ "inputs": [three, p], "passwords": pw }));
    assert!(matches!(
        merge(json!([null, WRONG])),
        Err(PagesError::WrongPassword)
    ));
    assert!(matches!(
        merge(json!(["x"])),
        Err(PagesError::EncryptedInput)
    ));
    let e = merge(json!([null, "user", "x"])).unwrap_err().to_string();
    assert_eq!(e, "invalid input: more passwords than inputs");
    let e = run_input!(
        RotateInput,
        json!({ "input": p, "pages": [0], "degrees": 90, "password": "a\nb" })
    );
    assert_eq!(
        e.unwrap_err().to_string(),
        "invalid input: password must not contain line breaks or NUL"
    );
    assert_eq!(names(dir.path()), ["enc.pdf", "three.pdf"]);
}
