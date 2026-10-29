mod common;
use common::fixture;
use monax_core::pages::{self, PagesError};
use monax_sidecar::SidecarError;
use std::path::Path;

fn use_bin_dir(dir: &Path) {
    // SAFETY: called only from the single test fn in this binary.
    unsafe { std::env::set_var("MONAX_BIN_DIR", dir) };
}

fn count(dir: &Path) -> usize {
    std::fs::read_dir(dir).unwrap().count()
}

// Every scenario mutates process env, so they share one test to avoid races.
#[test]
fn failing_or_missing_qpdf_leaves_no_output() {
    require_pdfium!();
    let dir = tempfile::tempdir().unwrap();
    let three = dir.path().join("three.pdf");
    std::fs::copy(fixture("three-pages.pdf"), &three).unwrap();
    let out = dir.path().join("o.pdf");

    // qpdf passes preflight, writes a partial file, then exits 2: an op-time
    // failure reports qpdf's reason, never Damaged.
    use_bin_dir(&fixture("bin-partial"));
    let r = pages::extract(&three, &[0], &out);
    assert!(
        matches!(&r, Err(PagesError::Qpdf(m)) if m == "qpdf: boom"),
        "{r:?}"
    );
    assert_eq!(count(dir.path()), 1, "partial output left behind");

    // qpdf that fails every call (exit 1).
    let fail = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../monax-sidecar/tests/fixtures/bin-fail"
    );
    use_bin_dir(Path::new(fail));
    let r = pages::rotate(&three, &[0], 90, &out);
    assert!(
        matches!(&r, Err(PagesError::Qpdf(m)) if m == "exit code 1"),
        "{r:?}"
    );

    // qpdf absent.
    use_bin_dir(Path::new("/nonexistent"));
    // SAFETY: as above.
    unsafe { std::env::remove_var("PATH") };
    let r = pages::reorder(&three, &[0], &out);
    assert!(
        matches!(r, Err(PagesError::Sidecar(SidecarError::NotFound("qpdf")))),
        "{r:?}"
    );
    assert_eq!(count(dir.path()), 1);

    // The pdfium ops never need qpdf.
    let ok = [
        monax_core::content::page_numbers(&three, &out, Default::default()),
        pages::nup(&three, 2, &out),
        pages::crop(&three, &[0], BOX, &out),
        pages::insert_blank(&three, &[0], &out),
        pages::split_half(&three, &out),
    ];
    for r in ok {
        assert!(r.is_ok(), "{r:?}");
    }
}

const BOX: pages::Rect = pages::Rect {
    left: 0.,
    bottom: 0.,
    right: 100.,
    top: 100.,
};
