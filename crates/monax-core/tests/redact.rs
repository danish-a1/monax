//! redact: the redacted page is a 300 dpi raster with the boxes burned in
//! and nothing of the original page survives anywhere in the file; other
//! pages keep their content streams and text.
mod common;
use common::copy_fixture;
use common::secret::{assert_box, dark, page_contents, qdf, rd, secret_pdf};
use monax_core::pdfium;
use monax_core::security::redact;
use std::path::Path;

fn text(p: &Path, i: usize) -> String {
    pdfium::extract_text(p, i..=i).unwrap()
}

/// Asserts nothing of `secret.pdf`'s redacted page 1 is left in `out`,
/// raw or decoded, referenced or not.
fn assert_unrecoverable(out: &Path) {
    let raw = String::from_utf8_lossy(&std::fs::read(out).unwrap()).to_lowercase();
    assert!(!raw.contains("xyzzy"), "raw bytes keep the secret");
    let all = qdf(out).to_lowercase();
    for gone in [
        "xyzzy",
        "courier",
        "/thumb",
        "/widget",
        "/link",
        "/annots",
        "/acroform",
    ] {
        assert!(!all.contains(gone), "{gone} survived");
    }
    assert_eq!(text(out, 1), "");
}

#[test]
fn redacted_page_keeps_nothing_of_the_original() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let src = secret_pdf(dir.path());
    let before = page_contents(&src);
    let out = redact(&src, &dir.path().join("o.pdf"), &[rd(1, 20, 90, 120, 120)]).unwrap();
    assert_unrecoverable(&out);
    let after = page_contents(&out);
    assert_eq!((&after[0], &after[2]), (&before[0], &before[2]));
    assert_eq!(
        (text(&out, 0), text(&out, 2)),
        ("KeepAlpha".into(), "PLUGHPLUGH".into())
    );
    assert!(
        after[1].contains(" Do"),
        "raster page draws its image: {}",
        after[1]
    );
    assert_eq!(pdfium::displayed_size(&out, 1).unwrap(), (200.0, 200.0));
    common::docs::assert_clean(&out);
}

#[test]
fn several_pages_and_rects_in_one_job() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let src = secret_pdf(dir.path());
    let boxes = [
        rd(2, 10, 10, 50, 50),
        rd(1, 20, 90, 120, 120),
        rd(2, 100, 100, 190, 190),
        rd(1, 0, 0, 10, 10),
    ];
    let out = redact(&src, &dir.path().join("o.pdf"), &boxes).unwrap();
    assert_unrecoverable(&out);
    assert!(!qdf(&out).contains("PLUGH"));
    assert_eq!(text(&out, 2), "");
    assert_eq!(text(&out, 0), "KeepAlpha");
    assert_eq!(pdfium::page_count(&out).unwrap(), 3);
    // Every rect is black where it was put: page 2 has two.
    assert_box(&src, &out, 2, (10, 150, 49, 189));
    let r = pdfium::render(&out, 2, 72).unwrap();
    assert!(dark(&r, 150, 50) && dark(&r, 15, 155) && !dark(&r, 80, 80));
}

#[test]
fn rects_land_where_user_space_puts_them() {
    require_qpdf!();
    // (fixture, rect, expected 72 dpi pixel box as displayed)
    let cases = [
        (
            "three-pages.pdf",
            rd(0, 100, 20, 180, 60),
            (100, 140, 179, 179),
        ),
        // MediaBox 300x200, CropBox [50 20 250 120], /Rotate 90: displayed
        // 100 x 200; user (50, 20) is the displayed top-left corner.
        ("rotated-crop.pdf", rd(0, 60, 30, 100, 50), (10, 10, 29, 49)),
        // Inherited MediaBox [0 0 300 400], CropBox [10 10 200 300].
        (
            "inherited-boxes.pdf",
            rd(0, 110, 200, 150, 250),
            (100, 50, 139, 99),
        ),
    ];
    // rotated-crop.pdf turned to 90, 180 and 270: the rect covers the
    // "Rotated" glyphs (baseline at user (60, 60), 18 pt).
    let over_text = [
        ("+0", 90, (30, 5, 59, 89)),
        ("+90", 180, (110, 30, 194, 59)),
        ("+180", 270, (40, 110, 69, 194)),
    ];
    for (turn, deg, want) in over_text {
        let (dir, src) = copy_fixture("rotated-crop.pdf", "in.pdf");
        let turned = dir.path().join("turned.pdf");
        let rot = format!("--rotate={turn}");
        common::docs::qpdf(&[src.to_str().unwrap(), &rot, "--", turned.to_str().unwrap()]);
        assert_eq!(pdfium::rotation(&turned, 0).unwrap(), deg);
        let out = redact(
            &turned,
            &dir.path().join("o.pdf"),
            &[rd(0, 55, 50, 140, 80)],
        )
        .unwrap();
        assert_box(&turned, &out, 0, want);
    }
    for (name, r, want) in cases {
        let (dir, src) = copy_fixture(name, "in.pdf");
        let out = redact(&src, &dir.path().join("o.pdf"), &[r]).unwrap();
        assert_eq!(pdfium::rotation(&out, 0).unwrap(), 0, "{name}");
        assert_eq!(
            pdfium::displayed_size(&out, 0).unwrap(),
            pdfium::displayed_size(&src, 0).unwrap(),
            "{name}"
        );
        assert_eq!(text(&out, 0), "", "{name}");
        assert_box(&src, &out, 0, want);
    }
}

#[test]
fn a_rect_partly_off_the_page_is_clipped() {
    require_qpdf!();
    let (dir, src) = copy_fixture("rotated-crop.pdf", "in.pdf");
    // Crosses the CropBox's left edge (x 50) and the MediaBox's bottom.
    let out = redact(&src, &dir.path().join("o.pdf"), &[rd(0, 0, -10, 70, 40)]).unwrap();
    assert_box(&src, &out, 0, (0, 0, 19, 19));
}

#[test]
fn same_input_redacts_to_the_same_pages() {
    require_qpdf!();
    let (dir, src) = common::setup();
    let r = [rd(1, 10, 10, 90, 90)];
    let a = redact(&src, &dir.path().join("a.pdf"), &r).unwrap();
    let b = redact(&src, &dir.path().join("b.pdf"), &r).unwrap();
    assert_eq!(page_contents(&a), page_contents(&b));
    for i in 0..3 {
        assert_eq!(common::diff_box(&a, &b, i), None);
    }
}
