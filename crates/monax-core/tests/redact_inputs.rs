//! RedactInput from client JSON: default output, the not-searchable note,
//! encrypted input opened with its password (output not encrypted), and
//! the schema of the boxes.
mod common;
use common::docs::requires_password;
use common::{copy_fixture, names};
use monax_core::pages::PagesError;
use monax_core::pdfium;
use monax_core::security::RedactInput;
use serde_json::{Value, json};

fn run(v: Value) -> Result<(std::path::PathBuf, Vec<String>), PagesError> {
    serde_json::from_value::<RedactInput>(v).unwrap().run()
}

const SHARED: &str = "form field values and objects shared with kept pages are not redacted";

fn rect() -> Value {
    json!({ "left": 10, "bottom": 10, "right": 90, "top": 90 })
}

#[test]
fn redacts_beside_the_input_and_notes_lost_text() {
    require_qpdf!();
    let (dir, p) = copy_fixture("three-pages.pdf", "three.pdf");
    let boxes = json!([{ "page": 2, "rect": rect() }, { "page": 0, "rect": rect() },
        { "page": 2, "rect": { "left": 100, "bottom": 100, "right": 150, "top": 150 } }]);
    let (out, notes) = run(json!({ "input": p, "boxes": boxes })).unwrap();
    assert_eq!(out, dir.path().join("three-redact.pdf"));
    assert_eq!(
        notes,
        ["text on 2 redacted page(s) is no longer searchable", SHARED]
    );
    assert_eq!(pdfium::extract_text(&out, 1..=1).unwrap(), "Page two");
}

#[test]
fn encrypted_input_opens_with_its_password_and_output_is_plain() {
    require_qpdf!();
    let (dir, enc) = copy_fixture("encrypted.pdf", "enc.pdf");
    let boxes = json!([{ "page": 0, "rect": rect() }]);
    let (out, notes) = run(json!({ "input": enc, "boxes": boxes, "password": "user" })).unwrap();
    assert_eq!(
        notes,
        [
            "text on 1 redacted page(s) is no longer searchable",
            SHARED,
            "output is not password-protected"
        ]
    );
    assert_eq!(requires_password(&out), Some(2), "output is not encrypted");
    assert_eq!(pdfium::extract_text(&out, 0..=0).unwrap(), "");
    let e = run(json!({ "input": enc, "boxes": boxes, "password": "w-Sesame" }));
    assert!(matches!(e, Err(PagesError::WrongPassword)), "{e:?}");
    let e = run(json!({ "input": enc, "boxes": boxes }));
    assert!(matches!(e, Err(PagesError::EncryptedInput)), "{e:?}");
    assert_eq!(names(dir.path()), ["enc-redact.pdf", "enc.pdf"]);
}

#[test]
fn boxes_are_page_and_user_space_rect() {
    let s = serde_json::to_value(schemars::schema_for!(RedactInput)).unwrap();
    assert_eq!(s["required"], json!(["input", "boxes"]));
    assert_eq!(s["properties"]["password"]["writeOnly"], true);
    let e = serde_json::from_value::<RedactInput>(
        json!({ "input": "/a.pdf", "boxes": [{ "page": 0, "rect": rect(), "x": 1 }] }),
    );
    assert!(e.unwrap_err().to_string().starts_with("unknown field `x`"));
}
