mod common;
use common::guard::{Op, invalid};
use common::{names, setup};
use monax_core::pages::{self, MergeOpts, PagesError};
use std::path::{Path, PathBuf};

/// Every single-input op, as `(input, out) -> result`.
const OPS: [(&str, Op); 6] = [
    ("merge", |i, o| {
        pages::merge(&[i.to_path_buf()], o, MergeOpts::default())
    }),
    ("extract", |i, o| pages::extract(i, &[0], o)),
    ("delete", |i, o| pages::delete(i, &[0], o)),
    ("rotate", |i, o| pages::rotate(i, &[0], 90, o)),
    ("reorder", |i, o| pages::reorder(i, &[0], o)),
    ("alternate_mix", |i, o| pages::alternate_mix(i, i, false, o)),
];

guard_tests!(OPS, common::have_pdfium() && common::have_qpdf());

#[test]
fn bad_inputs_and_page_lists() {
    require_qpdf!();
    let (dir, three) = setup();
    let out = dir.path().join("o.pdf");
    let oor = |r: Result<PathBuf, PagesError>| {
        matches!(r, Err(PagesError::PageOutOfRange { page: 3, count: 3 }))
    };
    assert!(oor(pages::extract(&three, &[0, 3], &out)));
    assert!(oor(pages::delete(&three, &[3], &out)));
    assert!(oor(pages::rotate(&three, &[3], 90, &out)));
    assert!(oor(pages::reorder(&three, &[1, 3], &out)));
    assert!(invalid(pages::extract(&three, &[], &out)));
    assert!(invalid(pages::delete(&three, &[], &out)));
    assert!(invalid(pages::rotate(&three, &[], 90, &out)));
    assert!(invalid(pages::reorder(&three, &[], &out)));
    assert!(invalid(pages::merge(&[], &out, MergeOpts::default())));
    let missing = dir.path().join("missing.pdf");
    assert!(invalid(pages::extract(&missing, &[0], &out)));
    assert!(invalid(pages::extract(Path::new("three.pdf"), &[0], &out)));
    assert!(invalid(pages::alternate_mix(&three, &missing, false, &out)));
    assert_eq!(names(dir.path()), ["three.pdf"]);
}
