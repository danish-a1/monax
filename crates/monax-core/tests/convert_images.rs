//! Images to PDF: one page per PNG or JPEG in order, at the image's size or
//! fitted and centred on a fixed page; bad images are refused naming the
//! file, and a failed conversion leaves no file.
mod common;
use common::convert::idle;
use common::names;
use common::sign::{BLUE, RED, jpeg, pixel, png};
use monax_core::convert::{ImagesToPdfInput, PageSize, images_to_pdf};
use monax_core::pages::PagesError;
use monax_core::pdfium::{displayed_size, page_count};
use serde_json::json;
use std::path::{Path, PathBuf};

const WHITE: [u8; 4] = [255, 255, 255, 255];
const A4: PageSize = PageSize {
    width: 595.28,
    height: 841.89,
};

fn invalid(r: Result<PathBuf, PagesError>) -> String {
    match r {
        Err(PagesError::InvalidInput(m)) => m,
        other => panic!("expected InvalidInput, got {other:?}"),
    }
}

fn near(got: (f32, f32), want: (f32, f32)) {
    let ok = (got.0 - want.0).abs() < 0.01 && (got.1 - want.1).abs() < 0.01;
    assert!(ok, "page {got:?}, want {want:?}");
}

/// True when `rgba` is within 8 of `want` per channel (JPEG is lossy).
fn like(rgba: [u8; 4], want: [u8; 4]) -> bool {
    rgba.iter().zip(want).all(|(a, b)| a.abs_diff(b) <= 8)
}

#[test]
fn two_pngs_become_a_two_page_pdf_of_the_requested_size() {
    require_qpdf!();
    let d = tempfile::tempdir().unwrap();
    let a = png(d.path(), "a.png", (40, 20), |_, _| RED);
    let b = png(d.path(), "b.png", (10, 30), |_, _| BLUE);
    let out = d.path().join("o.pdf");
    assert_eq!(
        images_to_pdf(&[a, b], &out, Some(A4), &idle()).unwrap(),
        out
    );
    assert_eq!(page_count(&out).unwrap(), 2);
    common::docs::assert_clean(&out);
    for i in 0..2 {
        near(displayed_size(&out, i).unwrap(), (595.28, 841.89));
    }
    // 40x20 fitted to the width (595 x 298), centred: white above and below.
    assert!(like(pixel(&out, 0, (297, 421)), RED));
    assert!(like(pixel(&out, 0, (297, 20)), WHITE));
    assert!(like(pixel(&out, 0, (297, 820)), WHITE));
    // 10x30 fitted to the height (281 x 842), centred: white left and right.
    assert!(like(pixel(&out, 1, (297, 421)), BLUE));
    assert!(like(pixel(&out, 1, (20, 421)), WHITE));
    assert!(like(pixel(&out, 1, (575, 421)), WHITE));
}

#[test]
fn without_a_page_size_each_page_is_its_image_at_one_point_per_pixel() {
    require_qpdf!();
    let d = tempfile::tempdir().unwrap();
    let a = jpeg(d.path(), "a.jpg", (16, 8));
    let b = png(d.path(), "b.png", (30, 20), |_, _| BLUE);
    let run = || {
        let v = json!({ "inputs": [a, b] });
        serde_json::from_value::<ImagesToPdfInput>(v)
            .unwrap()
            .run(&idle())
    };
    let out = run().unwrap();
    assert_eq!(out, d.path().join("a-images.pdf"));
    near(displayed_size(&out, 0).unwrap(), (16.0, 8.0));
    near(displayed_size(&out, 1).unwrap(), (30.0, 20.0));
    assert!(like(pixel(&out, 0, (8, 4)), RED));
    assert!(like(pixel(&out, 1, (15, 10)), BLUE));
    // Same images again: a new file beside the first, which is untouched.
    let before = std::fs::read(&out).unwrap();
    assert_eq!(run().unwrap(), d.path().join("a-images-1.pdf"));
    assert_eq!(std::fs::read(&out).unwrap(), before);
    assert_eq!(
        names(d.path()),
        ["a-images-1.pdf", "a-images.pdf", "a.jpg", "b.png"]
    );
}

#[test]
fn bad_images_are_refused_naming_the_file_without_output() {
    require_qpdf!();
    let d = tempfile::tempdir().unwrap();
    let good = png(d.path(), "good.png", (4, 4), |_, _| RED);
    let gif = d.path().join("x.gif");
    std::fs::write(&gif, b"GIF89a").unwrap();
    let out = d.path().join("o.pdf");
    let run = |inputs: &[&Path], size| {
        let v: Vec<PathBuf> = inputs.iter().map(|p| p.to_path_buf()).collect();
        images_to_pdf(&v, &out, size, &idle())
    };
    assert_eq!(invalid(run(&[], None)), "no images given");
    let many = vec![good.as_path(); 501];
    assert_eq!(invalid(run(&many, None)), "501 images given, at most 500");
    // The second image is bad: nothing is written.
    let m = invalid(run(&[&good, &gif], None));
    assert_eq!(m, format!("image {} is not a PNG or JPEG", gif.display()));
    let m = invalid(run(&[Path::new("rel.png")], None));
    assert_eq!(m, "path must be absolute: rel.png");
    // A PNG whose header claims 10000 x 10000 pixels.
    let big = png(d.path(), "big.png", (8, 8), |_, _| RED);
    let mut b = std::fs::read(&big).unwrap();
    b[16..24].copy_from_slice(&[0, 0, 0x27, 0x10, 0, 0, 0x27, 0x10]);
    std::fs::write(&big, &b).unwrap();
    let m = invalid(run(&[&big], None));
    let want = "is 10000x10000 px, over the 64000000 pixel budget";
    assert_eq!(m, format!("image {} {want}", big.display()));
    // At 1 pt per pixel a 14401 px wide image is past the largest page.
    let wide = png(d.path(), "wide.png", (14401, 1), |_, _| RED);
    let m = invalid(run(&[&wide], None));
    let want = "is 14401x1 px: as a page at 1 pt per pixel it is over 14400 pt; choose a page size";
    assert_eq!(m, format!("image {} {want}", wide.display()));
    for (w, h, bad) in [(71.0, 100.0, "71"), (100.0, 14400.5, "14400.5")] {
        let size = Some(PageSize {
            width: w,
            height: h,
        });
        let m = invalid(run(&[&good], size));
        assert_eq!(m, format!("page size {bad} pt not in 72..=14400"));
    }
    let nan = Some(PageSize {
        width: f32::NAN,
        height: 100.0,
    });
    assert_eq!(
        invalid(run(&[&good], nan)),
        "page size NaN pt not in 72..=14400"
    );
    // The output may not replace an input image.
    let m = invalid(images_to_pdf(
        std::slice::from_ref(&good),
        &good,
        None,
        &idle(),
    ));
    assert!(m.starts_with("output would overwrite an input"), "{m}");
    assert_eq!(
        names(d.path()),
        ["big.png", "good.png", "wide.png", "x.gif"]
    );
}
