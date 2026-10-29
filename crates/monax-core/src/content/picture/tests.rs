//! Unit tests of image reading: PNG decoding to BGRA and magic-byte, JPEG
//! structure and size checks (no pdfium).
use super::{Picture, read, read_capped};

/// How the signature tool names its image (kept in its messages).
const SIG: &str = "signature image";

/// An 8-bit `color` PNG of `data` in `dir`.
fn png_file(
    dir: &std::path::Path,
    color: png::ColorType,
    data: &[u8],
    (w, h): (u32, u32),
) -> std::path::PathBuf {
    let p = dir.join(format!("{color:?}.png"));
    let mut e = png::Encoder::new(std::fs::File::create(&p).unwrap(), w, h);
    e.set_color(color);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(data).unwrap();
    p
}

fn pixels(p: &std::path::Path) -> (i32, i32, Vec<u8>, bool) {
    match read(p, SIG).unwrap() {
        Picture::Png { w, h, px, alpha } => (w, h, px, alpha),
        Picture::Jpeg { .. } => panic!("read a PNG as JPEG"),
    }
}

#[test]
fn png_decodes_to_bgra_or_bgrx() {
    let d = tempfile::tempdir().unwrap();
    use png::ColorType::*;
    let rgba = png_file(d.path(), Rgba, &[1, 2, 3, 4, 5, 6, 7, 8], (2, 1));
    assert_eq!(pixels(&rgba), (2, 1, vec![3, 2, 1, 4, 7, 6, 5, 8], true));
    let rgb = png_file(d.path(), Rgb, &[1, 2, 3], (1, 1));
    assert_eq!(pixels(&rgb), (1, 1, vec![3, 2, 1, 255], false));
    let gray = png_file(d.path(), Grayscale, &[9], (1, 1));
    assert_eq!(pixels(&gray), (1, 1, vec![9, 9, 9, 255], false));
    let ga = png_file(d.path(), GrayscaleAlpha, &[9, 0], (1, 1));
    assert_eq!(pixels(&ga), (1, 1, vec![9, 9, 9, 0], true));
    // Palette with a tRNS entry: index 0 clear red, index 1 opaque blue.
    let p = d.path().join("palette.png");
    let mut e = png::Encoder::new(std::fs::File::create(&p).unwrap(), 2, 1);
    e.set_color(Indexed);
    e.set_depth(png::BitDepth::Eight);
    e.set_palette(vec![255, 0, 0, 0, 0, 255]);
    e.set_trns(vec![0]);
    e.write_header().unwrap().write_image_data(&[0, 1]).unwrap();
    assert_eq!(pixels(&p), (2, 1, vec![0, 0, 255, 0, 255, 0, 0, 255], true));
}

#[test]
fn read_sniffs_magic_bytes_not_names() {
    let d = tempfile::tempdir().unwrap();
    // A real 2 x 1 JPEG passes as is (named .png: the bytes decide).
    let px = [255, 0, 0, 0, 0, 255];
    let mut j = Vec::new();
    jpeg_encoder::Encoder::new(&mut j, 90)
        .encode(&px, 2, 1, jpeg_encoder::ColorType::Rgb)
        .unwrap();
    let jpg = d.path().join("sig.png");
    std::fs::write(&jpg, &j).unwrap();
    assert!(matches!(read(&jpg, SIG), Ok(Picture::Jpeg { bytes, .. }) if bytes == j));
    let not_jpeg = |k: &[u8], case: &str| {
        std::fs::write(&jpg, k).unwrap();
        let r = read(&jpg, SIG);
        let e = r.err().unwrap_or_else(|| panic!("{case}: accepted"));
        assert_eq!(
            e.to_string(),
            "invalid input: signature image is not a valid JPEG",
            "{case}"
        );
    };
    // SOI, DQT, SOF0 2 x 1, SOS, two scan bytes, EOI.
    let dqt = [&[0xFF, 0xDB, 0, 67, 0][..], &[1; 64]].concat();
    let sof = [0xFF, 0xC0, 0, 11, 8, 0, 1, 0, 2, 1, 1, 0x11, 0];
    let sos = [0xFF, 0xDA, 0, 8, 1, 1, 0, 0, 0x3F, 0];
    let jpeg = |parts: &[&[u8]]| [&[0xFF, 0xD8][..], &parts.concat()].concat();
    let ok = jpeg(&[&dqt, &sof, &sos, &[0x12, 0x34, 0xFF, 0xD9]]);
    std::fs::write(&jpg, &ok).unwrap();
    assert!(read(&jpg, SIG).is_ok(), "tables, frame and scan data");
    let mut zero_w = sof;
    zero_w[8] = 0;
    not_jpeg(
        &jpeg(&[&dqt, &zero_w, &sos, &[0x12, 0xFF, 0xD9]]),
        "zero width",
    );
    not_jpeg(&jpeg(&[&dqt, &sof, &[0xFF, 0xD9]]), "no scan");
    not_jpeg(&jpeg(&[&sof, &sos, &[0xFF, 0xD9]]), "structure only");
    not_jpeg(&jpeg(&[&sof, &sos, &[0x12, 0xFF, 0xD9]]), "no DQT");
    not_jpeg(&jpeg(&[&dqt, &sof, &sos, &[0xFF, 0xD9]]), "no scan data");
    not_jpeg(&jpeg(&[&dqt, &sof, &sos, &[0x12]]), "scan data cut short");
    let gif = d.path().join("sig.gif");
    std::fs::write(&gif, b"GIF89a").unwrap();
    let e = read(&gif, SIG).err().unwrap().to_string();
    assert_eq!(e, "invalid input: signature image is not a PNG or JPEG");
    let stub = d.path().join("stub.png");
    std::fs::write(&stub, b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR").unwrap();
    let e = read(&stub, SIG).err().unwrap().to_string();
    assert_eq!(
        e,
        "invalid input: signature image is not a valid PNG: truncated"
    );
    assert!(read(d.path(), SIG).is_err(), "a directory is not an image");
}

#[test]
fn reading_stops_past_the_cap() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("grown.png");
    std::fs::write(&p, [7; 11]).unwrap();
    assert_eq!(read_capped(&p, 11, SIG).unwrap(), [7; 11]);
    let e = read_capped(&p, 10, SIG).err().unwrap().to_string();
    assert_eq!(
        e,
        "invalid input: signature image is over the 10 byte limit"
    );
}
