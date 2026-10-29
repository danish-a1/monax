//! Header/footer, watermark and Bates: text, color, options, errors.
mod common;
use common::{copy_fixture, diff_box, setup};
use monax_core::content::{self, Anchor, BatesOpts, HfOpts, NumberOpts, Row, WatermarkOpts};
use monax_core::pages::PagesError;
use monax_core::pdfium;
use std::path::Path;

fn hf(header_left: &str, footer_right: &str) -> HfOpts {
    HfOpts {
        header: Row {
            left: header_left.into(),
            ..Row::default()
        },
        footer: Row {
            right: footer_right.into(),
            ..Row::default()
        },
        margin: 10.0,
        ..HfOpts::default()
    }
}

fn mark(text: &str) -> WatermarkOpts {
    WatermarkOpts {
        text: text.into(),
        ..WatermarkOpts::default()
    }
}

fn text(p: &Path, i: usize) -> String {
    pdfium::extract_text(p, i..=i).unwrap()
}

#[test]
fn header_footer_fill_their_corners_with_page_numbers() {
    require_pdfium!();
    let (dir, three) = setup();
    let o = hf("Head %[page]", "%[page] of %[pages]");
    let out = content::header_footer(&three, &dir.path().join("hf.pdf"), o).unwrap();
    for i in 0..3 {
        let t = text(&out, i);
        assert!(t.contains(&format!("Head {}", i + 1)), "{t}");
        assert!(t.contains(&format!("{} of 3", i + 1)), "{t}");
        // Ink spans from the top-left corner to the bottom-right corner.
        let (x0, y0, x1, y1) = diff_box(&three, &out, i).unwrap();
        assert!(x0.abs_diff(10) <= 2 && (10..=16).contains(&y0), "{x0},{y0}");
        assert!(x1.abs_diff(189) <= 2 && y1.abs_diff(189) <= 2, "{x1},{y1}");
    }
    let r = content::header_footer(&three, &dir.path().join("e.pdf"), HfOpts::default());
    assert!(
        matches!(&r, Err(PagesError::InvalidInput(m)) if m == "header and footer are all empty")
    );
}

#[test]
fn watermark_is_translucent_colored_and_placed() {
    require_pdfium!();
    let (dir, three) = setup();
    // Top center, clear of the page's own text. The margin reaches the
    // font's ascent, a little above the capitals' ink.
    let o = WatermarkOpts {
        color: [255, 0, 0],
        opacity: 0.5,
        angle: 0.0,
        size: 30.0,
        position: Anchor::TopCenter,
        margin: 10.0,
        ..mark("WM")
    };
    let out = content::watermark(&three, &dir.path().join("w.pdf"), o).unwrap();
    let (x0, y0, x1, y1) = diff_box(&three, &out, 0).unwrap();
    assert!(
        ((x0 + x1) as i32 - 199).abs() <= 3 && (10..=16).contains(&y0),
        "{x0} {y0} {x1}"
    );
    let r = pdfium::render(&out, 0, 72).unwrap();
    let px = |x: u32, y: u32| &r.rgba[((y * r.w + x) * 4) as usize..][..3];
    let ink: Vec<_> = (y0..=y1)
        .flat_map(|y| (x0..=x1).map(move |x| (x, y)))
        .map(|(x, y)| px(x, y))
        .collect();
    // Half-opaque red on white: red stays high, green never drops near 0.
    assert!(
        ink.iter().any(|p| p[0] > 240 && (110..150).contains(&p[1])),
        "no translucent red"
    );
    assert!(ink.iter().all(|p| p[1] > 100), "opaque ink found");
    assert!(text(&out, 0).contains("WM"));
}

#[test]
fn rotated_watermark_spreads_diagonally() {
    require_pdfium!();
    let (dir, three) = setup();
    let o = WatermarkOpts {
        angle: 45.0,
        size: 24.0,
        opacity: 1.0,
        ..mark("DIAGONAL")
    };
    let out = content::watermark(&three, &dir.path().join("w.pdf"), o).unwrap();
    let (x0, y0, x1, y1) = diff_box(&three, &out, 2).unwrap();
    let (w, h) = (x1 - x0, y1 - y0);
    assert!(w > 80 && w.abs_diff(h) <= 12, "not diagonal: {w}x{h}");
}

#[test]
fn bates_numbers_are_padded_and_sequential() {
    require_pdfium!();
    let (dir, three) = setup();
    let o = BatesOpts {
        prefix: "ABC-".into(),
        suffix: "-X".into(),
        start: 7,
        digits: 4,
        ..BatesOpts::default()
    };
    let out = content::bates(&three, &dir.path().join("b.pdf"), o).unwrap();
    for (i, n) in ["0007", "0008", "0009"].iter().enumerate() {
        assert!(
            text(&out, i).contains(&format!("ABC-{n}-X")),
            "{}",
            text(&out, i)
        );
    }
    let at = dir.path().join("o.pdf");
    let bad = |o: BatesOpts| content::bates(&three, &at, o).unwrap_err().to_string();
    let d = BatesOpts::default();
    assert_eq!(
        bad(BatesOpts {
            digits: 0,
            ..d.clone()
        }),
        "invalid input: digits must be 1..=12, got 0"
    );
    assert_eq!(
        bad(BatesOpts {
            digits: 13,
            ..d.clone()
        }),
        "invalid input: digits must be 1..=12, got 13"
    );
    let over = bad(BatesOpts {
        digits: 1,
        start: 8,
        ..d.clone()
    });
    assert_eq!(
        over,
        "invalid input: Bates number 8 + 2 needs more than 1 digits"
    );
    assert!(
        bad(BatesOpts {
            start: u64::MAX,
            ..d
        })
        .contains("needs more than")
    );
}

#[test]
fn stamps_survive_content_that_leaves_the_ctm_scaled() {
    require_pdfium!();
    // Page content ends with an unbalanced `0.5 0 0 0.5 0 0 cm`.
    let (dir, p) = copy_fixture("unbalanced-ctm.pdf", "u.pdf");
    let o = NumberOpts {
        format: "Page %[page]".into(),
        margin: 10.0,
        ..NumberOpts::default()
    };
    let out = content::page_numbers(&p, &dir.path().join("n.pdf"), o).unwrap();
    let (x0, _, x1, y1) = diff_box(&p, &out, 0).unwrap();
    assert!(
        ((x0 + x1) as i32 - 199).abs() <= 3 && y1.abs_diff(189) <= 2,
        "{x0} {x1} {y1}"
    );
}
