//! Annotation ops through pdfium: flatten and delete. Page content streams
//! are not regenerated, only the page's /Annots change (flatten appends one
//! stream drawing the appearances).
use super::PagesError;
use crate::pages::{check_pages, write_bytes};
use crate::pdfium::{
    Checked, PdfiumError, count_of, fields, lib, page_of, render_page, save, with_checked,
};
use pdfium_render::prelude::{
    PdfDocument, PdfPageAnnotationType as Kind, PdfPageContentRegenerationStrategy as Regen,
};
use std::path::{Path, PathBuf};

/// Removes the annotations of 0-based `page` whose type `drop` selects.
pub(super) fn remove_annots(
    doc: &PdfDocument<'_>,
    page: usize,
    drop: impl Fn(Kind) -> bool,
    _: &Checked,
) -> Result<(), PdfiumError> {
    let mut p = page_of(doc, page)?;
    p.set_content_regeneration_strategy(Regen::Manual);
    let annots = p.annotations_mut();
    for i in (0..annots.len()).rev() {
        let a = annots.get(i).map_err(lib)?;
        if drop(a.annotation_type()) {
            annots.delete_annotation(a).map_err(lib)?;
        }
    }
    Ok(())
}

/// Draws the printable annotations and form fields of every page into the
/// page content as pdfium displays them (pdfium `FPDFPage_Flatten`) and
/// removes every annotation from the pages, so nothing can be edited or
/// clicked any more: links and hidden or non-printing annotations go without
/// a trace. Returns the path written.
///
/// # Errors
/// `InvalidInput` (bad paths), `EncryptedInput`, `Damaged`, `Pdfium` (among
/// them `FieldRevisit`: the form's field tree reaches a field twice), `Io`.
pub fn flatten(input: &Path, out: &Path) -> Result<PathBuf, PagesError> {
    let ok = fields::check(input, None)?;
    let bytes = with_checked(&ok, None, |_, doc| {
        for i in 0..count_of(doc) {
            let mut page = page_of(doc, i)?;
            // Rendering makes pdfium write the appearance streams it would
            // generate for display (e.g. note icons) into the annotations,
            // so flatten draws what the viewer shows.
            render_page(&page, 1, 1, &ok)?;
            page.flatten().map_err(lib)?;
        }
        save(doc)
    })?;
    write_bytes(out, input, &bytes)
}

/// Removes all non-form annotations (comments, highlights and other markup,
/// links, stamps, media) from the 0-based `pages` (`None` = all; duplicates
/// ignored). Form field widgets stay, with their values; other pages are
/// unchanged. Returns the path written.
///
/// # Errors
/// `InvalidInput` (bad paths, empty `pages`), `PageOutOfRange`,
/// `EncryptedInput`, `Damaged`, `Pdfium` (among them `FieldRevisit`: the
/// form's field tree reaches a field twice), `Io`.
pub fn delete_annotations(
    input: &Path,
    out: &Path,
    pages: Option<&[usize]>,
) -> Result<PathBuf, PagesError> {
    let ok = fields::check(input, None)?; // the widgets build pdfium's form
    let bytes = with_checked(&ok, None, |_, doc| {
        let all: Vec<usize> = (0..count_of(doc)).collect();
        let sel = pages.unwrap_or(&all);
        check_pages(sel, all.len())?;
        for &i in sel {
            remove_annots(
                doc,
                i,
                |k| !matches!(k, Kind::Widget | Kind::XfaWidget),
                &ok,
            )?;
        }
        save(doc)
    })?;
    write_bytes(out, input, &bytes)
}
