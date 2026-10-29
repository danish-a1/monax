//! Content operations: text overlays (page numbers, header/footer, watermark,
//! Bates numbers) stamped onto pages, and document-level rewrites (compress,
//! set metadata, flatten, delete annotations, repair, linearize; see each).
//!
//! Text is pdfium's standard-14 Helvetica (not embedded); characters outside
//! WinAnsi fail with `InvalidInput` naming the first one. Overlay strings
//! accept `%[page]` and `%[pages]` (see each op). Positions are an [`Anchor`]
//! plus a margin in points, resolved per page against the page as displayed
//! (visible box, /Rotate applied), so text is upright where the user sees it.
//! The margin is measured to the text's line box: its ink width by the
//! font's ascent/descent at the size, so lines of one size share a baseline.
//! `pages` are 0-based (`None` = all). Inputs are refused when encrypted or
//! damaged (`repair` takes damaged input); output is copy-on-save as in
//! [`crate::pages`] and errors are [`PagesError`].
mod annots;
mod compress;
mod images;
mod inputs;
mod jpeg;
mod marks;
mod meta;
mod numbers;
mod overlay;
pub(crate) mod picture;
pub(crate) mod qjson;
mod rewrite;
mod text;

pub use crate::pages::PagesError;
pub use annots::{delete_annotations, flatten};
pub use compress::{CompressOpts, compress, compress_with_notes};
pub use inputs::{
    BatesInput, CompressInput, DeleteAnnotationsInput, FlattenInput, HeaderFooterInput,
    LinearizeInput, PageNumbersInput, RepairInput, SetMetadataInput, WatermarkInput,
};
pub(crate) use jpeg::encode as encode_jpeg;
pub use marks::{HfOpts, Row, WatermarkOpts, header_footer, watermark};
pub use meta::{Metadata, set_metadata};
pub use numbers::{BatesOpts, NumberOpts, bates, page_numbers};
pub(crate) use overlay::{
    check_opacity, check_size, check_text, edit_pages, origin, shown, to_user,
};
pub(crate) use rewrite::qpdf_write;
pub use rewrite::{linearize, repair, repair_with_notes};
pub(crate) use text::{check_winansi, winansi_byte};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where overlay text sits on the page as displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    Center,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Anchor {
    /// `(x, y)` share of the free space left of / below the text: 0, 0.5 or 1.
    fn fractions(self) -> (f32, f32) {
        use Anchor::*;
        let x = match self {
            TopLeft | MiddleLeft | BottomLeft => 0.0,
            TopCenter | Center | BottomCenter => 0.5,
            TopRight | MiddleRight | BottomRight => 1.0,
        };
        let y = match self {
            BottomLeft | BottomCenter | BottomRight => 0.0,
            MiddleLeft | Center | MiddleRight => 0.5,
            TopLeft | TopCenter | TopRight => 1.0,
        };
        (x, y)
    }
}
