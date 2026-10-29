//! Signature images written on the fly (no binary fixtures) and the
//! placement every sign test starts from.
use monax_core::forms::Placement;
use monax_core::pages::Rect;
use monax_core::pdfium;
use std::path::{Path, PathBuf};

pub const RED: [u8; 4] = [255, 0, 0, 255];
pub const BLUE: [u8; 4] = [0, 0, 255, 255];
pub const CLEAR: [u8; 4] = [0, 0, 0, 0];

/// An 8-bit RGBA PNG `dir/name` of `w` x `h` pixels colored by `px(x, y)`.
pub fn png(
    dir: &Path,
    name: &str,
    (w, h): (u32, u32),
    px: impl Fn(u32, u32) -> [u8; 4],
) -> PathBuf {
    let p = dir.join(name);
    let mut e = png::Encoder::new(std::fs::File::create(&p).unwrap(), w, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    let data: Vec<u8> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .flat_map(|(x, y)| px(x, y))
        .collect();
    e.write_header().unwrap().write_image_data(&data).unwrap();
    p
}

/// Left half red, right half blue: shows which way the image turned.
pub fn halves(dir: &Path, name: &str, size: (u32, u32)) -> PathBuf {
    png(
        dir,
        name,
        size,
        |x, _| if x < size.0 / 2 { RED } else { BLUE },
    )
}

/// A solid red baseline JPEG `dir/name`.
pub fn jpeg(dir: &Path, name: &str, (w, h): (u16, u16)) -> PathBuf {
    let p = dir.join(name);
    let px: Vec<u8> = (0..usize::from(w) * usize::from(h))
        .flat_map(|_| [255, 0, 0])
        .collect();
    let mut bytes = Vec::new();
    let enc = jpeg_encoder::Encoder::new(&mut bytes, 90);
    enc.encode(&px, w, h, jpeg_encoder::ColorType::Rgb).unwrap();
    std::fs::write(&p, bytes).unwrap();
    p
}

/// Placement on 0-based `page`, unturned and opaque.
pub fn at(page: usize, (left, bottom, right, top): (f32, f32, f32, f32)) -> Placement {
    Placement {
        page,
        rect: Rect {
            left,
            bottom,
            right,
            top,
        },
        rotate: 0,
        opacity: 1.0,
    }
}

/// RGBA of displayed pixel `(x, y)` (top-left origin) of `page` at 72 dpi.
pub fn pixel(p: &Path, page: usize, (x, y): (u32, u32)) -> [u8; 4] {
    let r = pdfium::render(p, page, 72).unwrap();
    let i = ((y * r.w + x) * 4) as usize;
    [r.rgba[i], r.rgba[i + 1], r.rgba[i + 2], r.rgba[i + 3]]
}

/// Asserts `got` is within 2 px of `want` on every side.
pub fn near(got: (u32, u32, u32, u32), want: (u32, u32, u32, u32)) {
    let d = [
        got.0.abs_diff(want.0),
        got.1.abs_diff(want.1),
        got.2.abs_diff(want.2),
        got.3.abs_diff(want.3),
    ];
    assert!(d.iter().all(|&v| v <= 2), "ink {got:?}, want {want:?}");
}
