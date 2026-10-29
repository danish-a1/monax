//! R115: the final gate of a redaction. qpdf builds the output and pdfium
//! draws both files, so wherever the two read the input differently a kept
//! page of the output can draw other than in the input (content a layer
//! hid shown, a resource lost). Every kept page of both is drawn at 72 dpi
//! and must match pixel for pixel, or nothing is published.
use crate::pages::PagesError;
use crate::pdfium::{Checked, PdfiumError, count_of, page_of, px_size, render_page, with_engine};
use pdfium_render::prelude::PdfPage;

const DPI: u32 = 72;

/// Checks that every page of the input not in `redacted` draws exactly as
/// the same page of the output (same order, same count), each drawn from
/// the file itself (`ok`: input, output, from [`fields::original`], R119).
/// Both
/// stay open under the pdfium lock; one pair of pages is drawn at a time.
///
/// # Errors
/// `KeptPageChanged`; the loads' and renders' (`Damaged`, `Pdfium`).
pub(super) fn same_kept_pages(
    redacted: &[usize],
    ok: (&Checked, &Checked),
) -> Result<(), PagesError> {
    let size = |p: &PdfPage<'_>| px_size(p.width().value, p.height().value, DPI);
    let same = with_engine(|engine| {
        let (a, b) = (ok.0.open(engine, None)?, ok.1.open(engine, None)?);
        if count_of(&a) != count_of(&b) {
            return Ok(false);
        }
        for i in (0..count_of(&a)).filter(|i| !redacted.contains(i)) {
            let (pa, pb) = (page_of(&a, i)?, page_of(&b, i)?);
            let (sa, sb) = (size(&pa)?, size(&pb)?);
            if sa != sb
                || render_page(&pa, sa.0, sa.1, ok.0)?.rgba
                    != render_page(&pb, sb.0, sb.1, ok.1)?.rgba
            {
                return Ok::<_, PdfiumError>(false);
            }
        }
        Ok(true)
    })?;
    if !same {
        return Err(PagesError::KeptPageChanged);
    }
    Ok(())
}
