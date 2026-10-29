//! Signature placement refusals: bad placements, images and text, with
//! nothing left behind.
mod common;
use common::sign::{RED, at, jpeg, png};
use common::{copy_fixture, names, setup};
use monax_core::forms::{Placement, sign_image, sign_text};
use monax_core::pages::PagesError;
use std::path::Path;

const RECT: (f32, f32, f32, f32) = (60.0, 20.0, 160.0, 60.0);

fn invalid(r: Result<std::path::PathBuf, PagesError>) -> String {
    match r {
        Err(PagesError::InvalidInput(m)) => m,
        other => panic!("expected InvalidInput, got {other:?}"),
    }
}

#[test]
fn bad_placements_are_refused_and_write_nothing() {
    require_pdfium!();
    let (dir, three) = setup();
    let img = png(dir.path(), "s.png", (10, 10), |_, _| RED);
    let out = dir.path().join("o.pdf");
    let try_at = |p: Placement| sign_image(&three, &out, &p, &img);
    let r = try_at(at(3, RECT));
    assert!(
        matches!(r, Err(PagesError::PageOutOfRange { page: 3, count: 3 })),
        "{r:?}"
    );
    assert_eq!(
        invalid(try_at(at(0, (60.0, 20.0, 60.0, 60.0)))),
        "signature rect is empty"
    );
    assert_eq!(
        invalid(try_at(at(0, (60.0, f32::NAN, 70.0, 60.0)))),
        "signature rect is empty"
    );
    assert_eq!(
        invalid(try_at(at(0, (150.0, 20.0, 210.0, 60.0)))),
        "signature rect is outside page 0"
    );
    let turned = Placement {
        rotate: 45,
        ..at(0, RECT)
    };
    assert_eq!(
        invalid(try_at(turned)),
        "rotate must be 0, 90, 180 or 270 degrees, got 45"
    );
    let faint = Placement {
        opacity: 1.5,
        ..at(0, RECT)
    };
    assert_eq!(invalid(try_at(faint)), "opacity must be in 0..=1, got 1.5");
    assert_eq!(names(dir.path()), ["s.png", "three.pdf"]);
}

#[test]
fn rect_must_lie_inside_the_visible_crop_box() {
    require_pdfium!();
    let (dir, rot) = copy_fixture("rotated-crop.pdf", "rot.pdf");
    let out = dir.path().join("o.pdf");
    let m = invalid(sign_text(
        &rot,
        &out,
        &at(0, (10.0, 10.0, 60.0, 40.0)),
        "A",
        10.0,
    ));
    assert_eq!(m, "signature rect is outside page 0");
    assert!(sign_text(&rot, &out, &at(0, (50.0, 20.0, 250.0, 120.0)), "A", 10.0).is_ok());
}

#[test]
fn bad_text_is_refused() {
    require_pdfium!();
    let (dir, three) = setup();
    let out = dir.path().join("o.pdf");
    let t = |text: &str, size: f32, rect| sign_text(&three, &out, &at(0, rect), text, size);
    assert_eq!(invalid(t("", 12.0, RECT)), "signature text is empty");
    assert_eq!(invalid(t("   ", 12.0, RECT)), "signature text is empty");
    assert_eq!(
        invalid(t("A", 0.0, RECT)),
        "font size must be a positive number of points, got 0"
    );
    assert!(invalid(t("\u{3b1}", 12.0, RECT)).contains("U+03B1"));
    let tiny = (60.0, 20.0, 160.0, 22.0);
    assert_eq!(
        invalid(t("Ann", 12.0, tiny)),
        "signature text does not fit in the rect even at 4 pt"
    );
    assert_eq!(names(dir.path()), ["three.pdf"]);
    assert!(t("Ann", 3.0, RECT).is_ok(), "3 pt fits as given");
}

#[test]
fn bad_images_are_refused() {
    require_pdfium!();
    let (dir, three) = setup();
    let out = dir.path().join("o.pdf");
    let try_img = |p: &Path| sign_image(&three, &out, &at(0, RECT), p);
    assert_eq!(
        invalid(try_img(Path::new("rel.png"))),
        "path must be absolute: rel.png"
    );
    let gone = dir.path().join("gone.png");
    assert!(invalid(try_img(&gone)).starts_with("file does not exist"));
    let txt = dir.path().join("a.png");
    std::fs::write(&txt, b"not an image").unwrap();
    assert_eq!(
        invalid(try_img(&txt)),
        "signature image is not a PNG or JPEG"
    );
    // A PNG whose header claims 10000 x 10000 pixels.
    let big = png(dir.path(), "big.png", (8, 8), |_, _| RED);
    let mut b = std::fs::read(&big).unwrap();
    b[16..24].copy_from_slice(&[0, 0, 0x27, 0x10, 0, 0, 0x27, 0x10]);
    std::fs::write(&big, &b).unwrap();
    let m = invalid(try_img(&big));
    assert_eq!(
        m,
        "signature image is 10000x10000 px, over the 64000000 pixel budget"
    );
    // A JPEG whose frame header claims 65000 x 65000 pixels.
    let huge = jpeg(dir.path(), "huge.jpg", (8, 8));
    let mut j = std::fs::read(&huge).unwrap();
    let sof = j.windows(2).position(|w| w == [0xFF, 0xC0]).unwrap();
    j[sof + 5..sof + 9].copy_from_slice(&[0xFD, 0xE8, 0xFD, 0xE8]);
    std::fs::write(&huge, &j).unwrap();
    let m = invalid(try_img(&huge));
    assert_eq!(
        m,
        "signature image is 65000x65000 px, over the 64000000 pixel budget"
    );
    let broken = png(dir.path(), "broken.png", (8, 8), |_, _| RED);
    let cut = std::fs::read(&broken).unwrap()[..40].to_vec();
    std::fs::write(&broken, cut).unwrap();
    assert!(invalid(try_img(&broken)).starts_with("signature image is not a valid PNG"));
    assert!(!out.exists());
}

#[test]
fn unreadable_jpegs_and_huge_files_are_refused() {
    require_pdfium!();
    let (dir, three) = setup();
    let out = dir.path().join("o.pdf");
    let try_img = |p: &Path| sign_image(&three, &out, &at(0, RECT), p);
    const NOT_JPEG: &str = "signature image is not a valid JPEG";
    // Frame header claiming 65535 x 65535 pixels, no tables, no scan.
    let mut sof = vec![0xFF, 0xD8, 0xFF, 0xC0, 0, 17, 8, 0xFF, 0xFF, 0xFF, 0xFF, 3];
    sof.extend([1, 0x11, 0, 2, 0x11, 0, 3, 0x11, 0, 0xFF, 0xD9]);
    assert_eq!(sof.len(), 23);
    let bare = dir.path().join("bare.jpg");
    std::fs::write(&bare, &sof).unwrap();
    assert_eq!(invalid(try_img(&bare)), NOT_JPEG);
    // A real 10 x 10 JPEG cut before its scan.
    let real = std::fs::read(jpeg(dir.path(), "real.jpg", (10, 10))).unwrap();
    let sos = real.windows(2).position(|w| w == [0xFF, 0xDA]).unwrap();
    let cut = dir.path().join("cut.jpg");
    std::fs::write(&cut, [&real[..sos], &[0xFF, 0xD9][..]].concat()).unwrap();
    assert_eq!(invalid(try_img(&cut)), NOT_JPEG);
    // Over 64 MiB on disk (sparse): refused before it is read.
    let big = dir.path().join("big.png");
    std::fs::File::create(&big)
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    assert_eq!(
        invalid(try_img(&big)),
        "signature image is 67108865 bytes, over the 67108864 byte limit"
    );
    assert!(!out.exists());
}
