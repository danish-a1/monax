//! Signature placement: image or text fitted in a rect, on one page only,
//! upright where the user sees it.
mod common;
use common::sign::{BLUE, CLEAR, RED, at, halves, jpeg, near, pixel, png};
use common::{copy_fixture, diff_box, setup};
use monax_core::forms::{sign_image, sign_text};
use monax_core::pdfium;

/// Asserts `c` is `want` within 40 per channel (JPEG and anti-aliasing).
fn color(c: [u8; 4], want: [u8; 4]) {
    assert!(
        c.iter().zip(want).all(|(a, b)| a.abs_diff(b) <= 40),
        "{c:?} vs {want:?}"
    );
}

#[test]
fn image_lands_in_the_rect_on_the_given_page_only() {
    require_pdfium!();
    let (dir, three) = setup();
    let img = png(dir.path(), "s.png", (100, 40), |_, _| RED);
    let out = sign_image(
        &three,
        &dir.path().join("o.pdf"),
        &at(1, (60.0, 20.0, 160.0, 60.0)),
        &img,
    )
    .unwrap();
    near(diff_box(&three, &out, 1).unwrap(), (60, 140, 159, 179));
    assert_eq!(diff_box(&three, &out, 0), None, "page 0 changed");
    assert_eq!(diff_box(&three, &out, 2), None, "page 2 changed");
    color(pixel(&out, 1, (110, 160)), RED);
}

#[test]
fn image_keeps_its_aspect_centred_in_the_rect() {
    require_pdfium!();
    let (dir, three) = setup();
    let img = png(dir.path(), "sq.png", (50, 50), |_, _| RED);
    let out = sign_image(
        &three,
        &dir.path().join("o.pdf"),
        &at(0, (60.0, 20.0, 160.0, 60.0)),
        &img,
    )
    .unwrap();
    near(diff_box(&three, &out, 0).unwrap(), (90, 140, 129, 179));
}

#[test]
fn image_is_upright_as_displayed_on_a_rotated_cropped_page() {
    require_pdfium!();
    // /Rotate 90, CropBox [50 20 250 120]: user rect (150 40 230 80) shows
    // at displayed x 20..60, y 100..180 (pixels, top-left origin).
    let (dir, rot) = copy_fixture("rotated-crop.pdf", "rot.pdf");
    let img = halves(dir.path(), "h.png", (200, 100));
    let place = at(0, (150.0, 40.0, 230.0, 80.0));
    let out = sign_image(&rot, &dir.path().join("o.pdf"), &place, &img).unwrap();
    near(diff_box(&rot, &out, 0).unwrap(), (20, 130, 59, 149));
    color(pixel(&out, 0, (25, 140)), RED);
    color(pixel(&out, 0, (55, 140)), BLUE);
    // Turned 90 degrees clockwise: the left (red) half is on top.
    let turned = monax_core::forms::Placement {
        rotate: 90,
        ..place
    };
    let out = sign_image(&rot, &dir.path().join("t.pdf"), &turned, &img).unwrap();
    near(diff_box(&rot, &out, 0).unwrap(), (20, 100, 59, 179));
    color(pixel(&out, 0, (40, 110)), RED);
    color(pixel(&out, 0, (40, 170)), BLUE);
}

#[test]
fn png_alpha_and_opacity_show_the_page_through() {
    require_pdfium!();
    let (dir, three) = setup();
    let img = png(dir.path(), "a.png", (100, 40), |x, _| {
        if x < 50 { CLEAR } else { RED }
    });
    let out = sign_image(
        &three,
        &dir.path().join("a.pdf"),
        &at(1, (60.0, 20.0, 160.0, 60.0)),
        &img,
    )
    .unwrap();
    near(diff_box(&three, &out, 1).unwrap(), (110, 140, 159, 179));
    let solid = png(dir.path(), "s.png", (100, 40), |_, _| RED);
    let half = monax_core::forms::Placement {
        opacity: 0.5,
        ..at(1, (60.0, 20.0, 160.0, 60.0))
    };
    let out = sign_image(&three, &dir.path().join("h.pdf"), &half, &solid).unwrap();
    color(pixel(&out, 1, (110, 160)), [255, 128, 128, 255]);
}

#[test]
fn jpeg_is_embedded_as_is() {
    require_pdfium!();
    let (dir, three) = setup();
    let img = jpeg(dir.path(), "s.jpg", (100, 40));
    let out = sign_image(
        &three,
        &dir.path().join("o.pdf"),
        &at(2, (60.0, 20.0, 160.0, 60.0)),
        &img,
    )
    .unwrap();
    near(diff_box(&three, &out, 2).unwrap(), (60, 140, 159, 179));
    color(pixel(&out, 2, (110, 160)), RED);
    let bytes = std::fs::read(&out).unwrap();
    assert!(
        bytes.windows(10).any(|w| w == b"/DCTDecode"),
        "JPEG was re-encoded"
    );
    let half = monax_core::forms::Placement {
        opacity: 0.5,
        ..at(2, (60.0, 20.0, 160.0, 60.0))
    };
    let out = sign_image(&three, &dir.path().join("h.pdf"), &half, &img).unwrap();
    color(pixel(&out, 2, (110, 160)), [255, 128, 128, 255]);
}

#[test]
fn text_is_fitted_centred_and_searchable() {
    require_pdfium!();
    let (dir, three) = setup();
    let rect = (20.0, 20.0, 180.0, 60.0); // pixels x 20..180, y 140..180
    for (i, size) in [(0usize, 18.0f32), (2, 300.0)] {
        let out = sign_text(
            &three,
            &dir.path().join(format!("t{i}.pdf")),
            &at(i, rect),
            "Ann Signer",
            size,
        )
        .unwrap();
        let (x0, y0, x1, y1) = diff_box(&three, &out, i).unwrap();
        // 1 px of anti-aliasing where the ink meets the rect edge.
        assert!(
            x0 >= 19 && y0 >= 139 && x1 <= 180 && y1 <= 180,
            "overflow: {x0},{y0} {x1},{y1}"
        );
        assert!(
            ((x0 + x1) as f32 / 2.0 - 100.0).abs() <= 2.0,
            "not centred: {x0}..{x1}"
        );
        assert!(x1 - x0 > y1 - y0, "not upright");
        let t = pdfium::extract_text(&out, i..=i).unwrap();
        assert!(t.contains("Ann Signer"), "{t:?}");
        assert_eq!(diff_box(&three, &out, 1), None, "page 1 changed");
    }
    // 18 pt fits as given: about 85 pt wide; 300 pt shrinks to the rect width.
    let small = sign_text(
        &three,
        &dir.path().join("s.pdf"),
        &at(0, rect),
        "Ann Signer",
        18.0,
    )
    .unwrap();
    let (x0, _, x1, _) = diff_box(&three, &small, 0).unwrap();
    assert!(
        (70..100).contains(&(x1 - x0)),
        "18 pt grew or shrank: {}",
        x1 - x0
    );
}

#[test]
fn text_is_upright_on_a_rotated_cropped_page() {
    require_pdfium!();
    let (dir, rot) = copy_fixture("rotated-crop.pdf", "rot.pdf");
    let out = sign_text(
        &rot,
        &dir.path().join("o.pdf"),
        &at(0, (150.0, 40.0, 230.0, 80.0)),
        "Ann",
        20.0,
    )
    .unwrap();
    let (x0, y0, x1, y1) = diff_box(&rot, &out, 0).unwrap();
    assert!(
        x0 >= 20 && y0 >= 100 && x1 < 60 && y1 < 180,
        "outside: {x0},{y0} {x1},{y1}"
    );
    assert!(x1 - x0 > y1 - y0, "not upright as displayed");
}
