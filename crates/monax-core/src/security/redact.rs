//! redact: rasterize + flatten (spec decision 3). Each redacted page is
//! replaced by a 300 dpi image of itself with the rects burned in (`burn`).
//! The output holds only its catalog (cut to pages, form and the layers
//! kept pages use) and its pages (R103, R104): kept pages with what they
//! reach, and the rasters. In a private staged copy, every object only
//! redacted pages reach, and what describes them, is set to null first, so
//! no kept reference (a reply, an action, a form field) can carry it;
//! other objects naming a redacted page only lose those entries (`clean`). The output is searched
//! for words of the redacted pages before it is published (`check`, simple
//! fonts only).
mod burn;
mod check;
mod clean;
mod gate;

use crate::content::qjson::Doc;
use crate::output::{check_out, private_dir, private_file, write_output};
use crate::pages::{PagesError, Rect, open, page_list, qpdf_op};
use crate::pdfium::{Box4, fields};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// One area to black out.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Redaction {
    /// 0-based page index.
    pub page: usize,
    /// Area to black out: PDF user space points, origin bottom-left, y-up,
    /// unrotated MediaBox. The part outside the visible page is ignored.
    pub rect: Rect,
}

/// Writes `input` to `out` with every page named in `boxes` replaced by a
/// raster of itself: rendered at 300 dpi as displayed (/Rotate and CropBox
/// applied, annotations and form fields drawn), each of its rects (user
/// space points, unrotated MediaBox; clipped to the visible box) filled
/// solid black, embedded as a JPEG that is the only content of an
/// unrotated page of the displayed size. The original page,
/// with its content streams, the resources no kept page draws, its
/// annotations, links, form fields, /Thumb and page metadata, is gone from
/// the output; text on it is no longer searchable. The output holds only
/// its pages and what kept pages reach; what only redacted pages reach,
/// and what describes one (a struct element, bookmark, destination,
/// action), is nulled first, whatever else refers to it; any other object
/// naming one only loses those entries; a form field left without widgets
/// is removed. Every document-level feature (tags, bookmarks, named
/// destinations, document JavaScript, attachments, metadata, ...) is
/// dropped but the layers kept pages use, with their default states (so
/// hidden content stays hidden); links to a redacted page lose their
/// target. What a kept page
/// also uses (a shared form, image, field value) stays. Other pages keep
/// their content. Before the output is published, the strings it draws are
/// searched for words (4+ ASCII alphanumerics) only the redacted pages
/// showed; text in CID fonts and appearances is not covered by that check.
/// Then every kept page is drawn from the input and the output (never from
/// qpdf's rewrite of a form, R119) and must match pixel for pixel. Returns
/// the path written.
///
/// # Errors
/// `InvalidInput` (bad paths, no boxes, a rect empty or not finite, or
/// outside its page's visible box, a page too large to rasterize),
/// `PageOutOfRange`, `RedactionLeak` and `KeptPageChanged` (R115: a kept
/// page of the output would not draw exactly as in the input, at 72 dpi;
/// nothing written either way), `EncryptedInput`,
/// `Damaged`, `DamagedRepairFirst` (qpdf warns or fails reading the
/// input: `repair` it first), `DamagedNesting` (its qpdf JSON nests too
/// deep to read whole), `Qpdf`, `Sidecar`, `Pdfium` (among them
/// `FieldRevisit`: the form's field tree reaches a field twice), `Io`.
pub fn redact(input: &Path, out: &Path, boxes: &[Redaction]) -> Result<PathBuf, PagesError> {
    redact_with_notes(input, out, boxes).map(|(p, _)| p)
}

/// [`redact`], also returning the job notes: how many pages lost their
/// text, that tags were removed (when there were any), that links on kept
/// pages pointed at a redacted page (when any did), which document-level
/// features were removed, that layers only redacted pages used were, that
/// kept fields lost a form font (when so), and that what kept pages share
/// is not redacted.
///
/// # Errors
/// As [`redact`].
pub fn redact_with_notes(
    input: &Path,
    out: &Path,
    boxes: &[Redaction],
) -> Result<(PathBuf, Vec<String>), PagesError> {
    if boxes.is_empty() {
        return Err(PagesError::InvalidInput("no redactions given".into()));
    }
    let mut pages: BTreeMap<usize, Vec<Box4>> = BTreeMap::new();
    for Redaction { page, rect: r } in boxes {
        let finite = [r.left, r.bottom, r.right, r.top]
            .iter()
            .all(|v| v.is_finite());
        if !finite || r.left >= r.right || r.bottom >= r.top {
            return Err(PagesError::InvalidInput(format!(
                "redaction rect on page {page} is empty"
            )));
        }
        pages
            .entry(*page)
            .or_default()
            .push((r.left, r.bottom, r.right, r.top));
    }
    let count = open(input)?;
    check_out(out, &[input])?; // before the costly renders
    if let Some(&page) = pages.keys().find(|&&p| p >= count) {
        return Err(PagesError::PageOutOfRange { page, count });
    }
    let pages: Vec<_> = pages.into_iter().collect();
    let redacted: Vec<usize> = pages.iter().map(|(p, _)| *p).collect();
    let dir = private_dir()?; // document data: private, like `Unlocked`
    let doc = Doc::read_intact(input, dir.path(), None)?;
    let ok = fields::original(input, &doc)?; // R119: never qpdf's rewrite
    let (updates, mut notes) = clean::plan(&doc, &redacted)?;
    let secret = check::secret_words(input, &redacted)?;
    let raster = dir.path().join("raster.pdf");
    private_file(&raster)?;
    std::fs::write(&raster, burn::burn(&pages, &ok)?)?;
    // The plan nulls what only redacted pages own (and what describes
    // them), cuts the catalog to its pages, form and kept layers and the
    // trailer to its root; the output then holds the staged catalog and the
    // pages alone (R103, R104), so nothing else of the input is carried.
    let staged = dir.path().join("staged.pdf");
    private_file(&staged)?;
    let update = doc.update_arg(updates, dir.path())?;
    qpdf_op(&[input.into(), update, staged.clone().into()])?;
    // Strips from kept pages' resources what only the redacted pages drew
    // ("auto" only does so for resources it sees shared).
    let mut args: Vec<OsString> = vec![staged.into(), "--remove-unreferenced-resources=yes".into()];
    args.extend(page_specs(Path::new("."), &raster, count, &redacted));
    let path = write_output(out, &[input], |tmp| {
        args.extend(["--".into(), tmp.into()]);
        qpdf_op(&args)?;
        check::verify(tmp, &secret, dir.path())?;
        let ok_out = fields::original(tmp, &Doc::read_intact(tmp, dir.path(), None)?)?;
        gate::same_kept_pages(&redacted, (&ok, &ok_out))
    })?;
    let searchable = format!(
        "text on {} redacted page(s) is no longer searchable",
        redacted.len()
    );
    notes.insert(0, searchable);
    Ok((path, notes))
}

/// qpdf `--pages` arguments: every page of `input` in order, the `redacted`
/// ones (sorted) taken from `raster` instead, one per page. qpdf copies a
/// page with what it reaches (not other pages) and the form fields of its
/// widgets.
fn page_specs(input: &Path, raster: &Path, count: usize, redacted: &[usize]) -> Vec<OsString> {
    let mut runs: Vec<(&Path, Vec<usize>)> = Vec::new();
    for i in 0..count {
        let (file, page) = match redacted.binary_search(&i) {
            Ok(k) => (raster, k),
            Err(_) => (input, i),
        };
        match runs.last_mut() {
            Some((f, v)) if *f == file => v.push(page),
            _ => runs.push((file, vec![page])),
        }
    }
    let mut args = vec![OsString::from("--pages")];
    for (file, v) in runs {
        args.extend([file.into(), page_list(&v)]);
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_specs_interleave_runs_of_kept_and_raster_pages() {
        let (i, r) = (Path::new("/in.pdf"), Path::new("/r.pdf"));
        let s = |v: &[&str]| v.iter().map(OsString::from).collect::<Vec<_>>();
        assert_eq!(
            page_specs(i, r, 5, &[1, 2, 4]),
            s(&[
                "--pages", "/in.pdf", "1", "/r.pdf", "1,2", "/in.pdf", "4", "/r.pdf", "3"
            ])
        );
        assert_eq!(page_specs(i, r, 1, &[0]), s(&["--pages", "/r.pdf", "1"]));
    }
}
