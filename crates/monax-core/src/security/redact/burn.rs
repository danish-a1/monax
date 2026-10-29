//! The raster half of redaction: each page rendered at 300 dpi as the user
//! sees it, its rects burned in black, then drawn as the only content of a
//! new unrotated page of the displayed size.
use crate::content::encode_jpeg;
use crate::pdfium::{
    Box4, Checked, PdfiumError, Raster, geometry, lib, page_of, px_size, render_page, save,
    with_checked,
};
use pdfium_render::prelude::{
    PdfPageImageObject, PdfPageObjectsCommon, PdfPagePaperSize, PdfPoints,
};
use std::io::Cursor;

const DPI: u32 = 300;
/// JPEG quality of the raster. The rects are black before encoding, so no
/// artifact can carry what they covered.
const QUALITY: u8 = 90;
/// Largest JPEG side, pixels.
const JPEG_MAX: i32 = 65_535;

/// `r` clipped to `visible`; `None` when nothing of it is left.
pub(super) fn clip(r: Box4, visible: Box4) -> Option<Box4> {
    let c = (
        r.0.max(visible.0),
        r.1.max(visible.1),
        r.2.min(visible.2),
        r.3.min(visible.3),
    );
    (c.0 < c.2 && c.1 < c.3).then_some(c)
}

/// Pixel box `(x0, y0, x1, y1)` (top-left origin, ends exclusive) covering
/// user-space `r` (inside `visible`) on a page turned `rotation` degrees
/// clockwise, rendered at `(w, h)` pixels as displayed. Rounds outward.
pub(super) fn to_pixels(r: Box4, visible: Box4, rotation: u16, (w, h): (u32, u32)) -> [u32; 4] {
    let (vl, vb, vr, vt) = visible;
    let (pw, ph) = (vr - vl, vt - vb);
    // Unrotated, top-left origin: u right, v down.
    let (u0, u1, v0, v1) = (r.0 - vl, r.2 - vl, vt - r.3, vt - r.1);
    let ((x0, x1, y0, y1), (dw, dh)) = match rotation {
        90 => ((ph - v1, ph - v0, u0, u1), (ph, pw)),
        180 => ((pw - u1, pw - u0, ph - v1, ph - v0), (pw, ph)),
        270 => ((v0, v1, pw - u1, pw - u0), (ph, pw)),
        _ => ((u0, u1, v0, v1), (pw, ph)),
    };
    let (sx, sy) = (w as f32 / dw, h as f32 / dh);
    let px = |v: f32, s: f32, max: u32, f: fn(f32) -> f32| (f(v * s).max(0.0) as u32).min(max);
    [
        px(x0, sx, w, f32::floor),
        px(y0, sy, h, f32::floor),
        px(x1, sx, w, f32::ceil),
        px(y1, sy, h, f32::ceil),
    ]
}

/// Paints pixel box `[x0, y0, x1, y1)` of `r` opaque black.
fn fill(r: &mut Raster, [x0, y0, x1, y1]: [u32; 4]) {
    for y in y0..y1 {
        let row = ((y * r.w + x0) * 4) as usize..((y * r.w + x1) * 4) as usize;
        for px in r.rgba[row].as_chunks_mut::<4>().0 {
            *px = [0, 0, 0, 255];
        }
    }
}

/// Bytes of a new PDF with one page per entry of `pages` (0-based page of
/// `input`, user-space rects), in order: the page as displayed, rects
/// burned in, as a single JPEG image. Every rect and page size is checked
/// before any page is rendered; `ok`: the field check `input` passed, and
/// the file drawn ([`Checked::path`]: `input` itself, R119).
///
/// # Errors
/// `InvalidInput` (a rect outside its page's visible area, a page too large
/// to rasterize at 300 dpi), `PageOutOfRange`, `Damaged`, `EncryptedInput`,
/// `Library`.
pub(super) fn burn(pages: &[(usize, Vec<Box4>)], ok: &Checked) -> Result<Vec<u8>, PdfiumError> {
    with_checked(ok, None, |engine, doc| {
        let mut plan = Vec::with_capacity(pages.len());
        for (i, rects) in pages {
            let g = geometry(engine, doc, *i)?;
            let outside = || {
                PdfiumError::InvalidInput(format!("redaction rect on page {i} is outside the page"))
            };
            let clipped: Option<Vec<Box4>> = rects.iter().map(|&r| clip(r, g.visible)).collect();
            let (dw, dh) = g.displayed();
            let px = px_size(dw, dh, DPI)?;
            if px.0 > JPEG_MAX || px.1 > JPEG_MAX {
                return Err(PdfiumError::InvalidInput(format!(
                    "page {i} is too large to redact"
                )));
            }
            plan.push((*i, clipped.ok_or_else(outside)?, g, px));
        }
        let mut out = engine.create_new_pdf().map_err(lib)?;
        for (i, rects, g, (w, h)) in plan {
            let mut r = render_page(&page_of(doc, i)?, w, h, ok)?;
            let size = (r.w, r.h);
            for rect in rects {
                fill(&mut r, to_pixels(rect, g.visible, g.rotation, size));
            }
            let rgb = r.rgb();
            let jpeg = encode_jpeg(&rgb, size, 3, QUALITY)
                .ok_or_else(|| PdfiumError::Library(format!("JPEG encoding of page {i} failed")))?;
            let (dw, dh) = g.displayed();
            let paper = PdfPagePaperSize::from_points(PdfPoints::new(dw), PdfPoints::new(dh));
            let mut page = out.pages_mut().create_page_at_end(paper).map_err(lib)?;
            let mut img =
                PdfPageImageObject::new_from_jpeg_reader(&out, Cursor::new(jpeg)).map_err(lib)?;
            img.scale(dw, dh).map_err(lib)?;
            page.objects_mut().add_image_object(img).map_err(lib)?;
        }
        save(&out)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_keeps_the_visible_part_only() {
        let v = (50.0, 20.0, 250.0, 120.0);
        assert_eq!(
            clip((0.0, 0.0, 60.0, 30.0), v),
            Some((50.0, 20.0, 60.0, 30.0))
        );
        assert_eq!(clip((0.0, 0.0, 50.0, 30.0), v), None);
        assert_eq!(clip((260.0, 0.0, 300.0, 30.0), v), None);
    }

    #[test]
    fn pixels_follow_rotation_and_crop_offset() {
        // CropBox [50 20 250 120] (200 x 100), 1 px per point.
        let v = (50.0, 20.0, 250.0, 120.0);
        let r = (60.0, 30.0, 100.0, 50.0);
        assert_eq!(to_pixels(r, v, 0, (200, 100)), [10, 70, 50, 90]);
        assert_eq!(to_pixels(r, v, 90, (100, 200)), [10, 10, 30, 50]);
        assert_eq!(to_pixels(r, v, 180, (200, 100)), [150, 10, 190, 30]);
        assert_eq!(to_pixels(r, v, 270, (100, 200)), [70, 150, 90, 190]);
        // 300 dpi rounds outward: 10.5 pt .. 11 pt -> px 43 .. 46.
        let s = to_pixels((60.5, 108.0, 61.0, 120.0), v, 0, (834, 417));
        assert_eq!(s, [43, 0, 46, 51]);
    }

    #[test]
    fn fill_blacks_out_exactly_the_box() {
        let mut r = Raster {
            w: 3,
            h: 2,
            rgba: vec![255; 24],
        };
        fill(&mut r, [1, 1, 3, 2]);
        let black: Vec<bool> = r.rgba.chunks(4).map(|p| p == [0, 0, 0, 255]).collect();
        assert_eq!(black, [false, false, false, false, true, true]);
    }
}
