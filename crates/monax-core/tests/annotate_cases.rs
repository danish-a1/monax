//! annotate, Review Focus cases: a rotated page with a cropped origin,
//! encrypted input through `AnnotateInput` (password), damaged input, a form document (fields kept, checked path),
//! unknown annotation types refused at the boundary.
mod common;
use common::annot::*;
use common::sign::near;
use common::{copy_fixture, diff_box, names};
use monax_core::annotate::{AnnotateInput, PageAnnot, save};
use monax_core::forms::list_fields;
use monax_core::pages::PagesError;
use serde_json::json;

fn args(input: &std::path::Path, password: Option<&str>) -> serde_json::Value {
    json!({ "input": input, "password": password, "annots": [
        { "page": 0, "annot": { "type": "highlight", "color": "#FFFF00",
          "rect": { "left": 10, "bottom": 10, "right": 50, "top": 30 } } } ] })
}

#[test]
fn encrypted_input_needs_its_password() {
    require_qpdf!();
    let (dir, enc) = copy_fixture("encrypted.pdf", "enc.pdf");
    let r = run_input!(AnnotateInput, args(&enc, None));
    assert!(matches!(r, Err(PagesError::EncryptedInput)), "{r:?}");
    let r = run_input!(AnnotateInput, args(&enc, Some("bad")));
    assert!(matches!(r, Err(PagesError::WrongPassword)), "{r:?}");
    let (out, notes) = run_input!(AnnotateInput, args(&enc, Some("user"))).unwrap();
    assert_eq!(out, dir.path().join("enc-annotated.pdf"));
    assert_eq!(notes, ["output is not password-protected"]);
    assert_eq!(dicts(&out)[0].len(), 1);
    assert_eq!(names(dir.path()), ["enc-annotated.pdf", "enc.pdf"]);
}

#[test]
fn owner_restricted_input_is_annotated_and_noted() {
    require_qpdf!();
    let (dir, own) = copy_fixture("owner-only.pdf", "own.pdf");
    let (out, notes) = run_input!(AnnotateInput, args(&own, None)).unwrap();
    assert_eq!(dicts(&out)[0].len(), 1);
    let want = format!("permissions of {} were not carried over", own.display());
    assert_eq!(notes, [want]);
    assert_eq!(names(dir.path()), ["own-annotated.pdf", "own.pdf"]);
}

#[test]
fn rotated_cropped_page_shows_the_annotation_where_its_user_rect_is() {
    require_qpdf!();
    // /Rotate 90, CropBox [50 20 250 120]: user rect (150 40 230 80) shows
    // at displayed x 20..60, y 100..180 (pixels, top-left origin).
    let (dir, rot) = copy_fixture("rotated-crop.pdf", "rot.pdf");
    let a = [on(0, highlight(rect(150.0, 40.0, 230.0, 80.0)))];
    let out = save(&rot, &dir.path().join("o.pdf"), &a).unwrap();
    near(diff_box(&rot, &out, 0).unwrap(), (20, 100, 59, 179));
}

#[test]
fn damaged_input_reports_damaged_file() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.pdf");
    std::fs::write(&bad, b"%PDF-1.7\n1 0 obj << /Type /Catalog").unwrap();
    let a = [on(0, highlight(rect(10.0, 10.0, 50.0, 30.0)))];
    let r = save(&bad, &dir.path().join("o.pdf"), &a);
    assert!(matches!(r, Err(PagesError::Damaged)), "{r:?}");
    assert_eq!(names(dir.path()), ["bad.pdf"]);
}

#[test]
fn input_qpdf_must_repair_is_refused() {
    require_qpdf!();
    let (dir, p) = copy_fixture("three-pages.pdf", "x.pdf");
    let bytes = std::fs::read(&p).unwrap();
    let at = bytes
        .windows(10)
        .rposition(|w| w == b"startxref\n")
        .unwrap()
        + 10;
    let mut broken = bytes[..at].to_vec();
    broken.extend(b"999999\n%%EOF\n");
    std::fs::write(&p, broken).unwrap();
    let a = [on(0, highlight(rect(10.0, 10.0, 50.0, 30.0)))];
    let r = save(&p, &dir.path().join("o.pdf"), &a);
    assert_eq!(r.unwrap_err().to_string(), "damaged file: repair it first");
    assert_eq!(names(dir.path()), ["x.pdf"]);
}

#[test]
fn form_fields_and_their_values_are_kept() {
    require_qpdf!();
    let (dir, form) = copy_fixture("form.pdf", "form.pdf");
    let a = [on(0, highlight(rect(200.0, 200.0, 290.0, 290.0)))];
    let out = save(&form, &dir.path().join("o.pdf"), &a).unwrap();
    assert_eq!(list_fields(&out).unwrap(), list_fields(&form).unwrap());
    let (before, after) = (dicts(&form), dicts(&out));
    assert_eq!(after[0].len(), before[0].len() + 1);
    // qpdf renumbers objects: compare the widgets without references.
    let plain = |v: &[serde_json::Value]| -> Vec<serde_json::Value> {
        let drop = |d: &serde_json::Value| {
            let mut d = d.as_object().unwrap().clone();
            d.retain(|k, _| !["/AP", "/P", "/Parent"].contains(&k.as_str()));
            serde_json::Value::Object(d)
        };
        v.iter().map(drop).collect()
    };
    assert_eq!(plain(&after[0][..before[0].len()]), plain(&before[0]));
}

#[test]
fn unknown_types_and_fields_are_refused_when_parsed() {
    let p = |v| {
        serde_json::from_value::<PageAnnot>(v)
            .unwrap_err()
            .to_string()
    };
    let rect = json!({ "left": 1, "bottom": 1, "right": 9, "top": 9 });
    let e =
        p(json!({ "page": 0, "annot": { "type": "squiggly", "rect": rect, "color": "#000000" } }));
    assert!(e.starts_with("unknown variant `squiggly`"), "{e}");
    let e = p(
        json!({ "page": 0, "annot": { "type": "highlight", "rect": rect, "color": "#000000", "opacity": 1 } }),
    );
    assert!(e.contains("unknown field `opacity`"), "{e}");
    let e = p(
        json!({ "page": 0, "annot": { "type": "shape", "kind": "star", "rect": rect,
        "color": "#000000", "width": 1 } }),
    );
    assert!(e.contains("unknown variant `star`"), "{e}");
}
