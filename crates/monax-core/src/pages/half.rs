//! Split every page into its left and right halves (pdfium).
use super::{PagesError, write_bytes};
use crate::pdfium::{self, Box4, PdfiumError, idx, lib, pin_boxes, save, set_visible, with_doc};
use std::path::{Path, PathBuf};

/// The two halves of `b` in reading order, split along what the viewer sees
/// for a page turned `rot` degrees clockwise. Unrotated: left | right. At 90
/// the unrotated bottom edge is displayed on the left, so left | right is
/// bottom | top; 180 swaps the unrotated sides; 270 is top | bottom.
fn halves((l, bo, r, t): Box4, rot: u16) -> [Box4; 2] {
    let (mx, my) = ((l + r) / 2.0, (bo + t) / 2.0);
    let (left, right) = ((l, bo, mx, t), (mx, bo, r, t));
    let (low, high) = ((l, bo, r, my), (l, my, r, t));
    match rot {
        90 => [low, high],
        180 => [right, left],
        270 => [high, low],
        _ => [left, right],
    }
}

/// Replaces every page with two copies, the left then the right half of what
/// is displayed (the CropBox when present, else the MediaBox; for /Rotate 90
/// or 270 pages the halves follow the rotated view, see `halves`). Each half
/// is the page with its CropBox set to that half, in PDF user space points, so
/// page content is shared, not cut. Returns the path written.
///
/// # Errors
/// `InvalidInput` (bad paths, too many pages), `EncryptedInput`,
/// `Damaged`, `Pdfium`, `Io`.
pub fn split_half(input: &Path, out: &Path) -> Result<PathBuf, PagesError> {
    let bytes = with_doc(input, |engine, doc| {
        let src = engine
            .load_pdf_from_file(input, None)
            .map_err(|_| PdfiumError::Damaged)?;
        let n = usize::try_from(src.pages().len()).unwrap_or(0);
        let mut boxes = Vec::with_capacity(2 * n);
        for i in 0..n {
            // Pin inherited boxes on the source page so each copy carries them.
            let g = pin_boxes(engine, &src, i)?;
            boxes.extend(halves(g.visible, g.rotation));
            doc.pages_mut()
                .copy_page_from_document(&src, idx(i)?, idx(2 * i + 1)?)
                .map_err(lib)?;
        }
        for (j, rect) in boxes.into_iter().enumerate() {
            let mut page = pdfium::page_of(doc, j)?;
            set_visible(&mut page, rect)?;
        }
        save(doc)
    })?;
    write_bytes(out, input, &bytes)
}
