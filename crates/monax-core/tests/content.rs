//! Overlay placement: ink lands where the user sees the page.
mod common;
use common::{copy_fixture, diff_box, diff_box_in, setup};
use monax_core::content::{self, Anchor, HfOpts, NumberOpts, Row};
use monax_core::{pages, pdfium};
use std::path::Path;

fn text(p: &Path, i: usize) -> String {
    pdfium::extract_text(p, i..=i).unwrap()
}

/// Asserts the diff ink of `page` is upright (wider than tall), horizontally
/// centered, and its bottom sits `margin` points (= px at 72 dpi) above the
/// displayed bottom edge. The margin is measured to the font's descent, so
/// the text needs a descender ('g', 'p').
fn bottom_center(orig: &Path, out: &Path, page: usize, margin: u32) {
    let (w, h) = pdfium::displayed_size(orig, page).unwrap();
    let (x0, y0, x1, y1) = diff_box(orig, out, page).expect("no ink");
    let mid = (x0 + x1) as f32 / 2.0;
    assert!(
        (mid - w / 2.0).abs() <= 2.0,
        "not centered: {x0}..{x1} on {w}"
    );
    assert!(x1 - x0 > y1 - y0, "not upright: {x0},{y0} {x1},{y1}");
    let gap = h as u32 - 1 - y1;
    assert!(
        gap.abs_diff(margin) <= 2,
        "bottom gap {gap}, margin {margin}"
    );
}

fn numbers(format: &str, margin: f32) -> NumberOpts {
    NumberOpts {
        format: format.into(),
        margin,
        size: 12.0,
        ..NumberOpts::default()
    }
}

#[test]
fn page_numbers_render_bottom_center_on_every_page() {
    require_pdfium!();
    let (dir, three) = setup();
    let o = numbers("Page %[page] of %[pages]", 10.0);
    let out = content::page_numbers(&three, &dir.path().join("n.pdf"), o).unwrap();
    for i in 0..3 {
        bottom_center(&three, &out, i, 10);
        let t = text(&out, i);
        assert!(t.contains(&format!("Page {} of 3", i + 1)), "{t}");
    }
}

#[test]
fn page_numbers_follow_rotation_and_crop() {
    require_pdfium!();
    // /Rotate 90, CropBox [50 20 250 120]: displayed 100 x 200.
    let (dir, rot) = copy_fixture("rotated-crop.pdf", "rot.pdf");
    let out = content::page_numbers(
        &rot,
        &dir.path().join("n.pdf"),
        numbers("pg %[page]/%[pages]", 8.0),
    )
    .unwrap();
    bottom_center(&rot, &out, 0, 8);
    let t = text(&out, 0);
    assert!(t.contains("1/1"), "{t:?}");
    // Inherited MediaBox [0 0 300 400] + CropBox [10 10 200 300].
    let (dir, inh) = copy_fixture("inherited-boxes.pdf", "inh.pdf");
    let out = content::page_numbers(
        &inh,
        &dir.path().join("n.pdf"),
        numbers("Page %[page]", 20.0),
    )
    .unwrap();
    bottom_center(&inh, &out, 0, 20);
}

#[test]
fn anchors_land_in_their_corner_on_a_rotated_page() {
    require_pdfium!();
    let (dir, rot) = copy_fixture("rotated-crop.pdf", "rot.pdf");
    let at = |position, n: &str| {
        let o = NumberOpts {
            position,
            ..numbers("Xg", 5.0)
        };
        content::page_numbers(&rot, &dir.path().join(n), o).unwrap()
    };
    let tl = diff_box(&rot, &at(Anchor::TopLeft, "tl.pdf"), 0).unwrap();
    assert!(tl.0.abs_diff(5) <= 2 && tl.1.abs_diff(5) <= 2, "{tl:?}");
    let br = diff_box(&rot, &at(Anchor::BottomRight, "br.pdf"), 0).unwrap();
    assert!(br.2.abs_diff(94) <= 2 && br.3.abs_diff(194) <= 2, "{br:?}");
}

#[test]
fn selected_pages_count_from_start_at() {
    require_pdfium!();
    let (dir, three) = setup();
    let o = NumberOpts {
        start_at: 5,
        pages: Some(vec![2, 0, 2]),
        ..numbers("No. %[page]", 10.0)
    };
    let out = content::page_numbers(&three, &dir.path().join("n.pdf"), o).unwrap();
    assert!(text(&out, 0).contains("No. 5"));
    assert!(
        diff_box(&three, &out, 1).is_none(),
        "unselected page changed"
    );
    assert!(text(&out, 2).contains("No. 6"));
}

#[test]
fn page_numbers_upright_at_every_rotation() {
    require_qpdf!();
    // rotated-crop.pdf turned further: /Rotate 180, 270 and 0 (= 360).
    let (dir, rot) = copy_fixture("rotated-crop.pdf", "rot.pdf");
    for by in [90i16, 180, 270] {
        let turned = pages::rotate(&rot, &[0], by, &dir.path().join(format!("r{by}.pdf"))).unwrap();
        let o = numbers("Page %[page]", 6.0);
        let out =
            content::page_numbers(&turned, &dir.path().join(format!("n{by}.pdf")), o).unwrap();
        bottom_center(&turned, &out, 0, 6);
        assert!(text(&out, 0).contains("Page 1"), "{by}");
    }
}

#[test]
fn slots_in_one_row_share_a_baseline() {
    require_pdfium!();
    let (dir, three) = setup();
    // 'x' and 'H' sit on the baseline (same ink bottom); 'x' and 'g' share
    // the x-height (same ink top).
    let o = HfOpts {
        header: Row {
            left: "xxxx".into(),
            right: "HHHH".into(),
            ..Row::default()
        },
        footer: Row {
            left: "xxxx".into(),
            right: "gggg".into(),
            ..Row::default()
        },
        size: 20.0,
        margin: 10.0,
        ..HfOpts::default()
    };
    let out = content::header_footer(&three, &dir.path().join("hf.pdf"), o).unwrap();
    let part = |cols, top: bool| {
        let rows = if top { 0..60 } else { 140..200 };
        diff_box_in(&three, &out, 0, cols, rows).unwrap()
    };
    let (hx, hh) = (part(0..100, true), part(100..200, true));
    assert!(
        hx.3.abs_diff(hh.3) <= 1,
        "header baselines: {hx:?} vs {hh:?}"
    );
    let (fx, fg) = (part(0..100, false), part(100..200, false));
    assert!(
        fx.1.abs_diff(fg.1) <= 1,
        "footer baselines: {fx:?} vs {fg:?}"
    );
}
