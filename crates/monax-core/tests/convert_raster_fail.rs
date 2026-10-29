//! PDF to images is all-or-nothing: a page that fails, or a cancel, removes
//! the files already written.
mod common;
use common::convert::{idle, invalid, to_images};
use common::{names, setup};
use monax_core::convert::{ImageFormat, PageSize, images_to_pdf, pdf_to_images};
use monax_core::pages::{MergeOpts, PagesError, merge};

#[test]
fn a_page_over_budget_removes_the_pages_already_written() {
    require_qpdf!();
    let (dir, three) = setup();
    let img = common::sign::png(dir.path(), "i.png", (4, 4), |_, _| common::sign::RED);
    let huge = PageSize {
        width: 14400.0,
        height: 14400.0,
    };
    let big = images_to_pdf(&[img], &dir.path().join("big.pdf"), Some(huge), &idle()).unwrap();
    let mixed = dir.path().join("mixed.pdf");
    merge(&[three, big], &mixed, MergeOpts::default()).unwrap();
    let before = names(dir.path());
    let m = invalid(to_images(&mixed, 300, ImageFormat::Png, Some(&[0, 3])));
    // 14400 pt at 300 dpi: 60000 px a side, give or take pdfium's rounding.
    let tail = " px exceeds the 64000000 pixel budget at 300 dpi; lower the dpi";
    assert!(m.starts_with("page 4: 6000") && m.ends_with(tail), "{m}");
    assert_eq!(names(dir.path()), before);
}

#[test]
fn a_cancelled_job_stops_before_the_next_page_and_leaves_no_file() {
    require_pdfium!();
    let (dir, three) = setup();
    let d = dir.path().to_path_buf();
    let reg = monax_core::Registry::new();
    let (go, wait) = std::sync::mpsc::channel::<()>();
    let (tx, rx) = std::sync::mpsc::channel();
    let id = reg.submit(move |s| {
        wait.recv().unwrap();
        tx.send(pdf_to_images(
            &three,
            &d,
            72,
            ImageFormat::Png,
            85,
            None,
            &s,
        ))
        .unwrap();
        Ok::<_, String>(vec![])
    });
    reg.cancel(id);
    go.send(()).unwrap();
    let r = rx.recv().unwrap();
    assert!(matches!(r, Err(PagesError::Cancelled)), "{r:?}");
    assert_eq!(names(dir.path()), ["three.pdf"]);
}

#[test]
fn a_page_past_the_jpeg_side_limit_is_refused_before_rendering() {
    require_qpdf!();
    let (dir, _) = setup();
    let img = common::sign::png(dir.path(), "i.png", (4, 4), |_, _| common::sign::RED);
    let strip = PageSize {
        width: 14400.0,
        height: 72.0,
    };
    let pdf = images_to_pdf(&[img], &dir.path().join("strip.pdf"), Some(strip), &idle()).unwrap();
    let before = names(dir.path());
    // 14400 pt at 330 dpi is 66000 px wide (330 px high: within the budget).
    let m = invalid(to_images(&pdf, 330, ImageFormat::Jpeg, None));
    let want = "page 1: 66000x330 px is over JPEG's 65535 px side at 330 dpi; lower the dpi";
    assert!(m == want || m == want.replace("66000x", "66001x"), "{m}");
    assert_eq!(names(dir.path()), before);
}

#[test]
fn a_document_without_pages_is_refused() {
    require_qpdf!();
    let (dir, _) = setup();
    let empty = dir.path().join("empty.pdf");
    common::docs::qpdf(&["--empty", empty.to_str().unwrap()]);
    let m = invalid(to_images(&empty, 72, ImageFormat::Png, None));
    assert_eq!(m, "document has no pages");
}
