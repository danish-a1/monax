//! pdfium-only ops need no qpdf for a plain PDF (R46), also through their
//! input structs, which first check the input for encryption.
mod common;
use common::copy_fixture;
use monax_core::pages::{CropInput, InsertBlankInput, NupInput, SplitHalfInput};
use monax_core::pdfium::ExtractImagesInput;
use monax_core::read::{PageInfoInput, ReadTextInput, RenderPageInput};
use serde_json::json;

// Mutates process env: the only test in this binary.
#[test]
fn pdfium_ops_run_on_a_plain_pdf_without_qpdf() {
    require_pdfium!();
    let (_d, p) = copy_fixture("three-pages.pdf", "three.pdf");
    let (_i, img) = copy_fixture("image.pdf", "img.pdf");
    let empty = tempfile::tempdir().unwrap();
    // SAFETY: the only test in this binary.
    unsafe {
        std::env::set_var("MONAX_BIN_DIR", empty.path());
        std::env::remove_var("PATH");
    }
    assert!(monax_sidecar::find(monax_sidecar::Tool::Qpdf).is_none());
    let rect = json!({ "left": 10, "bottom": 10, "right": 100, "top": 100 });
    run_input!(NupInput, json!({ "input": p, "per_sheet": 2 })).unwrap();
    run_input!(CropInput, json!({ "input": p, "pages": [0], "rect": rect })).unwrap();
    run_input!(InsertBlankInput, json!({ "input": p, "after": [0] })).unwrap();
    run_input!(SplitHalfInput, json!({ "input": p })).unwrap();
    run_input!(PageInfoInput, json!({ "input": p })).unwrap();
    run_input!(ReadTextInput, json!({ "input": p })).unwrap();
    run_input!(RenderPageInput, json!({ "input": p, "page": 0 })).unwrap();
    let (outs, notes) = run_input!(ExtractImagesInput, json!({ "input": img })).unwrap();
    assert_eq!((outs.len(), notes.len()), (1, 0));
}
