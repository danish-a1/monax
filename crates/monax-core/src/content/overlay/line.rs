//! One line of overlay text and the checks every overlay shares.
use crate::content::Anchor;
use crate::content::text::check_winansi;
use crate::pdfium::PdfiumError;

/// One line of Helvetica text on one page. `size` in points; `margin` in
/// points from the displayed page edges to the line box (ink width by the
/// font's ascent/descent); `angle` degrees counter-clockwise
/// as displayed; `opacity` 0.0..=1.0.
#[derive(Debug, Clone)]
pub(crate) struct Stamp {
    pub text: String,
    pub at: Anchor,
    pub margin: f32,
    pub size: f32,
    pub color: [u8; 3],
    pub opacity: f32,
    pub angle: f32,
}

impl Stamp {
    /// Opaque black, unrotated text.
    pub(crate) fn plain(text: String, at: Anchor, margin: f32, size: f32) -> Self {
        Self {
            text,
            at,
            margin,
            size,
            color: [0, 0, 0],
            opacity: 1.0,
            angle: 0.0,
        }
    }

    pub(super) fn check(&self) -> Result<(), PdfiumError> {
        let bad = |s: String| Err(PdfiumError::InvalidInput(s));
        check_size(self.size, "size")?;
        if !(self.margin.is_finite() && self.margin >= 0.0) {
            return bad(format!("margin must be >= 0 points, got {}", self.margin));
        }
        check_opacity(self.opacity)?;
        if !self.angle.is_finite() {
            return bad(format!("angle must be finite, got {}", self.angle));
        }
        check_text(&self.text, "overlay text")
    }
}

/// Fails on empty `text` (named `what` in the error) or a character
/// Helvetica (WinAnsi) cannot show.
pub(crate) fn check_text(text: &str, what: &str) -> Result<(), PdfiumError> {
    if text.is_empty() {
        return Err(PdfiumError::InvalidInput(format!("{what} is empty")));
    }
    check_winansi(text).map_err(PdfiumError::InvalidInput)
}

/// Fails unless `size` (named `what` in the error) is a positive number of
/// points.
pub(crate) fn check_size(size: f32, what: &str) -> Result<(), PdfiumError> {
    if size.is_finite() && size > 0.0 {
        return Ok(());
    }
    Err(PdfiumError::InvalidInput(format!(
        "{what} must be a positive number of points, got {size}"
    )))
}

/// Fails unless `opacity` is in 0.0..=1.0.
pub(crate) fn check_opacity(opacity: f32) -> Result<(), PdfiumError> {
    if (0.0..=1.0).contains(&opacity) {
        return Ok(());
    }
    Err(PdfiumError::InvalidInput(format!(
        "opacity must be in 0..=1, got {opacity}"
    )))
}
