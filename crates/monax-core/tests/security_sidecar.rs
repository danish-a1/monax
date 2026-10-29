//! Failing or missing qpdf: inspect (and the security ops) report the exact
//! reason and leave no file; a failing probe never reads as "not encrypted".
mod common;
use common::{fixture, names};
use monax_core::pages::PagesError;
use monax_core::security::{self, InspectSecurityInput};
use monax_sidecar::SidecarError;
use std::path::Path;

fn inspect_into(input: &Path, out: &Path) -> Result<std::path::PathBuf, PagesError> {
    let v = serde_json::json!({ "input": input, "out": out });
    serde_json::from_value::<InspectSecurityInput>(v)
        .unwrap()
        .run()
}

// Every scenario mutates process env, so they share one test to avoid races.
#[test]
fn failing_or_missing_qpdf_is_reported_without_output() {
    require_pdfium!();
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("p.pdf");
    std::fs::copy(fixture("three-pages.pdf"), &p).unwrap();
    let out = dir.path().join("o.json");
    let fail = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../monax-sidecar/tests/fixtures/bin-fail"
    );
    // SAFETY: the only test in this binary.
    unsafe { std::env::set_var("MONAX_BIN_DIR", fail) };
    let r = security::inspect(&p, None);
    assert!(
        matches!(&r, Err(PagesError::Qpdf(m)) if m == "exit code 1"),
        "{r:?}"
    );
    let r = inspect_into(&p, &out);
    assert!(
        matches!(&r, Err(PagesError::Qpdf(m)) if m == "exit code 1"),
        "{r:?}"
    );
    assert_eq!(names(dir.path()), ["p.pdf"]);

    // SAFETY: as above.
    unsafe {
        std::env::set_var("MONAX_BIN_DIR", "/nonexistent");
        std::env::remove_var("PATH");
    }
    let r = inspect_into(&p, &out);
    let missing = matches!(r, Err(PagesError::Sidecar(SidecarError::NotFound("qpdf"))));
    assert!(missing, "{r:?}");
    assert_eq!(names(dir.path()), ["p.pdf"]);
}
