//! compress: dpi downsampling, JPEG quality, media and metadata removal.
mod common;
use common::docs::{assert_clean, attachments, image_sizes, info, with_attachment};
use common::guard::invalid;
use common::scan::{Img, images_pdf, rgb, scan_pdf};
use common::{copy_fixture, names};
use monax_core::content::{self, CompressOpts, Metadata};
use monax_core::pdfium;

fn opts(dpi: u32, jpeg_quality: u8) -> CompressOpts {
    CompressOpts {
        dpi,
        jpeg_quality,
        ..CompressOpts::default()
    }
}

fn len(p: &std::path::Path) -> u64 {
    std::fs::metadata(p).unwrap().len()
}

#[test]
fn a_300_dpi_scan_is_downsampled_to_72_dpi_and_shrinks() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    // 600 px over 144 pt (2 in) = 300 dpi; at 72 dpi it is 144 px.
    let scan = scan_pdf(dir.path(), 600, 144);
    let out = content::compress(&scan, &dir.path().join("c.pdf"), opts(72, 50)).unwrap();
    assert_eq!(image_sizes(&out), [(144, 144)]);
    assert_clean(&out);
    assert!(
        len(&out) * 20 < len(&scan),
        "{} vs {}",
        len(&out),
        len(&scan)
    );
    // Same picture: the gradient survives (mean channel error is small).
    let (a, b) = (
        pdfium::render(&scan, 0, 72).unwrap(),
        pdfium::render(&out, 0, 72).unwrap(),
    );
    let err: u64 = a
        .rgba
        .iter()
        .zip(&b.rgba)
        .map(|(x, y)| u64::from(x.abs_diff(*y)))
        .sum();
    assert!(
        err / (a.rgba.len() as u64) < 4,
        "mean error {}",
        err / a.rgba.len() as u64
    );
}

#[test]
fn images_at_or_below_the_dpi_keep_their_pixels_and_quality_sets_size() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let scan = scan_pdf(dir.path(), 300, 144); // 150 dpi
    let keep = content::compress(&scan, &dir.path().join("k.pdf"), opts(150, 50)).unwrap();
    assert_eq!(image_sizes(&keep), [(300, 300)]);
    let lo = content::compress(&scan, &dir.path().join("lo.pdf"), opts(100, 10)).unwrap();
    let hi = content::compress(&scan, &dir.path().join("hi.pdf"), opts(100, 95)).unwrap();
    assert_eq!(image_sizes(&lo), [(200, 200)]);
    assert!(len(&lo) < len(&hi), "{} vs {}", len(&lo), len(&hi));
}

#[test]
fn compress_is_idempotent() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let scan = scan_pdf(dir.path(), 400, 144);
    let a = content::compress(&scan, &dir.path().join("a.pdf"), opts(72, 60)).unwrap();
    let b = content::compress(&scan, &dir.path().join("b.pdf"), opts(72, 60)).unwrap();
    assert_eq!(len(&a), len(&b));
    assert_eq!(image_sizes(&a), image_sizes(&b));
}

#[test]
fn discard_media_and_strip_metadata_remove_what_they_name() {
    require_qpdf!();
    let (dir, three) = copy_fixture("three-pages.pdf", "three.pdf");
    let titled = Metadata {
        title: Some("T".into()),
        ..Metadata::default()
    };
    let t = content::set_metadata(&three, &dir.path().join("t.pdf"), titled).unwrap();
    let att = with_attachment(&t);
    let kept = content::compress(&att, &dir.path().join("k.pdf"), opts(150, 75)).unwrap();
    assert_eq!(attachments(&kept), ["note.txt"]);
    assert!(info(&kept).contains(&("/Title".into(), "T".into())));
    let all = CompressOpts {
        discard_media: true,
        strip_metadata: true,
        ..CompressOpts::default()
    };
    let gone = content::compress(&att, &dir.path().join("g.pdf"), all).unwrap();
    assert!(attachments(&gone).is_empty());
    assert!(
        !info(&gone).iter().any(|(k, _)| k == "/Title"),
        "{:?}",
        info(&gone)
    );
    assert_eq!(pdfium::page_count(&gone).unwrap(), 3);
}

#[test]
fn bad_options_are_invalid_and_write_nothing() {
    require_qpdf!();
    let (dir, three) = copy_fixture("three-pages.pdf", "three.pdf");
    let out = dir.path().join("o.pdf");
    for o in [opts(0, 50), opts(2401, 50), opts(72, 0), opts(72, 101)] {
        assert!(invalid(content::compress(&three, &out, o)), "{o:?}");
    }
    assert_eq!(names(dir.path()), ["three.pdf"]);
}

#[test]
fn an_indirect_image_length_does_not_stop_downsampling() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let p = images_pdf(dir.path(), "ind.pdf", &[rgb(600, 144)], true);
    let out = content::compress(&p, &dir.path().join("c.pdf"), opts(72, 50)).unwrap();
    assert_eq!(image_sizes(&out), [(144, 144)]);
    assert_clean(&out);
}

#[test]
fn images_left_above_the_dpi_are_noted_by_reason() {
    require_qpdf!();
    let dir = tempfile::tempdir().unwrap();
    let with = |dict, bpp| Img {
        dict,
        bpp,
        ..rgb(300, 72)
    };
    let cmyk = with("/ColorSpace /DeviceCMYK /BitsPerComponent 8", 4);
    let imgs = [
        rgb(300, 72),
        cmyk,
        cmyk,
        with("/ColorSpace /DeviceRGB /BitsPerComponent 16", 6),
        with(
            "/ColorSpace /DeviceRGB /BitsPerComponent 8 /Decode [1 0 1 0 1 0]",
            3,
        ),
        Img {
            smask: Some("/Matte [0 0 0]"),
            ..rgb(300, 72)
        },
        Img {
            smask: Some(""),
            ..rgb(300, 72)
        },
        with(
            "/ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /JPXDecode",
            3,
        ),
    ];
    let p = images_pdf(dir.path(), "mix.pdf", &imgs, false);
    let (out, notes) =
        content::compress_with_notes(&p, &dir.path().join("c.pdf"), opts(72, 50)).unwrap();
    let want = [
        "2 images left above 72 dpi: color space not gray or RGB",
        "1 image left above 72 dpi: not 8 bits per component",
        "1 image left above 72 dpi: has a /Decode array",
        "1 image left above 72 dpi: soft mask with /Matte or color-key mask",
        "1 image left above 72 dpi: compression qpdf cannot decode (JPX, JBIG2, CCITT)",
    ];
    assert_eq!(notes, want);
    // The plain image and the one with a plain soft mask are resampled; the
    // /Matte parent keeps its mask's size.
    let small = image_sizes(&out).iter().filter(|s| **s == (72, 72)).count();
    assert_eq!(small, 2, "{:?}", image_sizes(&out));
    assert_clean(&out);
    let quiet = content::compress_with_notes(&p, &dir.path().join("q.pdf"), opts(600, 50));
    assert_eq!(quiet.unwrap().1, Vec::<String>::new());
}
