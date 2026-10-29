//! Forms against the Review Focus cases: a rotated page with an offset
//! CropBox, XFA, encrypted, damaged, taken or clashing output paths.
mod common;
use common::docs::{get, objects, qpdf};
use common::forms::{catalog_with, derive, field, form, fv};
use monax_core::forms::{FillFormInput, ListFieldsInput, fill, fill_with_notes, list_fields};
use monax_core::pages::PagesError;
use serde_json::json;

#[test]
fn a_rotated_cropped_page_lists_raw_rects_and_shows_the_value() {
    require_qpdf!();
    let (d, p) = form();
    let objs = objects(&p);
    let mut page = get(&objs, &json!("3 0 R")).clone();
    page["/Rotate"] = json!(90);
    page["/CropBox"] = json!([10, 10, 290, 290]);
    let rot = derive(&p, "rot.pdf", json!({ "obj:3 0 R": { "value": page } }));
    let unrotated = list_fields(&p).unwrap();
    assert_eq!(list_fields(&rot).unwrap(), unrotated);
    let out = fill(&rot, &d.path().join("o.pdf"), &[fv("person.name", "Zed")]).unwrap();
    assert_eq!(field(&out, "person.name").value, "Zed");
    assert!(
        common::diff_box(&rot, &out, 0).is_some(),
        "no visible change"
    );
}

#[test]
fn an_xfa_form_keeps_its_standard_fields_and_says_so() {
    require_qpdf!();
    let (d, p) = form();
    let xfa = derive(
        &p,
        "xfa.pdf",
        catalog_with(&p, json!({ "/XFA": "u:<xdp/>" })),
    );
    let (out, notes) = fill_with_notes(&xfa, &d.path().join("o.pdf"), &[fv("email", "x")]).unwrap();
    assert_eq!(
        notes,
        ["XFA form data was removed; the form now uses its standard fields"]
    );
    let objs = objects(&out);
    let root = json!("1 0 R");
    let form = get(&objs, &get(&objs, &root)["/AcroForm"]);
    assert!(
        form.get("/XFA").is_none() && form["/Fields"].is_array(),
        "{form}"
    );
    let (_, plain) = fill_with_notes(&p, &d.path().join("p.pdf"), &[fv("email", "x")]).unwrap();
    assert!(plain.is_empty(), "{plain:?}");
}

#[test]
fn an_encrypted_form_needs_its_password() {
    require_qpdf!();
    let (d, p) = form();
    let enc = d.path().join("enc.pdf");
    qpdf(&[
        p.to_str().unwrap(),
        "--encrypt",
        "user",
        "owner",
        "256",
        "--",
        enc.to_str().unwrap(),
    ]);
    let out = d.path().join("o.pdf");
    let e = fill(&enc, &out, &[fv("email", "x")]).unwrap_err();
    assert!(matches!(e, PagesError::EncryptedInput), "{e:?}");
    assert!(matches!(list_fields(&enc), Err(PagesError::EncryptedInput)));
    let filled = serde_json::from_value::<FillFormInput>(json!({ "input": enc, "password": "user",
        "values": [{ "name": "email", "value": "x" }] }))
    .unwrap()
    .run()
    .unwrap();
    assert_eq!(filled.0, d.path().join("enc-filled.pdf"));
    assert_eq!(filled.1, ["output is not password-protected"]);
    assert_eq!(field(&filled.0, "email").value, "x");
    let wrong = json!({ "input": enc, "password": "nope" });
    let e = serde_json::from_value::<ListFieldsInput>(wrong)
        .unwrap()
        .run();
    assert!(matches!(e, Err(PagesError::WrongPassword)), "{e:?}");
    let ok = json!({ "input": enc, "password": "owner" });
    let (json, _) = serde_json::from_value::<ListFieldsInput>(ok)
        .unwrap()
        .run()
        .unwrap();
    assert_eq!(json, d.path().join("enc-fields.json"));
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&json).unwrap()).unwrap();
    assert_eq!(
        v["fields"][1],
        json!({ "name": "email", "kind": "text", "page": 0,
        "rect": { "left": 20.0, "bottom": 220.0, "right": 180.0, "top": 240.0 },
        "value": "", "read_only": false })
    );
    assert_eq!(v["fields"][2]["options"], json!(["Agreed"]));
}

#[test]
fn damaged_input_and_clashing_outputs_write_nothing() {
    require_qpdf!();
    let (d, p) = form();
    let bad = d.path().join("bad.pdf");
    std::fs::write(&bad, b"%PDF-1.7\ntruncated").unwrap();
    let out = d.path().join("o.pdf");
    assert!(matches!(list_fields(&bad), Err(PagesError::Damaged)));
    let e = fill(&bad, &out, &[fv("email", "x")]).unwrap_err();
    assert!(matches!(e, PagesError::Damaged), "{e:?}");
    // A wrong startxref: pdfium rebuilds the xref, qpdf warns.
    let mut raw = std::fs::read(&p).unwrap();
    let at = raw.windows(9).rposition(|w| w == b"startxref").unwrap() + 10;
    raw[at] = b'9';
    std::fs::write(&bad, raw).unwrap();
    let e = fill(&bad, &out, &[fv("email", "x")]).unwrap_err();
    assert_eq!(e.to_string(), "damaged file: repair it first");
    let e = fill(&p, &p, &[fv("email", "x")]).unwrap_err();
    assert!(
        e.to_string().contains("output would overwrite an input"),
        "{e}"
    );
    assert_eq!(common::names(d.path()), ["bad.pdf", "form.pdf"]);
    std::fs::write(&out, b"taken").unwrap();
    let got = fill(&p, &out, &[fv("email", "x")]).unwrap();
    assert_eq!(got, d.path().join("o-1.pdf"));
    assert_eq!(std::fs::read(&out).unwrap(), b"taken");
    let rel = fill(&p, std::path::Path::new("o.pdf"), &[fv("email", "x")]).unwrap_err();
    assert!(rel.to_string().contains("must be absolute"), "{rel}");
}
