//! R113: pdfium loads a form's field tree with no visited set (depth cap
//! 32) when it first draws the form, so a /Kids DAG costs one walk per
//! path: exponential time. Every render is checked first: the tree is
//! walked once, through qpdf JSON, and refused when it reaches a field
//! twice; pdfium then draws the qpdf rewrite the check read (R118), so it
//! cannot resolve a reference into a tree the check did not see. Redaction
//! alone draws the original ([`original`], R119).
//! [`render_page`](super::render_page) takes the proof.
use super::{PdfiumError, with_doc_opened};
use crate::Secret;
use crate::content::qjson::{Doc, rewrite_intact};
use crate::output::private_dir;
use crate::pages::PagesError;
use pdfium_render::prelude::{PdfDocument, Pdfium};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

/// Proof that the document about to be drawn passed the field check, and
/// the file to draw: the input, or for a form qpdf's private rewrite of it,
/// held only as an open handle (R122: unlinked, so the OS reclaims it on
/// any exit).
pub(crate) enum Checked {
    Input(PathBuf),
    Copy(Arc<File>),
}

impl Checked {
    /// Loads the file checked with `engine` (`password` in memory). A copy
    /// is read through a clone of its handle: clones share one offset,
    /// which is safe because pdfium-render seeks to the absolute position
    /// before every block it reads, and pdfium runs under the global lock.
    ///
    /// # Errors
    /// `Io` (the handle cannot be cloned), the load's (`Damaged`,
    /// `EncryptedInput`, `WrongPassword`).
    pub(crate) fn open<'a>(
        &self,
        engine: &'a Pdfium,
        password: Option<&str>,
    ) -> Result<PdfDocument<'a>, PdfiumError> {
        match self {
            Checked::Input(p) => engine.load_pdf_from_file(p, password),
            Checked::Copy(f) => engine.load_pdf_from_reader(f.try_clone()?, password),
        }
        .map_err(|e| super::map_load(e, password.is_some()))
    }
}

/// A file's current contents: device, inode, size, mtime and ctime (s, ns).
type Stamp = (u64, u64, u64, i64, i64, i64, i64);

/// The last file that passed, with its form's private copy (none for no
/// form): thumbnails come page by page, so a form is rewritten once while
/// it is unchanged. Replacing the entry closes the old copy's handle once
/// no proof holds it. A form opened with a password is not cached: its
/// copy is served only to the call that gave it.
// ponytail: one entry. A file rewritten between the check and a no-form
// load is drawn unchecked: at worst slow, never wrong.
static PASSED: Mutex<Option<(Stamp, Option<Arc<File>>)>> = Mutex::new(None);

fn stamp(path: &Path) -> Result<Stamp, PdfiumError> {
    use std::os::unix::fs::MetadataExt;
    let m = std::fs::metadata(path)?;
    let (mt, ct) = ((m.mtime(), m.mtime_nsec()), (m.ctime(), m.ctime_nsec()));
    Ok((m.dev(), m.ino(), m.size(), mt.0, mt.1, ct.0, ct.1))
}

/// Checks `path` (opened with `password` when given) before it is drawn.
/// A document pdfium sees no form in is drawn as it is (and needs no
/// qpdf); one that passed and is unchanged is not loaded again. A form is
/// rewritten by qpdf (encryption kept) into a private temp file, which is
/// read and walked and is what pdfium then opens
/// ([`Checked::path`]). A form qpdf warns reading is refused: after an
/// xref rebuild its rewrite may hold other objects than the input shows.
///
/// # Errors
/// `FieldRevisit`, `FieldDepth`, `DamagedRepairFirst` (qpdf exits 3 or 2
/// reading it), `FieldCheck` (qpdf could not read or rewrite it; its
/// reason), and the pdfium load's: `InvalidInput`, `Damaged`, `EncryptedInput`,
/// `WrongPassword`, `Library`.
pub(crate) fn check(path: &Path, password: Option<&Secret>) -> Result<Checked, PdfiumError> {
    crate::check_input_file(path)?;
    let s = stamp(path)?;
    let passed = || PASSED.lock().unwrap_or_else(PoisonError::into_inner);
    let proof = |copy: Option<&Arc<File>>| match copy {
        Some(f) => Checked::Copy(Arc::clone(f)),
        None => Checked::Input(path.to_path_buf()),
    };
    match &*passed() {
        Some((hit, None)) if *hit == s => return Ok(proof(None)),
        Some((hit, Some(c))) if *hit == s && password.is_none() => return Ok(proof(Some(c))),
        _ => {}
    }
    if !with_doc_opened(path, password, |_, doc| Ok(doc.form().is_some()))? {
        *passed() = Some((s, None));
        return Ok(proof(None));
    }
    let fail = |e| match e {
        PagesError::DamagedRepairFirst => PdfiumError::DamagedRepairFirst,
        e => PdfiumError::FieldCheck(e.to_string()),
    };
    let dir = private_dir()?; // document data
    let copy = dir.path().join("form.pdf");
    let pw = password.map(Secret::expose);
    rewrite_intact(path, &copy, pw).map_err(fail)?;
    let json = private_dir()?; // its JSON, removed now
    walk::check_doc(
        &Doc::read_intact(&copy, json.path(), pw).map_err(fail)?,
        false,
    )?;
    // R122: kept only as an open handle; the file and its dir go now.
    let copy = Arc::new(File::open(&copy)?);
    dir.close()?;
    let ok = proof(Some(&copy));
    if password.is_none() {
        *passed() = Some((s, Some(copy)));
    }
    Ok(ok)
}

/// The redaction exception (R119), its only caller: proof that `path`
/// (read by qpdf as `doc`) passed the field check, `strict` (a reference
/// qpdf cannot resolve fails closed), for pdfium to open `path` ITSELF.
/// Redaction must draw what pdfium shows of the original (R115): a rewrite
/// could show a layer the original hides.
// ponytail: the walk costs widgets x /Parent chain length (1000 widgets x
// 10000 levels is about 19 s of the pdfium lock), and a /Parent behind a
// generation mismatch (see `walk`'s TODO) bypasses FieldDepth. Upgrade path:
// a raw scan of the file that refuses generation mismatches.
///
/// # Errors
/// As [`check`]'s walk, and `FieldCheck` for a reference qpdf cannot resolve.
pub(crate) fn original(path: &Path, doc: &Doc) -> Result<Checked, PdfiumError> {
    walk::check_doc(doc, true)?;
    Ok(Checked::Input(path.to_path_buf()))
}

mod walk;

#[cfg(test)]
mod tests;
