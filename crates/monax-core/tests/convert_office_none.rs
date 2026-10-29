//! Without the bundled `wordcraft-cli`, Office to PDF fails with
//! `OfficeUnavailable` before anything is read or run; a `wordcraft-cli` on
//! `PATH` is never used (R152).
mod common;
use common::convert::idle;
use monax_core::convert::{OfficeToPdfInput, office_available, office_to_pdf};
use monax_core::pages::PagesError;
use serde_json::json;
use std::path::Path;

const MISSING: &str = "WordCraft not found - the bundled converter is missing";

// Mutates process env: the only test in this binary.
#[test]
fn without_wordcraft_nothing_runs() {
    // SAFETY: the only test in this binary.
    unsafe {
        std::env::set_var("MONAX_BIN_DIR", "/nonexistent");
        std::env::set_var("PATH", common::fixture("bin-wordcraft"));
    }
    assert_eq!(office_available(), None);
    // Not even the input is looked at: it does not exist.
    let (input, dir) = (Path::new("/nonexistent/a.docx"), Path::new("/nonexistent"));
    let unavailable = |r: Result<_, PagesError>| match r {
        Err(e @ PagesError::OfficeUnavailable) => assert_eq!(e.to_string(), MISSING),
        r => panic!("expected OfficeUnavailable, got {r:?}"),
    };
    unavailable(office_to_pdf(input, dir, &idle()));
    let d = tempfile::tempdir().unwrap();
    let doc = d.path().join("a.docx");
    std::fs::write(&doc, b"PK\x03\x04").unwrap();
    let i: OfficeToPdfInput = serde_json::from_value(json!({ "input": doc })).unwrap();
    unavailable(i.run(&idle()));
    assert_eq!(common::names(d.path()), ["a.docx"]);
}
