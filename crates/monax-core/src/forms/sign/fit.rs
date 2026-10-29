//! Fitting a signature's ink box into its rect (pure math, points).
use crate::pdfium::{Box4, PdfiumError};

/// Smallest font size signature text is shrunk to, points.
pub(super) const MIN_TEXT_PT: f32 = 4.0;

/// The largest factor in `(min, max)` that fits an `ink` box in a `room`
/// box (both `(left, bottom, right, top)` points).
///
/// # Errors
/// `InvalidInput` when `ink` has no positive width and height, or when even
/// `min` overflows `room` (only text has a `min` above 0).
pub(super) fn fit_scale(ink: Box4, room: Box4, (min, max): (f32, f32)) -> Result<f32, PdfiumError> {
    let (w, h) = (ink.2 - ink.0, ink.3 - ink.1);
    if !(w > 0.0 && h > 0.0) {
        return Err(PdfiumError::InvalidInput(
            "signature has nothing to draw".into(),
        ));
    }
    let s = ((room.2 - room.0) / w).min((room.3 - room.1) / h).min(max);
    if s < min {
        return Err(PdfiumError::InvalidInput(format!(
            "signature text does not fit in the rect even at {MIN_TEXT_PT} pt"
        )));
    }
    Ok(s)
}
