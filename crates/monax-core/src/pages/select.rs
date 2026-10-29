//! Single-input page selection: extract, delete, reorder, rotate.
use super::{PagesError, check_pages, open, page_list, qpdf_op};
use crate::output::write_output;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Writes `input`'s 0-based `pages` (in the given order) to `out` via
/// `qpdf in --pages . list -- tmp`, keeping document-level data.
pub(super) fn select(input: &Path, pages: &[usize], out: &Path) -> Result<PathBuf, PagesError> {
    write_output(out, &[input], |tmp| {
        let args: [OsString; 6] = [
            input.into(),
            "--pages".into(),
            ".".into(),
            page_list(pages),
            "--".into(),
            tmp.into(),
        ];
        qpdf_op(&args)
    })
}

fn sorted(pages: &[usize]) -> Vec<usize> {
    let mut v = pages.to_vec();
    v.sort_unstable();
    v.dedup();
    v
}

/// Writes the 0-based `pages` of `input` to `out` in document order
/// (duplicates ignored). Returns the path written (see module docs).
///
/// # Errors
/// `InvalidInput` (bad paths, empty `pages`), `PageOutOfRange`,
/// `EncryptedInput`, `Damaged`, `Qpdf`, `Sidecar`, `Pdfium`, `Io`.
pub fn extract(input: &Path, pages: &[usize], out: &Path) -> Result<PathBuf, PagesError> {
    let count = open(input)?;
    check_pages(pages, count)?;
    select(input, &sorted(pages), out)
}

/// Writes `input` without its 0-based `pages` (duplicates ignored) to `out`.
///
/// # Errors
/// As [`extract`]; `InvalidInput` when every page would be deleted.
pub fn delete(input: &Path, pages: &[usize], out: &Path) -> Result<PathBuf, PagesError> {
    let count = open(input)?;
    check_pages(pages, count)?;
    let gone = sorted(pages);
    let keep: Vec<usize> = (0..count)
        .filter(|p| gone.binary_search(p).is_err())
        .collect();
    if keep.is_empty() {
        return Err(PagesError::InvalidInput("cannot delete every page".into()));
    }
    select(input, &keep, out)
}

/// Writes the 0-based pages of `input` in `order` to `out`; a page may repeat
/// and pages not listed are dropped.
///
/// # Errors
/// As [`extract`].
pub fn reorder(input: &Path, order: &[usize], out: &Path) -> Result<PathBuf, PagesError> {
    let count = open(input)?;
    check_pages(order, count)?;
    select(input, order, out)
}

/// Rotates the 0-based `pages` (duplicates ignored) clockwise by `degrees`,
/// relative to their current /Rotate (90 on a /Rotate 90 page gives 180).
/// `degrees` must be a non-zero multiple of 90; negative is counter-clockwise.
///
/// # Errors
/// As [`extract`]; `InvalidInput` for any other `degrees`.
pub fn rotate(
    input: &Path,
    pages: &[usize],
    degrees: i16,
    out: &Path,
) -> Result<PathBuf, PagesError> {
    if degrees == 0 || degrees % 90 != 0 {
        return Err(PagesError::InvalidInput(format!(
            "rotation must be a non-zero multiple of 90, got {degrees}"
        )));
    }
    let count = open(input)?;
    check_pages(pages, count)?;
    let mut arg = OsString::from(format!("--rotate=+{}:", degrees.rem_euclid(360)));
    arg.push(page_list(&sorted(pages)));
    write_output(out, &[input], |tmp| {
        qpdf_op(&[input.into(), arg, "--".into(), tmp.into()])
    })
}
