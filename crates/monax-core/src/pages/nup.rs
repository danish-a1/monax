//! N-up: several source pages tiled onto each sheet (pdfium).
use super::{PagesError, write_bytes};
use crate::pdfium::{lib, pin_boxes, save, with_doc};
use pdfium_render::prelude::{PdfPagePaperSize, PdfPoints};
use std::path::{Path, PathBuf};

/// `(rows, columns, landscape)` for a supported pages-per-sheet count.
fn grid(per_sheet: u8) -> Option<(u8, u8, bool)> {
    Some(match per_sheet {
        2 => (1, 2, true),
        4 => (2, 2, false),
        6 => (2, 3, true),
        9 => (3, 3, false),
        16 => (4, 4, false),
        _ => return None,
    })
}

/// Tiles the pages of `input` `per_sheet` to a sheet (2, 4, 6, 9 or 16), in
/// reading order, producing `ceil(pages / per_sheet)` sheets. The sheet is as
/// large as the first page as displayed (CropBox, /Rotate applied), in points,
/// turned landscape for 2 and 6 per sheet; pages shrink to fit their cell.
/// Returns the path written.
///
/// # Errors
/// `InvalidInput` (bad paths, unsupported `per_sheet`), `EncryptedInput`,
/// `Damaged`, `Pdfium`, `Io`.
pub fn nup(input: &Path, per_sheet: u8, out: &Path) -> Result<PathBuf, PagesError> {
    let Some((rows, cols, landscape)) = grid(per_sheet) else {
        return Err(PagesError::InvalidInput(format!(
            "pages per sheet must be 2, 4, 6, 9 or 16, got {per_sheet}"
        )));
    };
    let bytes = with_doc(input, |engine, doc| {
        let (mut w, mut h) = pin_boxes(engine, doc, 0)?.displayed();
        if landscape && w < h {
            (w, h) = (h, w);
        }
        let size = PdfPagePaperSize::from_points(PdfPoints::new(w), PdfPoints::new(h));
        let pages = doc.pages();
        let sheets = pages
            .tile_into_new_document(rows, cols, size)
            .map_err(lib)?;
        save(&sheets)
    })?;
    write_bytes(out, input, &bytes)
}
