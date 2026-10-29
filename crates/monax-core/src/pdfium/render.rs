//! Page rasterisation.
use super::{Checked, PdfiumError, fields, page_of, with_checked};
use crate::Secret;
use pdfium_render::prelude::{PdfPage, PdfRenderConfig};
use std::path::Path;

/// Upper bound on rendered pixels per page (64 Mpx = 256 MiB of RGBA).
pub(crate) const MAX_PIXELS: u64 = 64_000_000;

/// Straight (non-premultiplied) RGBA, row-major, `w * h * 4` bytes.
#[derive(Debug, Clone)]
pub struct Raster {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

impl Raster {
    /// Its pixels as RGB (alpha dropped: renders have a white background),
    /// `w * h * 3` bytes; the RGBA buffer is freed.
    pub(crate) fn rgb(self) -> Vec<u8> {
        let px = self.rgba.as_chunks::<4>().0.iter();
        px.flat_map(|p| [p[0], p[1], p[2]]).collect()
    }
}

/// Renders 0-based `page` as the user sees it at the pixel size `size` picks
/// from the displayed `(width, height)` in points. That size is pdfium's page
/// width and height, which equal `Geom::displayed` (CropBox clipped to the
/// MediaBox, inheritance honoured, swapped for /Rotate 90/270) without
/// `geometry`'s MediaBox lookup.
pub(super) fn raster(
    path: &Path,
    password: Option<&Secret>,
    page: usize,
    size: impl FnOnce(f32, f32) -> Result<(i32, i32), PdfiumError>,
) -> Result<Raster, PdfiumError> {
    let ok = fields::check(path, password)?;
    with_checked(&ok, password, |_, doc| {
        let p = page_of(doc, page)?;
        let (w, h) = size(p.width().value, p.height().value)?;
        render_page(&p, w, h, &ok)
    })
}

/// Renders `p` as the user sees it (/Rotate, CropBox, annotations and form
/// fields drawn, white background) at `w` x `h` pixels, once its document
/// passed the field check (R113).
pub(crate) fn render_page(
    p: &PdfPage<'_>,
    w: i32,
    h: i32,
    _: &Checked,
) -> Result<Raster, PdfiumError> {
    let cfg = PdfRenderConfig::new().set_target_size(w, h);
    let bmp = p
        .render_with_config(&cfg)
        .map_err(|_| PdfiumError::Damaged)?;
    Ok(Raster {
        w: bmp.width() as u32,
        h: bmp.height() as u32,
        rgba: bmp.as_rgba_bytes(),
    })
}

/// Pixel size of `(pw, ph)` points at `dpi`: `ceil(points * dpi / 72)` per
/// side, at least 1, at most 64 million pixels in all.
///
/// # Errors
/// `InvalidInput` (pixel budget).
pub(crate) fn px_size(pw: f32, ph: f32, dpi: u32) -> Result<(i32, i32), PdfiumError> {
    let scale = f64::from(dpi) / 72.0;
    let w = (f64::from(pw) * scale).ceil().max(1.0);
    let h = (f64::from(ph) * scale).ceil().max(1.0);
    if w * h > MAX_PIXELS as f64 {
        return Err(PdfiumError::InvalidInput(format!(
            "{w}x{h} px exceeds the {MAX_PIXELS} pixel budget"
        )));
    }
    Ok((w as i32, h as i32))
}

/// Renders 0-based `page` as the user sees it (applies /Rotate and CropBox) at
/// `dpi` (1..=1200). Output is `ceil(points * dpi / 72)` pixels per side and may
/// not exceed 64 million pixels.
///
/// # Errors
/// `InvalidInput` (bad path, dpi, or pixel budget), `Damaged`, `EncryptedInput`,
/// `PageOutOfRange`, `Library`, `FieldRevisit` (the form's field tree reaches
/// a field twice), `FieldDepth` (a field's /Parent chain is longer than 32),
/// `DamagedRepairFirst` (qpdf warns reading a document with a form),
/// `FieldCheck` (qpdf could not read a document with a form).
pub fn render(path: &Path, page: usize, dpi: u32) -> Result<Raster, PdfiumError> {
    if !(1..=1200).contains(&dpi) {
        return Err(PdfiumError::InvalidInput(format!(
            "dpi {dpi} not in 1..=1200"
        )));
    }
    raster(path, None, page, |pw, ph| px_size(pw, ph, dpi))
}
