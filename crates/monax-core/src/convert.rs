//! Conversions between images and PDF: images become one page each
//! ([`images_to_pdf`]), pages become PNG or JPEG files ([`pdf_to_images`]);
//! Office documents become PDF through the bundled WordCraft sidecar
//! ([`office_to_pdf`]).
//! PDF to text is `read::ReadTextInput` (R136). Inputs are never modified;
//! outputs are copy-on-save (a taken name gets `-1`, `-2`, ...) and a
//! failed or cancelled conversion leaves no file.
mod images;
mod input;
mod office;
mod raster;

pub use images::images_to_pdf;
pub(crate) use images::matrix;
pub use input::{ImagesToPdfInput, OfficeToPdfInput, PdfToImagesInput};
pub use office::{office_available, office_to_pdf};
pub use raster::pdf_to_images;

use crate::pages::PagesError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A fixed page size in points (1/72 in); each side 72..=14400.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PageSize {
    /// Width in points, 72..=14400 (A4 portrait: 595.28, Letter: 612).
    pub width: f32,
    /// Height in points, 72..=14400 (A4 portrait: 841.89, Letter: 792).
    pub height: f32,
}

/// Page sides, points: PDF readers' practical range (1 in .. 200 in).
const SIDE: std::ops::RangeInclusive<f32> = 72.0..=14400.0;

impl PageSize {
    fn check(&self) -> Result<(), PagesError> {
        match [self.width, self.height]
            .into_iter()
            .find(|v| !SIDE.contains(v))
        {
            Some(v) => Err(PagesError::InvalidInput(format!(
                "page size {v} pt not in 72..=14400"
            ))),
            None => Ok(()),
        }
    }
}

/// Image file format of rendered pages.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    /// Lossless PNG (`.png`).
    #[default]
    Png,
    /// JPEG at `jpeg_quality` (`.jpg`).
    Jpeg,
}

#[cfg(test)]
mod tests;
