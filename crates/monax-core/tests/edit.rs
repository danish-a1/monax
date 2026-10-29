mod common;
use common::{fixture, setup};
use monax_core::pages::{self, PagesError, Rect};
use monax_core::pdfium;
use std::path::Path;

fn rect(left: f32, bottom: f32, right: f32, top: f32) -> Rect {
    Rect {
        left,
        bottom,
        right,
        top,
    }
}

fn count(p: &Path) -> usize {
    pdfium::page_count(p).unwrap()
}

#[test]
fn nup_sheet_count_and_size() {
    require_pdfium!();
    let (dir, three) = setup();
    let two = pages::nup(&three, 2, &dir.path().join("n2.pdf")).unwrap();
    assert_eq!(count(&two), 2);
    let four = pages::nup(&three, 4, &dir.path().join("n4.pdf")).unwrap();
    assert_eq!(count(&four), 1);
    assert_eq!(pdfium::page_size(&four, 0).unwrap(), (200.0, 200.0));
    // Portrait source (100 x 200): 2-up and 6-up sheets are landscape.
    let narrow = pages::crop(
        &three,
        &[0, 1, 2],
        rect(0., 0., 100., 200.),
        &dir.path().join("c.pdf"),
    )
    .unwrap();
    for k in [2u8, 6] {
        let n = pages::nup(&narrow, k, &dir.path().join(format!("w{k}.pdf"))).unwrap();
        assert_eq!(pdfium::page_size(&n, 0).unwrap(), (200.0, 100.0), "{k}");
    }
    let n = pages::nup(&narrow, 4, &dir.path().join("w4.pdf")).unwrap();
    assert_eq!(pdfium::page_size(&n, 0).unwrap(), (100.0, 200.0));
}

#[test]
fn nup_rejects_bad_per_sheet() {
    require_pdfium!();
    let (dir, three) = setup();
    for k in [0u8, 1, 3, 5, 8, 10, 25] {
        let r = pages::nup(&three, k, &dir.path().join("o.pdf"));
        assert!(matches!(r, Err(PagesError::InvalidInput(_))), "{k}");
    }
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn crop_sets_cropbox_on_listed_pages_only() {
    require_pdfium!();
    let (dir, three) = setup();
    let out = dir.path().join("c.pdf");
    let got = pages::crop(&three, &[1], rect(10., 20., 110., 120.), &out).unwrap();
    assert_eq!(pdfium::crop_box(&got, 1).unwrap(), (10., 20., 110., 120.));
    assert_eq!(pdfium::crop_box(&got, 0).unwrap(), (0., 0., 200., 200.));
}

#[test]
fn crop_rejects_empty_and_outside_rects() {
    require_pdfium!();
    let (dir, three) = setup();
    let out = dir.path().join("c.pdf");
    for r in [
        rect(50., 50., 50., 100.),
        rect(60., 50., 50., 100.),
        rect(10., 10., 10., 10.),
        rect(-1., 0., 100., 100.),
        rect(0., 0., 201., 100.),
        rect(0., 0., 100., f32::NAN),
    ] {
        let got = pages::crop(&three, &[0], r, &out);
        assert!(matches!(got, Err(PagesError::InvalidInput(_))), "{r:?}");
    }
    assert!(matches!(
        pages::crop(&three, &[3], rect(0., 0., 1., 1.), &out),
        Err(PagesError::PageOutOfRange { page: 3, count: 3 })
    ));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn crop_on_rotated_cropped_page_uses_unrotated_user_space() {
    require_pdfium!();
    let dir = tempfile::tempdir().unwrap();
    let rc = fixture("rotated-crop.pdf");
    let out = dir.path().join("c.pdf");
    let got = pages::crop(&rc, &[0], rect(0., 0., 300., 100.), &out).unwrap();
    assert_eq!(pdfium::crop_box(&got, 0).unwrap(), (0., 0., 300., 100.));
    assert_eq!(pdfium::rotation(&got, 0).unwrap(), 90);
    assert_eq!(pdfium::page_size(&got, 0).unwrap(), (300., 200.));
}

#[test]
fn insert_blank_after_listed_pages() {
    require_pdfium!();
    let (dir, three) = setup();
    let rc = fixture("rotated-crop.pdf");
    let got = pages::insert_blank(&rc, &[0], &dir.path().join("rc.pdf")).unwrap();
    assert_eq!(count(&got), 2);
    assert_eq!(
        pdfium::page_size(&got, 1).unwrap(),
        (100., 200.) // displayed size of the rotated, cropped neighbour
    );
    let got = pages::insert_blank(&three, &[0, 2, 0], &dir.path().join("b.pdf")).unwrap();
    assert_eq!(count(&got), 5);
    let text = |i| pdfium::extract_text(&got, i..=i).unwrap();
    let t: Vec<bool> = (0..5).map(|i| text(i).trim().is_empty()).collect();
    assert_eq!(t, [false, true, false, false, true]);
    assert_eq!(pdfium::page_size(&got, 1).unwrap(), (200., 200.));
}

#[test]
fn insert_blank_rejects_bad_indices() {
    require_pdfium!();
    let (dir, three) = setup();
    let out = dir.path().join("o.pdf");
    assert!(matches!(
        pages::insert_blank(&three, &[3], &out),
        Err(PagesError::PageOutOfRange { page: 3, count: 3 })
    ));
    assert!(matches!(
        pages::insert_blank(&three, &[], &out),
        Err(PagesError::InvalidInput(_))
    ));
}

#[test]
fn split_half_doubles_pages_and_halves_width() {
    require_pdfium!();
    let (dir, three) = setup();
    let got = pages::split_half(&three, &dir.path().join("h.pdf")).unwrap();
    assert_eq!(count(&got), 6);
    assert_eq!(pdfium::crop_box(&got, 0).unwrap(), (0., 0., 100., 200.));
    assert_eq!(pdfium::crop_box(&got, 1).unwrap(), (100., 0., 200., 200.));
    let words = |i| pdfium::extract_text(&got, i..=i).unwrap();
    assert!(words(2).contains("two") && words(3).contains("two"));
}

#[test]
fn split_half_follows_the_visual_axis_on_rotated_pages() {
    require_pdfium!();
    let dir = tempfile::tempdir().unwrap();
    let rc = fixture("rotated-crop.pdf");
    let got = pages::split_half(&rc, &dir.path().join("h.pdf")).unwrap();
    assert_eq!(count(&got), 2);
    // /Rotate 90 (clockwise): the visual left half is the bottom of user space.
    assert_eq!(pdfium::crop_box(&got, 0).unwrap(), (50., 20., 250., 70.));
    assert_eq!(pdfium::crop_box(&got, 1).unwrap(), (50., 70., 250., 120.));
    assert_eq!(pdfium::rotation(&got, 0).unwrap(), 90);
}
