//! pdfium access: process-wide lock, lazy init, typed errors.
//!
//! pdfium is not thread-safe. Every call (load, render, drop) runs inside
//! [`with_doc`] / [`with_engine`] while holding one global `Mutex`, and documents
//! are dropped before the guard is released. Page indices are 0-based; sizes are
//! PDF points (1/72 in); public coordinates follow PDF user space.
mod blank;
mod edit;
mod error;
pub(crate) mod fields;
mod geometry;
mod images;
mod info;
mod placed;
mod render;
mod text;
mod thumb;

pub(crate) use blank::blank_pdf;
pub(crate) use edit::{idx, lib, save};
pub use error::PdfiumError;
pub(crate) use error::out_of_range;
pub(crate) use fields::Checked;
pub(crate) use geometry::{Box4, Geom, geometry, pin_boxes, set_visible};
pub use images::{ExtractImagesInput, extract_images};
pub(crate) use info::has_security;
pub use info::{
    PageBoxes, PageInfo, crop_box, displayed_size, page_count, page_count_with_password, page_info,
    page_size, rotation,
};
pub(crate) use placed::{Extents, image_extents};
pub(crate) use render::{MAX_PIXELS, px_size, render_page};
pub use render::{Raster, render};
pub use text::extract_text;
pub(crate) use text::page_texts;
pub(crate) use thumb::encode as encode_png;
pub use thumb::{VIEW_MAX_PX, thumbnail_png, thumbnail_png_with_password, view_png};

use crate::Secret;
use crate::paths::check_input_file;
use pdfium_render::prelude::{
    PdfDocument, PdfPage, Pdfium, PdfiumError as Lib, PdfiumInternalError,
};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

static ENGINE: Mutex<Option<Pdfium>> = Mutex::new(None);

/// `MONAX_LIBPDFIUM` (`env`) when set, else the app bundle's
/// `Contents/Frameworks/libpdfium.dylib` beside `Contents/MacOS/<exe>`.
fn library_path(env: Option<OsString>, exe: Option<PathBuf>) -> Option<PathBuf> {
    match env {
        Some(p) => Some(p.into()),
        None => Some(exe?.parent()?.join("../Frameworks/libpdfium.dylib")),
    }
}

fn bind() -> Result<Pdfium, PdfiumError> {
    let path = library_path(
        std::env::var_os("MONAX_LIBPDFIUM"),
        std::env::current_exe().ok(),
    )
    .ok_or_else(|| PdfiumError::Library("no library path".into()))?;
    let bindings = Pdfium::bind_to_library(&path)
        .map_err(|e| PdfiumError::Library(format!("{}: {e}", path.display())))?;
    Ok(Pdfium::new(bindings))
}

/// Runs `f` with the initialised engine while holding the global lock.
pub(crate) fn with_engine<R>(
    f: impl FnOnce(&Pdfium) -> Result<R, PdfiumError>,
) -> Result<R, PdfiumError> {
    let mut guard = ENGINE.lock().unwrap_or_else(PoisonError::into_inner);
    if guard.is_none() {
        *guard = Some(bind()?);
    }
    match guard.as_ref() {
        Some(engine) => f(engine),
        None => Err(PdfiumError::Library("engine not initialised".into())),
    }
}

/// Binds the pdfium library once (from `MONAX_LIBPDFIUM`, else
/// `Contents/Frameworks/libpdfium.dylib` in the app bundle).
/// Every other function calls this lazily, so calling it is optional.
///
/// # Errors
/// [`PdfiumError::Library`] when the library is missing or cannot be bound.
pub fn init() -> Result<(), PdfiumError> {
    with_engine(|_| Ok(()))
}

/// A load failure: a password error is `WrongPassword` when one was given,
/// else `EncryptedInput`; anything else is `Damaged`.
pub(crate) fn map_load(e: Lib, with_password: bool) -> PdfiumError {
    match e {
        Lib::PdfiumLibraryInternalError(
            PdfiumInternalError::PasswordError | PdfiumInternalError::SecurityError,
        ) if with_password => PdfiumError::WrongPassword,
        Lib::PdfiumLibraryInternalError(
            PdfiumInternalError::PasswordError | PdfiumInternalError::SecurityError,
        ) => PdfiumError::EncryptedInput,
        _ => PdfiumError::Damaged,
    }
}

/// Validates `path`, loads it under the global lock and runs `f` with the
/// engine and the document. Readers and edits share it: an edit changes only
/// the in-memory document and returns bytes (see [`save`]); nothing is written
/// to disk here. The document is dropped before the lock is released.
///
/// # Errors
/// `InvalidInput` (bad path), `Damaged`, `EncryptedInput`, `Library`, or `f`'s error.
pub(crate) fn with_doc<R>(
    path: &Path,
    f: impl FnOnce(&Pdfium, &mut PdfDocument<'_>) -> Result<R, PdfiumError>,
) -> Result<R, PdfiumError> {
    with_doc_opened(path, None, f)
}

/// [`with_doc`] opening the document with `password` (a secret, handed to
/// pdfium in memory) when given; a rejected one is `WrongPassword`.
pub(crate) fn with_doc_opened<R>(
    path: &Path,
    password: Option<&Secret>,
    f: impl FnOnce(&Pdfium, &mut PdfDocument<'_>) -> Result<R, PdfiumError>,
) -> Result<R, PdfiumError> {
    check_input_file(path)?;
    let pw = password.map(Secret::expose);
    // ponytail: pdfium's OCG visibility check is quadratic in /OCGs, so a
    // crafted file with more than ~5k layers stalls the global lock. Upgrade
    // path: refuse more than about 5000 /OCGs entries, using the qpdf JSON
    // already read.
    with_engine(|engine| {
        let mut doc = engine
            .load_pdf_from_file(path, pw)
            .map_err(|e| map_load(e, pw.is_some()))?;
        f(engine, &mut doc)
    })
}

/// Loads the file `ok` passed the field check for ([`Checked::open`])
/// under the global lock and runs `f` with it, as [`with_doc`].
///
/// # Errors
/// As [`Checked::open`], or `f`'s error.
pub(crate) fn with_checked<R>(
    ok: &Checked,
    password: Option<&Secret>,
    f: impl FnOnce(&Pdfium, &mut PdfDocument<'_>) -> Result<R, PdfiumError>,
) -> Result<R, PdfiumError> {
    with_engine(|engine| f(engine, &mut ok.open(engine, password.map(Secret::expose))?))
}

pub(crate) fn count_of(doc: &PdfDocument<'_>) -> usize {
    usize::try_from(doc.pages().len()).unwrap_or(0)
}

/// Loads 0-based `page`, failing with [`PdfiumError::PageOutOfRange`] past the end.
pub(crate) fn page_of<'a>(
    doc: &'a PdfDocument<'_>,
    page: usize,
) -> Result<PdfPage<'a>, PdfiumError> {
    let count = count_of(doc);
    if page >= count {
        return Err(PdfiumError::PageOutOfRange { page, count });
    }
    let idx = i32::try_from(page).map_err(|_| PdfiumError::Damaged)?;
    doc.pages().get(idx).map_err(|_| PdfiumError::Damaged)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_path_prefers_env_then_bundle_frameworks() {
        let exe = Some(PathBuf::from("/A/Monax.app/Contents/MacOS/monax"));
        let fw = PathBuf::from("/A/Monax.app/Contents/MacOS/../Frameworks/libpdfium.dylib");
        assert_eq!(library_path(None, exe.clone()), Some(fw));
        let env = Some(OsString::from("/x/libpdfium.dylib"));
        assert_eq!(library_path(env, exe), Some("/x/libpdfium.dylib".into()));
        assert_eq!(library_path(None, None), None);
    }
}
