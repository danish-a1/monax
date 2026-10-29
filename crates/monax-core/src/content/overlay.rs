//! Overlay engine: draws text stamps (and, through [`edit_pages`], signature
//! images and text) straight onto the target pages with pdfium. The
//! original content streams are kept byte for byte; pdfium adds one new
//! stream per page, wrapped in `q`/`Q` and undoing any transform the
//! original content leaves behind, so the stamps land where they are placed.
//!
//! Not qpdf `--overlay`: qpdf 12.4.2 moves the original page content on
//! /Rotate pages whose CropBox origin is offset and on pages with inherited
//! boxes.
use crate::pages::{PagesError, check_pages, write_bytes};
use crate::pdfium::{Geom, PdfiumError, count_of, geometry, lib, page_of, save, with_doc};
use layout::{line_box, place};
pub(crate) use layout::{origin, shown};
pub(crate) use line::{Stamp, check_opacity, check_size, check_text};
use pdfium_render::prelude::{
    PdfColor, PdfDocument, PdfFontToken, PdfPage, PdfPageContentRegenerationStrategy as Regen,
    PdfPageObject, PdfPageObjectCommon, PdfPageObjectsCommon, PdfPoints,
};
use std::path::{Path, PathBuf};

/// Draws `s` on 0-based page `i` (geometry `g`): placed in displayed space,
/// then mapped to user space.
fn draw(
    page: &mut PdfPage<'_>,
    font: PdfFontToken,
    s: &Stamp,
    i: usize,
    g: &Geom,
) -> Result<(), PdfiumError> {
    s.check()?;
    let [r, gr, b] = s.color;
    let alpha = (s.opacity * 255.0).round() as u8;
    let pt = PdfPoints::new;
    let mut obj = page
        .objects_mut()
        .create_text_object(PdfPoints::ZERO, PdfPoints::ZERO, &s.text, font, pt(s.size))
        .map_err(lib)?;
    obj.set_fill_color(PdfColor::new(r, gr, b, alpha))
        .map_err(lib)?;
    // Vertical extent from the font's ascent/descent, so every line at a
    // size shares a baseline; horizontal extent from the ink.
    let flat = obj.bounds().map_err(lib)?;
    let ink_x = (flat.left().value, flat.right().value);
    let not_text = || PdfiumError::Library("created object is not text".into());
    let metrics = {
        let f = obj.as_text_object().ok_or_else(not_text)?.font();
        (
            f.descent(pt(s.size)).map_err(lib)?.value,
            f.ascent(pt(s.size)).map_err(lib)?.value,
        )
    };
    let (l, b, r, t) = line_box(ink_x, metrics, s.angle);
    obj.rotate_counter_clockwise_degrees(s.angle).map_err(lib)?;
    let (w, h) = g.displayed();
    let at = place(s.at, s.margin, (w, h), (r - l, t - b));
    let (dx, dy) = at.map_or((0.0, 0.0), |(x, y)| (x - l, y - b));
    // Fit check on the placed ink box: no glyph may leave the page.
    let q = obj.bounds().map_err(lib)?;
    let (il, ib) = (q.left().value + dx, q.bottom().value + dy);
    let (ir, it) = (q.right().value + dx, q.top().value + dy);
    if at.is_none() || il < 0.0 || ib < 0.0 || ir > w || it > h {
        return Err(PdfiumError::InvalidInput(format!(
            "overlay text does not fit on page {i} at size {}",
            s.size
        )));
    }
    obj.translate(pt(dx), pt(dy)).map_err(lib)?;
    to_user(&mut obj, g)
}

/// Moves `obj`, placed in the displayed space of a page with geometry `g`
/// (points from the displayed bottom-left), to that page's user space.
pub(crate) fn to_user(obj: &mut PdfPageObject<'_>, g: &Geom) -> Result<(), PdfiumError> {
    let (ox, oy) = origin(g.rotation, g.visible);
    obj.rotate_counter_clockwise_degrees(f32::from(g.rotation))
        .map_err(lib)?;
    obj.translate(PdfPoints::new(ox), PdfPoints::new(oy))
        .map_err(lib)
}

/// Draws the stamps `make(k, page, count)` returns for the k-th selected page
/// (0-based `pages`, sorted and deduplicated; `None` = all; `count` = the
/// document's page count) onto `input`, on top of the page content. Writes
/// `out` copy-on-save and returns the path written.
///
/// # Errors
/// `InvalidInput` (bad paths, empty `pages`, a bad stamp, text that does not
/// fit), `PageOutOfRange`, `EncryptedInput`, `Damaged`, `Pdfium`, `Io`, or
/// `make`'s error.
pub(crate) fn stamp(
    input: &Path,
    out: &Path,
    pages: Option<&[usize]>,
    make: impl Fn(usize, usize, usize) -> Result<Vec<Stamp>, PdfiumError>,
) -> Result<PathBuf, PagesError> {
    let mut k = 0;
    let setup = |doc: &mut PdfDocument<'_>| (doc.fonts_mut().helvetica(), count_of(doc));
    edit_pages(input, out, pages, setup, |_, page, &(font, count), i, g| {
        for s in make(k, i, count)? {
            draw(page, font, &s, i, g)?;
        }
        k += 1;
        Ok(())
    })
}

/// The one stamping path: `setup` runs once on `input`'s document, then
/// `draw(doc, page, setup's value, i, geometry)` adds objects to each
/// selected page `i` (0-based `pages`, sorted and deduplicated; `None` =
/// all), in order. A page's original content streams are kept byte for
/// byte; its new objects go in one new stream. Writes `out` copy-on-save and
/// returns the path written.
///
/// # Errors
/// `InvalidInput` (bad paths, empty `pages`), `PageOutOfRange`,
/// `EncryptedInput`, `Damaged`, `Pdfium`, `Io`, or `draw`'s error.
pub(crate) fn edit_pages<T>(
    input: &Path,
    out: &Path,
    pages: Option<&[usize]>,
    setup: impl FnOnce(&mut PdfDocument<'_>) -> T,
    mut draw: impl for<'a, 'b> FnMut(
        &'a PdfDocument<'b>,
        &mut PdfPage<'a>,
        &T,
        usize,
        &Geom,
    ) -> Result<(), PdfiumError>,
) -> Result<PathBuf, PagesError> {
    let bytes = with_doc(input, |engine, doc| {
        let count = count_of(doc);
        let sel = match pages {
            None => (0..count).collect(),
            Some(p) => {
                check_pages(p, count)?;
                let mut v = p.to_vec();
                v.sort_unstable();
                v.dedup();
                v
            }
        };
        let t = setup(doc);
        for i in sel {
            let g = geometry(engine, doc, i)?;
            let mut page = page_of(doc, i)?;
            page.set_content_regeneration_strategy(Regen::Manual);
            draw(doc, &mut page, &t, i, &g)?;
            page.regenerate_content().map_err(lib)?;
        }
        save(doc)
    })?;
    write_bytes(out, input, &bytes)
}

mod layout;
mod line;
#[cfg(test)]
mod tests;
