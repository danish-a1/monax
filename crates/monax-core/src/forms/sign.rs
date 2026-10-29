//! Signature placement: a signature image or line of text stamped onto one
//! page through the overlay engine (`content::edit_pages`), fitted and
//! centred in a rect, upright as the page is displayed. Not a digital
//! (cryptographic) signature.
mod fit;

use crate::content::{check_opacity, check_size, check_text, edit_pages, picture, shown, to_user};
use crate::convert::matrix;
use crate::pages::{PagesError, Rect};
use crate::pdfium::{Box4, Geom, PdfiumError, lib};
use fit::{MIN_TEXT_PT, fit_scale};
use pdfium_render::prelude::{
    PdfColor, PdfDocument, PdfPageObject, PdfPageObjectCommon, PdfPageObjectsCommon, PdfPoints,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The job note of every signature placement (R132).
pub const SIGN_NOTE: &str =
    "this places a signature image or text; it is not a digital (cryptographic) signature";

/// Where a signature goes and how it shows.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    /// 0-based page index.
    pub page: usize,
    /// Box the signature is fitted and centred in: PDF user space points
    /// (origin bottom-left, y-up, unrotated MediaBox), inside the page's
    /// visible area.
    pub rect: Rect,
    /// Clockwise turn as the page is displayed: 0, 90, 180 or 270 degrees.
    #[serde(default)]
    pub rotate: u16,
    /// 0.0 (invisible) ..= 1.0 (opaque).
    #[serde(default = "opaque")]
    pub opacity: f32,
}

fn opaque() -> f32 {
    1.0
}

impl Placement {
    fn check(&self) -> Result<(), PdfiumError> {
        let r = self.rect;
        let finite = [r.left, r.bottom, r.right, r.top]
            .iter()
            .all(|v| v.is_finite());
        if !finite || r.left >= r.right || r.bottom >= r.top {
            return Err(PdfiumError::InvalidInput("signature rect is empty".into()));
        }
        if ![0, 90, 180, 270].contains(&self.rotate) {
            return Err(PdfiumError::InvalidInput(format!(
                "rotate must be 0, 90, 180 or 270 degrees, got {}",
                self.rotate
            )));
        }
        check_opacity(self.opacity)
    }
}

/// Places the PNG or JPEG `image` on page `at.page` of `input`: fitted
/// (scaled up or down, aspect kept) and centred in `at.rect` (PDF user
/// space points, unrotated MediaBox), upright as the page is displayed,
/// then turned `at.rotate` degrees clockwise, at `at.opacity`. PNG
/// transparency is kept; a JPEG is embedded as is, shown as its EXIF
/// Orientation says. Other pages are not
/// changed. Writes `out` copy-on-save and returns the path written.
///
/// # Errors
/// `InvalidInput` (bad paths, an image that is not a valid PNG or JPEG or is
/// over the 64 Mpx budget, an empty rect or one outside the page's visible
/// box, a bad turn or opacity), `PageOutOfRange`, `EncryptedInput`,
/// `Damaged`, `Pdfium`, `Io`.
pub fn sign_image(
    input: &Path,
    out: &Path,
    at: &Placement,
    image: &Path,
) -> Result<PathBuf, PagesError> {
    at.check()?;
    let mut pic = picture::read(image, "signature image")?;
    edit_pages(
        input,
        out,
        Some(&[at.page]),
        |_| (),
        |doc, page, _, i, g| {
            // EXIF Orientation first: `settle` fits the oriented bounds
            // (sides swapped for 5..=8).
            let o = pic.orientation();
            let (mut img, (w, h)) = pic.object(doc)?;
            let [a, b, c, d, e, f] = matrix(o, w as f32, h as f32);
            img.transform(a, b, c, d, e, f).map_err(lib)?;
            let obj = page.objects_mut().add_image_object(img).map_err(lib)?;
            settle(obj, at, i, g, (0.0, f32::INFINITY))
        },
    )
}

/// Places one line of `text` (standard-14 Helvetica, black) on page
/// `at.page` of `input` at `font_size` points, shrunk if it does not fit,
/// centred in `at.rect` (PDF user space points, unrotated MediaBox), upright
/// as the page is displayed, then turned `at.rotate` degrees clockwise, at
/// `at.opacity`. Other pages are not changed. Writes `out` copy-on-save and
/// returns the path written.
///
/// # Errors
/// `InvalidInput` (bad paths, empty text or a character outside WinAnsi, a
/// bad font size, text that does not fit even at 4 pt, an empty rect or one
/// outside the page's visible box, a bad turn or opacity), `PageOutOfRange`,
/// `EncryptedInput`, `Damaged`, `Pdfium`, `Io`.
pub fn sign_text(
    input: &Path,
    out: &Path,
    at: &Placement,
    text: &str,
    font_size: f32,
) -> Result<PathBuf, PagesError> {
    at.check()?;
    // Whitespace alone draws nothing: refused as empty.
    let shown_text = if text.trim().is_empty() { "" } else { text };
    check_text(shown_text, "signature text")?;
    check_size(font_size, "font size")?;
    let setup = |doc: &mut PdfDocument<'_>| doc.fonts_mut().helvetica();
    edit_pages(
        input,
        out,
        Some(&[at.page]),
        setup,
        |_, page, &font, i, g| {
            let size = PdfPoints::new(font_size);
            let objs = page.objects_mut();
            let obj = objs.create_text_object(PdfPoints::ZERO, PdfPoints::ZERO, text, font, size);
            let floor = (MIN_TEXT_PT / font_size).min(1.0);
            settle(obj.map_err(lib)?, at, i, g, (floor, 1.0))
        },
    )
}

/// Turns `obj` (drawn at the origin of page `i`, geometry `g`) by
/// `at.rotate`, scales it by the largest factor in `(min, max)` that fits
/// `at.rect` as displayed, centres it there, sets its opacity and maps it to
/// user space. Only text has a `min` above 0 (shrinks to 4 pt at most).
fn settle(
    mut obj: PdfPageObject<'_>,
    at: &Placement,
    i: usize,
    g: &Geom,
    (min, max): (f32, f32),
) -> Result<(), PdfiumError> {
    let r = at.rect;
    let (vl, vb, vr, vt) = g.visible;
    if r.left < vl || r.bottom < vb || r.right > vr || r.top > vt {
        return Err(PdfiumError::InvalidInput(format!(
            "signature rect is outside page {i}"
        )));
    }
    let (dl, db, dr, dt) = shown(g.rotation, g.visible, (r.left, r.bottom, r.right, r.top));
    obj.rotate_clockwise_degrees(f32::from(at.rotate))
        .map_err(lib)?;
    let s = fit_scale(ink(&obj)?, (dl, db, dr, dt), (min, max))?;
    obj.scale(s, s).map_err(lib)?;
    let (l, b, rt, t) = ink(&obj)?;
    let (dx, dy) = ((dl + dr - l - rt) / 2.0, (db + dt - b - t) / 2.0);
    obj.translate(PdfPoints::new(dx), PdfPoints::new(dy))
        .map_err(lib)?;
    let alpha = (at.opacity * 255.0).round() as u8;
    obj.set_fill_color(PdfColor::new(0, 0, 0, alpha))
        .map_err(lib)?;
    to_user(&mut obj, g)
}

/// Bounds of `obj` as `(left, bottom, right, top)` points.
fn ink(obj: &PdfPageObject<'_>) -> Result<Box4, PdfiumError> {
    let q = obj.bounds().map_err(lib)?;
    Ok((
        q.left().value,
        q.bottom().value,
        q.right().value,
        q.top().value,
    ))
}

#[cfg(test)]
mod tests;
