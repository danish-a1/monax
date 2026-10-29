//! Whole-file qpdf rewrites: repair and linearize.
use super::PagesError;
use crate::output::{private_dir, private_file, write_output};
use crate::pages::{open, qpdf_op, refuse_encrypted, write_bytes};
use crate::pdfium;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// `qpdf src args -- tmp`, published as `out` (copy-on-save; `out` may not
/// resolve to `input`, the file `src` derives from).
pub(crate) fn qpdf_write(
    input: &Path,
    src: &Path,
    mut args: Vec<OsString>,
    out: &Path,
) -> Result<PathBuf, PagesError> {
    write_output(out, &[input], |tmp| {
        args.insert(0, src.into());
        args.extend(["--".into(), tmp.into()]);
        qpdf_op(&args)
    })
}

/// Note of every repair (R114): qpdf's reading of a damaged file is what
/// the output shows.
pub(crate) const REPAIRED: &str = "repaired file may display differently from the damaged original";

/// Rewrites `input` with qpdf, which rebuilds a broken cross-reference table
/// and recovers what objects it can, then has pdfium load and save the
/// result (qpdf 12.4.2 leaves /Size out of a trailer it had to rebuild).
/// Unlike every other op it accepts input pdfium reports as damaged.
/// Returns the path written.
///
/// # Errors
/// `InvalidInput` (bad paths), `EncryptedInput`, `DamagedEncrypted` (an
/// encrypted file whose trailer is lost: never rebuilt as plain), `Qpdf`
/// (nothing recoverable; qpdf's reason), `Damaged` (the rewrite still does not load),
/// `Sidecar`, `Pdfium`, `Io`.
pub fn repair(input: &Path, out: &Path) -> Result<PathBuf, PagesError> {
    repair_with_notes(input, out).map(|(p, _)| p)
}

/// [`repair`], also returning the job notes: the repaired file may display
/// differently from the damaged original.
///
/// # Errors
/// As [`repair`].
pub fn repair_with_notes(input: &Path, out: &Path) -> Result<(PathBuf, Vec<String>), PagesError> {
    refuse_encrypted(input)?;
    let dir = private_dir()?; // document data
    let fixed = dir.path().join("fixed.pdf");
    private_file(&fixed)?;
    qpdf_op(&[input.into(), "--".into(), fixed.as_os_str().into()])?;
    let bytes = pdfium::with_doc(&fixed, |_, doc| pdfium::save(doc))?;
    Ok((
        write_bytes::<PagesError>(out, input, &bytes)?,
        vec![REPAIRED.into()],
    ))
}

/// Writes `input` linearized (web-optimized: the first page loads before the
/// rest of the file arrives) to `out`. Returns the path written.
///
/// # Errors
/// `InvalidInput` (bad paths), `EncryptedInput`, `Damaged`, `Qpdf`,
/// `Sidecar`, `Pdfium`, `Io`.
pub fn linearize(input: &Path, out: &Path) -> Result<PathBuf, PagesError> {
    open(input)?;
    qpdf_write(input, input, vec!["--linearize".into()], out)
}
