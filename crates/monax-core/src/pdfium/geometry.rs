//! Page geometry with inheritance: the one source of truth for page boxes.
use super::{PdfiumError, idx, lib, page_of};
use pdfium_render::prelude::{PdfDocument, PdfPage, PdfRect, Pdfium};

/// `(left, bottom, right, top)` in PDF user space points.
pub(crate) type Box4 = (f32, f32, f32, f32);

/// Boxes of one page in unrotated PDF user space points.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Geom {
    /// MediaBox (own or inherited), normalized so left < right, bottom < top.
    pub media: Box4,
    /// CropBox (own or inherited) clipped to the MediaBox: what is displayed.
    pub visible: Box4,
    /// /Rotate, clockwise degrees: 0, 90, 180 or 270.
    pub rotation: u16,
}

impl Geom {
    /// Displayed `(width, height)` in points: the visible box, swapped for
    /// /Rotate 90 or 270.
    pub(crate) fn displayed(&self) -> (f32, f32) {
        let (l, b, r, t) = self.visible;
        match self.rotation {
            90 | 270 => (t - b, r - l),
            _ => (r - l, t - b),
        }
    }
}

fn norm(r: PdfRect) -> Box4 {
    let (l, b, rt, t) = (
        r.left().value,
        r.bottom().value,
        r.right().value,
        r.top().value,
    );
    (l.min(rt), b.min(t), l.max(rt), b.max(t))
}

fn rect((l, b, r, t): Box4) -> PdfRect {
    PdfRect::new_from_values(b, l, t, r)
}

/// Page `rotation` in degrees clockwise (0, 90, 180 or 270).
pub(crate) fn degrees(page: &PdfPage<'_>) -> Result<u16, PdfiumError> {
    use pdfium_render::prelude::PdfPageRenderRotation as R;
    Ok(match page.rotation().map_err(|_| PdfiumError::Damaged)? {
        R::None => 0,
        R::Degrees90 => 90,
        R::Degrees180 => 180,
        R::Degrees270 => 270,
    })
}

/// Reads the boxes of 0-based `i`, following /Pages inheritance (pdfium's
/// page-level MediaBox getter does not). A pure query: `doc` is not changed.
/// An inherited MediaBox is read from a copy of the page in a throwaway
/// document, where pdfium writes inherited boxes onto the page.
///
/// # Errors
/// `PageOutOfRange`, `Damaged`, `Library`.
pub(crate) fn geometry(
    engine: &Pdfium,
    doc: &PdfDocument<'_>,
    i: usize,
) -> Result<Geom, PdfiumError> {
    let page = page_of(doc, i)?;
    let bd = |_| PdfiumError::Damaged;
    let visible = norm(page.boundaries().bounding().map_err(bd)?.bounds);
    let media = match page.boundaries().media() {
        Ok(m) => norm(m.bounds),
        Err(_) => {
            let mut scratch = engine.create_new_pdf().map_err(lib)?;
            scratch
                .pages_mut()
                .copy_page_from_document(doc, idx(i)?, 0)
                .map_err(lib)?;
            norm(
                page_of(&scratch, 0)?
                    .boundaries()
                    .media()
                    .map_err(bd)?
                    .bounds,
            )
        }
    };
    Ok(Geom {
        media,
        visible,
        rotation: degrees(&page)?,
    })
}

/// Edit ops only: reads [`geometry`] of 0-based `i`, then writes explicit
/// page-level boxes where they are inherited (MediaBox; CropBox when it is
/// inherited and differs from the MediaBox), so output saved from `doc` is
/// unambiguous. Returns the geometry, unchanged by the write.
///
/// # Errors
/// As [`geometry`].
pub(crate) fn pin_boxes(
    engine: &Pdfium,
    doc: &PdfDocument<'_>,
    i: usize,
) -> Result<Geom, PdfiumError> {
    let g = geometry(engine, doc, i)?;
    let mut page = page_of(doc, i)?;
    let own_media = page.boundaries().media().is_ok();
    let own_crop = page.boundaries().crop().is_ok();
    if !own_media {
        page.boundaries_mut()
            .set_media(rect(g.media))
            .map_err(lib)?;
    }
    if !own_media || (!own_crop && g.visible != g.media) {
        set_visible(&mut page, g.visible)?;
    }
    Ok(g)
}

/// Sets the explicit CropBox of `page` to `b`.
pub(crate) fn set_visible(page: &mut PdfPage<'_>, b: Box4) -> Result<(), PdfiumError> {
    page.boundaries_mut().set_crop(rect(b)).map_err(lib)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdfium::with_doc;
    use std::path::Path;

    #[test]
    fn reading_inherited_boxes_does_not_write_page_boxes() {
        let lib = std::env::var_os("MONAX_LIBPDFIUM");
        if !lib.is_some_and(|p| Path::new(&p).is_file()) {
            eprintln!("SKIPPED: MONAX_LIBPDFIUM not set or library missing");
            return;
        }
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/inherited-boxes.pdf");
        with_doc(&path, |engine, doc| {
            let g = geometry(engine, doc, 0)?;
            assert_eq!(g.media, (0., 0., 300., 400.));
            assert_eq!(g.visible, (10., 10., 200., 300.));
            let page = page_of(doc, 0)?;
            assert!(page.boundaries().media().is_err(), "MediaBox written");
            assert!(page.boundaries().crop().is_err(), "CropBox written");
            Ok(())
        })
        .unwrap();
    }
}
