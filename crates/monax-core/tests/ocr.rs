//! OCR: an image-only scan becomes searchable at its displayed size, with
//! the Review Focus cases (rotated + cropped page, encrypted, damaged,
//! taken output, form check, missing language, bad options).
mod common;
use common::copy_fixture;
use common::ocr::{NOTE, job};
use common::probe14::probe14_pdf;
use monax_core::ocr::{self, OcrInput};
use monax_core::pages::{PagesError, insert_blank};
use monax_core::pdfium::{displayed_size, extract_text, page_count};
use serde_json::json;
use std::path::{Path, PathBuf};

fn langs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// `ocr::run` on a registry job; its result and the job's notes.
fn ocr_run(
    input: &Path,
    out: &Path,
    lang: &[&str],
    dpi: u32,
) -> (Result<PathBuf, PagesError>, Vec<String>) {
    let (i, o, l) = (input.to_path_buf(), out.to_path_buf(), langs(lang));
    let (r, j) = job(move |s| ocr::run(&i, &o, &l, dpi, s));
    (r, j.notes)
}

/// [`OcrInput::run`] of `v` on a registry job.
fn ocr_input(v: serde_json::Value) -> Result<(PathBuf, Vec<String>), PagesError> {
    let input: OcrInput = serde_json::from_value(v).unwrap();
    job(move |s| input.run(s)).0
}

fn text(p: &Path, page: usize) -> String {
    extract_text(p, page..=page).unwrap()
}

#[test]
fn a_scan_becomes_searchable_at_its_displayed_size() {
    require_tesseract!();
    // The second: /Rotate 90, CropBox offset (10, 10), text upright as shown.
    for (name, shown) in [
        ("invoice-scan.pdf", (400.0, 150.0)),
        ("invoice-rotated-crop.pdf", (400.0, 150.0)),
    ] {
        let (dir, input) = copy_fixture(name, name);
        assert_eq!(text(&input, 0).trim(), "", "{name}: image only");
        assert_eq!(displayed_size(&input, 0).unwrap(), shown, "{name}");
        let out = dir.path().join("o.pdf");
        let (r, notes) = ocr_run(&input, &out, &["eng"], 300);
        assert_eq!(r.unwrap(), out);
        let t = text(&out, 0);
        assert!(t.contains("INVOICE") && t.contains("4471"), "{name}: {t:?}");
        let (w, h) = displayed_size(&out, 0).unwrap();
        assert!(
            (w - shown.0).abs() <= 1.0 && (h - shown.1).abs() <= 1.0,
            "{name}: {w}x{h}"
        );
        assert_eq!(notes, [NOTE]);
        assert_eq!(text(&input, 0).trim(), "", "input untouched");
    }
}

#[test]
fn defaults_name_the_output_and_a_page_without_text_is_no_error() {
    require_tesseract!();
    let (dir, scan) = copy_fixture("invoice-scan.pdf", "scan.pdf");
    let two = insert_blank(&scan, &[0], &dir.path().join("two.pdf")).unwrap();
    let defaults: OcrInput = serde_json::from_value(json!({ "input": two })).unwrap();
    assert_eq!(
        (defaults.lang.clone(), defaults.dpi),
        (langs(&["eng"]), 300)
    );
    let (r, j) = job(move |s| defaults.run(s));
    let (out, notes) = r.unwrap();
    assert_eq!(out, dir.path().join("two-ocr.pdf"));
    assert!(notes.is_empty());
    assert_eq!(j.notes, [NOTE]);
    assert_eq!(page_count(&out).unwrap(), 2);
    assert!(text(&out, 0).contains("4471"));
    assert_eq!(text(&out, 1).trim(), "");
    // A taken name gets -1; the existing file is left alone.
    let (r, _) = ocr_run(&scan, &out, &["eng"], 72);
    assert_eq!(r.unwrap(), dir.path().join("two-ocr-1.pdf"));
    assert_eq!(page_count(&out).unwrap(), 2);
}

#[test]
fn bad_options_fail_before_any_work() {
    let (dir, scan) = copy_fixture("invoice-scan.pdf", "scan.pdf");
    let out = dir.path().join("o.pdf");
    let code = |c: &str| {
        format!("invalid input: OCR language code {c:?} may hold only letters, digits and _")
    };
    let cases: [(&[&str], u32, String); 6] = [
        (&[], 300, "invalid input: no OCR language given".into()),
        (&["../eng"], 300, code("../eng")),
        (&["eng+deu"], 300, code("eng+deu")),
        (&["eng", ""], 300, code("")),
        (&["eng"], 71, "invalid input: dpi 71 not in 72..=600".into()),
        (
            &["eng"],
            601,
            "invalid input: dpi 601 not in 72..=600".into(),
        ),
    ];
    for (lang, dpi, want) in cases {
        let (r, notes) = ocr_run(&scan, &out, lang, dpi);
        assert_eq!(r.unwrap_err().to_string(), want);
        assert!(notes.is_empty());
    }
    assert_eq!(common::names(dir.path()), ["scan.pdf"]);
}

#[test]
fn a_missing_language_is_named_and_osd_is_not_a_language() {
    require_tesseract!();
    let installed = ocr::languages();
    assert!(installed.contains(&"eng".to_string()), "{installed:?}");
    assert!(!installed.contains(&"osd".to_string()), "{installed:?}");
    assert!(installed.is_sorted());
    let (dir, scan) = copy_fixture("invoice-scan.pdf", "scan.pdf");
    let out = dir.path().join("o.pdf");
    for (lang, missing) in [
        (&["deu"][..], "deu"),
        (&["eng", "deu"], "deu"),
        (&["osd"], "osd"),
    ] {
        let (r, _) = ocr_run(&scan, &out, lang, 300);
        let e = r.unwrap_err();
        assert!(
            matches!(&e, PagesError::MissingLanguage(m) if m == missing),
            "{e:?}"
        );
        assert_eq!(
            e.to_string(),
            format!("OCR language not installed: {missing}")
        );
    }
    assert_eq!(common::names(dir.path()), ["scan.pdf"]);
}

#[test]
fn encrypted_damaged_and_form_inputs() {
    require_tesseract!();
    let (dir, enc) = copy_fixture("encrypted.pdf", "enc.pdf");
    let with = |pw: Option<&str>| ocr_input(json!({ "input": enc, "password": pw, "dpi": 72 }));
    assert!(matches!(with(None), Err(PagesError::EncryptedInput)));
    assert!(matches!(with(Some("nope")), Err(PagesError::WrongPassword)));
    let (out, notes) = with(Some("user")).unwrap();
    assert_eq!(out, dir.path().join("enc-ocr.pdf"));
    assert_eq!(notes, ["output is not password-protected"]);
    assert_eq!(page_count(&out).unwrap(), 3);

    let bad = dir.path().join("bad.pdf");
    std::fs::write(&bad, b"%PDF-1.4\n1 0 obj << /Type /Catalog").unwrap();
    let (r, _) = ocr_run(&bad, &dir.path().join("b.pdf"), &["eng"], 72);
    assert!(matches!(r, Err(PagesError::Damaged)), "{r:?}");

    // Forms render through the field check (R113, R118).
    let dag = probe14_pdf(dir.path(), "fielddag:26");
    let (r, _) = ocr_run(&dag, &dir.path().join("d.pdf"), &["eng"], 72);
    assert_eq!(
        r.unwrap_err().to_string(),
        "damaged file: form field tree reaches a field twice"
    );
    for gone in ["b.pdf", "d.pdf"] {
        assert!(!dir.path().join(gone).exists());
    }
}
