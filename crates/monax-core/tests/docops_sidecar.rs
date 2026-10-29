//! Failing or missing qpdf: the document-level ops report the exact reason
//! and leave no file; the pdfium ones need qpdf only for a form.
mod common;
use common::fixture;
use monax_core::content::{self, CompressOpts, Metadata};
use monax_core::forms;
use monax_core::pages::PagesError;
use monax_sidecar::SidecarError;
use std::path::{Path, PathBuf};

type R = Result<PathBuf, PagesError>;

fn qpdf_ops(i: &Path, o: &Path) -> [R; 4] {
    [
        content::compress(i, o, CompressOpts::default()),
        content::set_metadata(i, o, Metadata::default()),
        content::repair(i, o),
        content::linearize(i, o),
    ]
}

// Every scenario mutates process env, so they share one test to avoid races.
#[test]
fn failing_or_missing_qpdf_leaves_no_output() {
    require_pdfium!();
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.pdf");
    std::fs::copy(fixture("annots.pdf"), &a).unwrap();
    let out = dir.path().join("o.pdf");
    let only_input = || assert_eq!(common::names(dir.path()), ["a.pdf"]);

    // SAFETY: the only test in this binary.
    unsafe { std::env::set_var("MONAX_BIN_DIR", fixture("bin-partial")) };
    for r in qpdf_ops(&a, &out) {
        assert!(
            matches!(&r, Err(PagesError::Qpdf(m)) if m == "qpdf: boom"),
            "{r:?}"
        );
    }
    only_input();
    // qpdf's exit 2 reading a form to fill reads as damage, as in the
    // form field check.
    let e = forms::fill(&a, &out, &[fill_name()]).unwrap_err();
    assert_eq!(e.to_string(), "damaged file: repair it first");
    assert_eq!(
        forms::list_fields(&a).unwrap_err().to_string(),
        "qpdf failed: qpdf: boom"
    );
    only_input();

    // SAFETY: as above.
    unsafe {
        std::env::set_var("MONAX_BIN_DIR", "/nonexistent");
        std::env::remove_var("PATH");
    }
    for r in qpdf_ops(&a, &out) {
        let missing = matches!(r, Err(PagesError::Sidecar(SidecarError::NotFound("qpdf"))));
        assert!(missing, "{r:?}");
    }
    only_input();
    // R113: drawing a form, or walking its widgets, needs its field tree
    // checked, which needs qpdf.
    let e = content::flatten(&a, &dir.path().join("f.pdf")).unwrap_err();
    assert_eq!(e.to_string(), "form field check failed: qpdf not found");
    only_input();
    let e = content::delete_annotations(&a, &dir.path().join("d.pdf"), None).unwrap_err();
    assert_eq!(e.to_string(), "form field check failed: qpdf not found");
    only_input();
    let e = forms::fill(&a, &out, &[fill_name()]).unwrap_err();
    assert!(
        matches!(e, PagesError::Sidecar(SidecarError::NotFound("qpdf"))),
        "{e:?}"
    );
    only_input();
}

fn fill_name() -> forms::FieldValue {
    forms::FieldValue {
        name: "name".into(),
        value: "x".into(),
    }
}
