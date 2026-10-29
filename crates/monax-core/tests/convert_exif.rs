//! Images to PDF and signature images honour JPEG EXIF Orientation (R143):
//! the image takes its oriented size and shows upright; broken EXIF reads
//! as upright.
mod common;
use common::convert::{exif, quadrant_jpeg};
use common::sign::{at, pixel};
use monax_core::convert::images_to_pdf;
use monax_core::forms::sign_image;
use monax_core::pdfium::displayed_size;
use std::path::Path;

/// Which displayed quadrant of page 0 of `pdf` (W x H points, 72 dpi probes
/// at the quadrant centres) is red: "TL", "TR", "BL" or "BR".
fn red_quadrant(pdf: &Path) -> &'static str {
    let (w, h) = displayed_size(pdf, 0).unwrap();
    let (w, h) = (w as u32, h as u32);
    let at = [("TL", (w / 4, h / 4)), ("TR", (3 * w / 4, h / 4))];
    let at = at
        .into_iter()
        .chain([("BL", (w / 4, 3 * h / 4)), ("BR", (3 * w / 4, 3 * h / 4))]);
    let red: Vec<&str> = at
        .filter(|&(_, xy)| {
            let p = pixel(pdf, 0, xy);
            p[0] > 200 && p[2] < 60
        })
        .map(|(n, _)| n)
        .collect();
    assert_eq!(red.len(), 1, "red at {red:?}");
    red[0]
}

#[test]
fn every_orientation_shows_upright_at_its_oriented_size() {
    require_qpdf!();
    let d = tempfile::tempdir().unwrap();
    // Where the stored top-left quadrant shows, and the page size.
    let want = [
        (1, "TL", (32.0, 16.0)),
        (2, "TR", (32.0, 16.0)),
        (3, "BR", (32.0, 16.0)),
        (4, "BL", (32.0, 16.0)),
        (5, "TL", (16.0, 32.0)),
        (6, "TR", (16.0, 32.0)),
        (7, "BR", (16.0, 32.0)),
        (8, "BL", (16.0, 32.0)),
    ];
    for (o, quad, size) in want {
        for le in [true, false] {
            let img = quadrant_jpeg(d.path(), &format!("{o}{le}.jpg"), exif(o, le));
            let out = d.path().join(format!("{o}{le}.pdf"));
            let out = images_to_pdf(&[img], &out, None, &common::convert::idle()).unwrap();
            assert_eq!(displayed_size(&out, 0).unwrap(), size, "orientation {o}");
            assert_eq!(red_quadrant(&out), quad, "orientation {o} le {le}");
        }
    }
}

#[test]
fn a_rotated_jpeg_is_fitted_by_its_oriented_size() {
    require_qpdf!();
    let d = tempfile::tempdir().unwrap();
    let img = quadrant_jpeg(d.path(), "six.jpg", exif(6, false));
    let size = monax_core::convert::PageSize {
        width: 100.0,
        height: 200.0,
    };
    let out = d.path().join("o.pdf");
    let out = images_to_pdf(&[img], &out, Some(size), &common::convert::idle()).unwrap();
    // 16 x 32 fills 100 x 200 exactly: red top-right, no white margin.
    assert_eq!(red_quadrant(&out), "TR");
    assert!(
        pixel(&out, 0, (2, 198))[2] > 200,
        "blue to the bottom-left corner"
    );
}

#[test]
fn truncated_or_bogus_exif_reads_as_upright() {
    require_qpdf!();
    let d = tempfile::tempdir().unwrap();
    // Cut anywhere before the Orientation value ends (payload byte 26).
    let full = exif(6, true);
    let mut cases: Vec<Vec<u8>> = (0..26).step_by(3).map(|n| full[..n].to_vec()).collect();
    cases.push(exif(9, true)); // out of range
    let mut bad_offset = exif(6, false);
    bad_offset[13] = 0xF0; // IFD0 offset far past the end
    cases.push(bad_offset);
    for (k, app1) in cases.into_iter().enumerate() {
        let img = quadrant_jpeg(d.path(), &format!("{k}.jpg"), app1);
        let out = d.path().join(format!("{k}.pdf"));
        let out = images_to_pdf(&[img], &out, None, &common::convert::idle()).unwrap();
        assert_eq!(displayed_size(&out, 0).unwrap(), (32.0, 16.0), "case {k}");
        assert_eq!(red_quadrant(&out), "TL", "case {k}");
    }
}

#[test]
fn a_rotated_signature_jpeg_is_placed_upright() {
    require_pdfium!();
    let (dir, three) = common::setup();
    let img = quadrant_jpeg(dir.path(), "six.jpg", exif(6, true));
    let out = dir.path().join("o.pdf");
    // Shown 16 x 32, fitted by height in the wide 100 x 40 rect: displayed
    // x 100..120, y 140..180 (page 200 pt high), stored top-left at top-right.
    let out = sign_image(&three, &out, &at(0, (60.0, 20.0, 160.0, 60.0)), &img).unwrap();
    let red = |xy| {
        let p = pixel(&out, 0, xy);
        p[0] > 200 && p[2] < 60
    };
    let quads = [(105, 150), (115, 150), (105, 170), (115, 170)].map(red);
    assert_eq!(quads, [false, true, false, false], "TL TR BL BR");
}
