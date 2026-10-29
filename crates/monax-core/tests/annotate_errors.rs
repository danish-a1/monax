//! annotate::save refusals: guard cases, bad annotations named by index,
//! free text that does not fit; nothing is ever written.
mod common;
use common::annot::*;
use common::guard::Op;
use common::{copy_fixture, names, setup};
use monax_core::annotate::{Annot, PageAnnot, ShapeKind, save};
use monax_core::pages::PagesError;
use std::path::PathBuf;

const OPS: [(&str, Op); 1] = [("annotate", |i, o| {
    save(i, o, &[on(0, highlight(rect(10.0, 10.0, 50.0, 30.0)))])
})];

guard_tests!(OPS, common::have_pdfium() && common::have_qpdf());

fn invalid(r: Result<PathBuf, PagesError>) -> String {
    match r {
        Err(PagesError::InvalidInput(m)) => m,
        other => panic!("expected InvalidInput, got {other:?}"),
    }
}

#[test]
fn bad_annotations_are_named_by_index_and_write_nothing() {
    require_qpdf!();
    let (dir, three) = setup();
    let out = dir.path().join("o.pdf");
    let ok = || on(0, highlight(rect(10.0, 10.0, 50.0, 30.0)));
    let try_with = |bad: PageAnnot| save(&three, &out, &[ok(), ok(), bad]);
    assert_eq!(invalid(save(&three, &out, &[])), "no annotations given");
    let many = vec![ok(); 1001];
    assert_eq!(
        invalid(save(&three, &out, &many)),
        "too many annotations: 1001 (at most 1000)"
    );
    let r = try_with(on(3, highlight(rect(10.0, 10.0, 50.0, 30.0))));
    assert!(
        matches!(r, Err(PagesError::PageOutOfRange { page: 3, count: 3 })),
        "{r:?}"
    );
    let cases = [
        (
            on(1, highlight(rect(150.0, 10.0, 201.0, 30.0))),
            "rect is outside the page's MediaBox",
        ),
        (
            on(1, highlight(rect(50.0, 10.0, 10.0, 30.0))),
            "rect is empty or not finite",
        ),
        (
            on(2, ink(&[&[(10.0, 10.0), (10.0, 250.0)]], 1.0)),
            "ink point is outside the page's MediaBox",
        ),
        (
            on(
                0,
                shape(ShapeKind::Rectangle, rect(1.0, 1.0, 9.0, 9.0), 0.1),
            ),
            "width must be 0.25..=20 points, got 0.1",
        ),
        (
            on(0, free_text(rect(10.0, 10.0, 60.0, 30.0), "\u{4e2d}", 10.0)),
            "character '\u{4e2d}' (U+4E2D) is not supported by Helvetica (WinAnsi)",
        ),
        (
            on(
                0,
                free_text(rect(10.0, 10.0, 60.0, 30.0), "A wide line of text", 10.0),
            ),
            "free text does not fit its rect at 10 pt",
        ),
        (
            on(0, free_text(rect(10.0, 10.0, 60.0, 30.0), "A\nB", 10.0)),
            "free text does not fit its rect at 10 pt",
        ),
        (
            on(
                0,
                Annot::Underline {
                    rect: rect(1.0, 1.0, 9.0, 9.0),
                    color: "#12".into(),
                },
            ),
            "color must be #RRGGBB, got \"#12\"",
        ),
    ];
    for (bad, why) in cases {
        assert_eq!(invalid(try_with(bad)), format!("annotation 2: {why}"));
    }
    assert_eq!(names(dir.path()), ["three.pdf"]);
}

#[test]
fn rects_are_bound_by_the_media_box_not_the_crop_box() {
    require_qpdf!();
    // MediaBox [0 0 300 200], CropBox [50 20 250 120], /Rotate 90.
    let (dir, rot) = copy_fixture("rotated-crop.pdf", "rot.pdf");
    let out = dir.path().join("o.pdf");
    let hidden = [on(0, highlight(rect(260.0, 150.0, 300.0, 200.0)))];
    assert!(save(&rot, &out, &hidden).is_ok());
    let over = [on(0, highlight(rect(260.0, 150.0, 301.0, 200.0)))];
    assert_eq!(
        invalid(save(&rot, &out, &over)),
        "annotation 0: rect is outside the page's MediaBox"
    );
}

#[test]
fn free_text_that_fits_exactly_is_kept() {
    require_qpdf!();
    let (dir, three) = setup();
    let out = dir.path().join("o.pdf");
    // One 10 pt line: 9.25 pt tall plus a 2 pt margin above and below.
    let a = [on(0, free_text(rect(10.0, 10.0, 190.0, 23.25), "Hi", 10.0))];
    assert!(save(&three, &out, &a).is_ok());
    let a = [on(0, free_text(rect(10.0, 10.0, 190.0, 23.0), "Hi", 10.0))];
    let m = invalid(save(&three, &out, &a));
    assert_eq!(m, "annotation 0: free text does not fit its rect at 10 pt");
}
