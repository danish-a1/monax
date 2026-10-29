mod common;
use common::fixture;
use monax_core::pages::{self, MergeOpts, PagesError, Rect};
use monax_core::pdfium;
use std::path::PathBuf;
use tempfile::TempDir;

/// One page whose MediaBox [0 0 300 400] and CropBox [10 10 200 300] live only
/// on the /Pages node. Visible area: 190 x 290.
fn inh() -> (TempDir, PathBuf) {
    common::copy_fixture("inherited-boxes.pdf", "inh.pdf")
}

#[test]
fn inherited_boxes_are_read() {
    require_pdfium!();
    let (_d, p) = inh();
    assert_eq!(pdfium::page_size(&p, 0).unwrap(), (300., 400.));
    assert_eq!(pdfium::crop_box(&p, 0).unwrap(), (10., 10., 200., 300.));
    assert_eq!(pdfium::displayed_size(&p, 0).unwrap(), (190., 290.));
}

#[test]
fn ops_work_on_inherited_boxes() {
    require_pdfium!();
    let (d, p) = inh();
    let at = |n: &str| d.path().join(n);
    let r = |l, b, r, t| Rect {
        left: l,
        bottom: b,
        right: r,
        top: t,
    };
    let c = pages::crop(&p, &[0], r(0., 0., 300., 400.), &at("c.pdf")).unwrap();
    assert_eq!(pdfium::crop_box(&c, 0).unwrap(), (0., 0., 300., 400.));
    let bad = pages::crop(&p, &[0], r(0., 0., 301., 400.), &at("x.pdf"));
    assert!(matches!(bad, Err(PagesError::InvalidInput(_))));
    let b = pages::insert_blank(&p, &[0], &at("b.pdf")).unwrap();
    assert_eq!(pdfium::page_size(&b, 1).unwrap(), (190., 290.));
    assert_eq!(pdfium::crop_box(&b, 0).unwrap(), (10., 10., 200., 300.));
    let n = pages::nup(&p, 4, &at("n.pdf")).unwrap();
    assert_eq!(pdfium::page_size(&n, 0).unwrap(), (190., 290.));
    let h = pages::split_half(&p, &at("h.pdf")).unwrap();
    assert_eq!(pdfium::crop_box(&h, 0).unwrap(), (10., 10., 105., 300.));
    assert_eq!(pdfium::crop_box(&h, 1).unwrap(), (105., 10., 200., 300.));
    assert_eq!(pdfium::page_size(&h, 1).unwrap(), (300., 400.));
}

#[test]
fn blanks_take_the_displayed_size_of_a_rotated_cropped_page() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let rc = fixture("rotated-crop.pdf");
    let b = pages::insert_blank(&rc, &[0], &dir.path().join("b.pdf")).unwrap();
    assert_eq!(pdfium::page_size(&b, 1).unwrap(), (100., 200.));
    let opts = MergeOpts {
        blank_before_even: true,
    };
    let m = pages::merge(&[rc.clone(), rc], &dir.path().join("m.pdf"), opts).unwrap();
    assert_eq!(pdfium::page_size(&m, 1).unwrap(), (100., 200.));
}
