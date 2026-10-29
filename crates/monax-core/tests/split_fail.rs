mod common;
use common::fixture;
use monax_core::pages::{self, PagesError, RangeSpec};

#[test]
fn mid_way_failure_removes_published_outputs() {
    require_pdfium!();
    let dir = tempfile::tempdir().unwrap();
    let three = dir.path().join("three.pdf");
    std::fs::copy(fixture("three-pages.pdf"), &three).unwrap();
    let out = tempfile::tempdir().unwrap();
    // SAFETY: the only test in this binary.
    unsafe { std::env::set_var("MONAX_BIN_DIR", fixture("bin-second-fails")) };
    let r = [0, 1, 2].map(|i| RangeSpec { start: i, end: i });
    let r = pages::split_pages(&three, &r, out.path(), "%[index].pdf");
    assert!(
        matches!(&r, Err(PagesError::Qpdf(m)) if m == "qpdf: boom"),
        "{r:?}"
    );
    let left: Vec<_> = std::fs::read_dir(out.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .filter(|n| n.to_string_lossy().ends_with(".pdf"))
        .collect();
    assert!(left.is_empty(), "left behind: {left:?}");
}
