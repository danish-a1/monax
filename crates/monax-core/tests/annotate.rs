//! annotate::save: every annotation type lands where its rect says, with an
//! appearance stream (qpdf JSON) that pdfium draws (render), on the listed
//! pages only.
mod common;
use common::annot::*;
use common::sign::near;
use common::{copy_fixture, diff_box, diff_box_in, setup};
use monax_core::annotate::{Annot, ShapeKind, save};
use serde_json::{Value, json};

/// Pixel rows/cols (72 dpi, 200 pt page) of user rect `l, b, r, t`, `pad`
/// pixels wider each way.
fn inked(a: &std::path::Path, b: &std::path::Path, page: usize, r: [u32; 4]) -> bool {
    let [l, bo, rt, t] = r;
    diff_box_in(a, b, page, l..rt, (200 - t)..(200 - bo)).is_some()
}

#[test]
fn highlight_is_written_at_its_rect_with_an_appearance() {
    require_qpdf!();
    let (dir, three) = setup();
    let a = [on(1, highlight(rect(40.0, 60.0, 140.0, 100.0)))];
    let out = save(&three, &dir.path().join("o.pdf"), &a).unwrap();
    let pages = dicts(&out);
    assert_eq!((pages[0].len(), pages[1].len(), pages[2].len()), (0, 1, 0));
    let h = &pages[1][0];
    assert_eq!(h["/Subtype"], "/Highlight");
    assert_eq!(nums(h, "/Rect"), [40.0, 60.0, 140.0, 100.0]);
    let quad = [40.0, 100.0, 140.0, 100.0, 40.0, 60.0, 140.0, 60.0];
    assert_eq!(nums(h, "/QuadPoints"), quad);
    assert_eq!(h["/C"], json!([1, 0.843, 0]));
    assert_eq!(h["/CA"], json!(0.4));
    assert_eq!(h["/F"], json!(4));
    assert!(has_normal_appearance(&out, h), "{h}");
    assert!(appearance(&out, h).contains("/GS0 gs"));
    near(diff_box(&three, &out, 1).unwrap(), (40, 100, 139, 139));
    assert_eq!(diff_box(&three, &out, 0), None, "page 0 changed");
    assert_eq!(diff_box(&three, &out, 2), None, "page 2 changed");
}

#[test]
fn every_type_has_an_appearance_drawn_inside_its_rect() {
    require_qpdf!();
    let (dir, three) = setup();
    let c = || "#00AA00".to_string();
    let a = [
        on(
            0,
            Annot::Underline {
                rect: rect(20.0, 20.0, 90.0, 40.0),
                color: c(),
            },
        ),
        on(
            0,
            Annot::Strikeout {
                rect: rect(110.0, 20.0, 180.0, 40.0),
                color: c(),
            },
        ),
        on(
            0,
            Annot::Note {
                rect: rect(20.0, 150.0, 60.0, 190.0),
                text: "Fix \u{3b1}".into(),
            },
        ),
        on(
            1,
            free_text(rect(20.0, 100.0, 180.0, 180.0), "Hello\nWorld", 14.0),
        ),
        on(1, ink(&[&[(20.0, 20.0), (60.0, 60.0), (100.0, 20.0)]], 3.0)),
        on(
            2,
            shape(ShapeKind::Rectangle, rect(20.0, 20.0, 90.0, 90.0), 2.0),
        ),
        on(
            2,
            shape(ShapeKind::Ellipse, rect(110.0, 20.0, 180.0, 90.0), 2.0),
        ),
    ];
    let out = save(&three, &dir.path().join("o.pdf"), &a).unwrap();
    let pages = dicts(&out);
    let types: Vec<Vec<&str>> = pages
        .iter()
        .map(|p| p.iter().map(|d| d["/Subtype"].as_str().unwrap()).collect())
        .collect();
    let want = [
        vec!["/Underline", "/StrikeOut", "/Text"],
        vec!["/FreeText", "/Ink"],
        vec!["/Square", "/Circle"],
    ];
    assert_eq!(types, want);
    for d in pages.iter().flatten() {
        assert!(has_normal_appearance(&out, d), "{d}");
    }
    for (page, r) in [
        (0, [20, 20, 90, 40]),
        (0, [110, 20, 180, 40]),
        (0, [20, 150, 60, 190]),
        (1, [20, 100, 180, 180]),
        (1, [20, 20, 100, 60]),
        (2, [20, 20, 90, 90]),
        (2, [110, 20, 180, 90]),
    ] {
        assert!(
            inked(&three, &out, page, r),
            "no ink on page {page} in {r:?}"
        );
    }
    // Nothing outside the rects (strokes straddle the edges by 1 pt).
    near(diff_box(&three, &out, 2).unwrap(), (19, 109, 180, 180));
}

#[test]
fn dictionaries_carry_what_readers_need() {
    require_qpdf!();
    let (dir, three) = setup();
    let a = [
        on(
            0,
            ink(
                &[&[(20.0, 20.0), (60.0, 60.0)], &[(70.0, 30.0), (90.0, 30.0)]],
                3.0,
            ),
        ),
        on(
            0,
            free_text(rect(20.0, 100.0, 180.0, 180.0), "Hello (1)", 14.0),
        ),
        on(
            0,
            Annot::Note {
                rect: rect(20.0, 150.0, 60.0, 190.0),
                text: "Fix \u{3b1}".into(),
            },
        ),
        on(
            0,
            shape(ShapeKind::Ellipse, rect(110.0, 20.0, 180.0, 90.0), 2.5),
        ),
    ];
    let out = save(&three, &dir.path().join("o.pdf"), &a).unwrap();
    let p = &dicts(&out)[0];
    assert_eq!(
        p[0]["/InkList"],
        json!([[20, 20, 60, 60], [70, 30, 90, 30]])
    );
    assert_eq!(p[0]["/BS"]["/W"], json!(3));
    assert_eq!(nums(&p[0], "/Rect"), [18.5, 18.5, 91.5, 61.5]);
    assert_eq!(p[1]["/DA"], "u:/Helv 14 Tf 0 0 0 rg");
    assert_eq!(p[1]["/Contents"], "u:Hello (1)");
    let ap = appearance(&out, &p[1]);
    assert!(ap.contains("/Helv 14 Tf"), "{ap}");
    assert!(ap.contains("(Hello \\(1\\)) Tj"), "{ap}");
    assert_eq!(p[2]["/Contents"], "u:Fix \u{3b1}");
    assert_eq!(p[2]["/Name"], "/Comment");
    assert_eq!(p[3]["/BS"]["/W"], json!(2.5));
    assert_eq!(nums(&p[3], "/Rect"), [108.75, 18.75, 181.25, 91.25]);
}

#[test]
fn inherited_media_box_bounds_the_rects() {
    require_qpdf!();
    // MediaBox [0 0 300 400] inherited from /Pages; the CropBox is smaller.
    let (dir, p) = copy_fixture("inherited-boxes.pdf", "inh.pdf");
    let a = [on(0, highlight(rect(250.0, 350.0, 290.0, 390.0)))];
    let out = save(&p, &dir.path().join("o.pdf"), &a).unwrap();
    assert_eq!(dicts(&out)[0].len(), 1);
}

#[test]
fn existing_annotations_are_kept_and_the_output_is_repeatable() {
    require_qpdf!();
    let (dir, p) = copy_fixture("annots.pdf", "a.pdf");
    let before = dicts(&p);
    let a = [on(0, highlight(rect(20.0, 20.0, 100.0, 40.0)))];
    let one = save(&p, &dir.path().join("o.pdf"), &a).unwrap();
    let two = save(&p, &dir.path().join("o.pdf"), &a).unwrap();
    let after = dicts(&one);
    let kinds = |v: &[Value]| -> Vec<Value> { v.iter().map(|d| d["/Subtype"].clone()).collect() };
    let mut want = kinds(&before[0]);
    want.push(json!("/Highlight"));
    assert_eq!(kinds(&after[0]), want);
    assert_eq!(kinds(&after[1]), kinds(&before[1]));
    let rects = |v: &[Value]| -> Vec<Value> { v.iter().map(|d| d["/Rect"].clone()).collect() };
    assert_eq!(rects(&after[0][..4]), rects(&before[0]));
    assert_eq!(after, dicts(&two));
    assert_eq!(
        appearance(&one, &after[0][4]),
        appearance(&two, &dicts(&two)[0][4])
    );
}
