//! PNG page thumbnails.
use super::PdfiumError;
use super::render::raster;
use crate::Secret;
use std::path::Path;

/// Renders 0-based `page` as the user sees it (CropBox and /Rotate applied),
/// scaled so the longer side is `max_px` pixels (16..=1024), as PNG bytes.
///
/// # Errors
/// `InvalidInput` (bad path or `max_px`), `Damaged`, `EncryptedInput`,
/// `PageOutOfRange`, `Library`, `FieldRevisit`, `FieldCheck` (as
/// [`super::render`]).
pub fn thumbnail_png(path: &Path, page: usize, max_px: u32) -> Result<Vec<u8>, PdfiumError> {
    thumbnail_png_with_password(path, page, max_px, None)
}

/// [`thumbnail_png`] of a document opened with `password` (a secret, user
/// or owner; handed to pdfium in memory) when given.
///
/// # Errors
/// As [`thumbnail_png`], plus `WrongPassword`.
pub fn thumbnail_png_with_password(
    path: &Path,
    page: usize,
    max_px: u32,
    password: Option<&Secret>,
) -> Result<Vec<u8>, PdfiumError> {
    fit_png(path, page, max_px, 1024, password)
}

/// Largest `max_px` of [`view_png`].
pub const VIEW_MAX_PX: u32 = 4096;

/// [`thumbnail_png_with_password`] for the viewer, which renders at the
/// displayed device-pixel size: `max_px` 16..=[`VIEW_MAX_PX`].
///
/// # Errors
/// As [`thumbnail_png_with_password`].
pub fn view_png(
    path: &Path,
    page: usize,
    max_px: u32,
    password: Option<&Secret>,
) -> Result<Vec<u8>, PdfiumError> {
    fit_png(path, page, max_px, VIEW_MAX_PX, password)
}

/// Renders `page` with its longer side `max_px` (16..=`cap`) as PNG bytes.
fn fit_png(
    path: &Path,
    page: usize,
    max_px: u32,
    cap: u32,
    password: Option<&Secret>,
) -> Result<Vec<u8>, PdfiumError> {
    check_max_px(max_px, cap)?;
    let r = raster(path, password, page, |pw, ph| {
        let (pw, ph) = (pw.max(1.0), ph.max(1.0));
        let scale = max_px as f32 / pw.max(ph);
        let w = ((pw * scale).round() as i32).clamp(1, max_px as i32);
        let h = ((ph * scale).round() as i32).clamp(1, max_px as i32);
        Ok((w, h))
    })?;
    encode(r.w, r.h, &r.rgba)
}

/// `max_px` must be 16..=`cap`.
fn check_max_px(max_px: u32, cap: u32) -> Result<(), PdfiumError> {
    if (16..=cap).contains(&max_px) {
        return Ok(());
    }
    Err(PdfiumError::InvalidInput(format!(
        "max_px {max_px} not in 16..={cap}"
    )))
}

/// Encodes straight RGBA as 8-bit PNG bytes.
pub(crate) fn encode(w: u32, h: u32, rgba: &[u8]) -> Result<Vec<u8>, PdfiumError> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let io = |e: png::EncodingError| PdfiumError::Io(std::io::Error::other(e));
    enc.write_header()
        .and_then(|mut wr| wr.write_image_data(rgba))
        .map_err(io)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_px_outside_16_to_the_cap_is_invalid() {
        for bad in [15, VIEW_MAX_PX + 1] {
            let r = check_max_px(bad, VIEW_MAX_PX);
            assert!(matches!(r, Err(PdfiumError::InvalidInput(_))), "{bad}");
        }
        assert!(check_max_px(1025, 1024).is_err());
        assert!(check_max_px(16, VIEW_MAX_PX).is_ok());
        assert!(check_max_px(VIEW_MAX_PX, VIEW_MAX_PX).is_ok());
    }
}
