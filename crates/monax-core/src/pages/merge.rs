//! Multi-input ops: merge and alternate mix.
use super::{PagesError, open, qpdf_op};
use crate::output::{private_dir, private_file, write_output};
use crate::pdfium;
use crate::security::secret_file;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Options for [`merge`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct MergeOpts {
    /// Insert a blank page after every input with an odd page count (except the
    /// last input), so each next input starts on an odd page (a right-hand
    /// page in duplex printing).
    pub blank_before_even: bool,
}

/// One-page PDF `at` (a private file), blank, as large as `page` of `input`
/// looks on screen (CropBox clipped to MediaBox, width and height swapped
/// for /Rotate 90 or 270).
fn blank_like(input: &Path, page: usize, at: &Path) -> Result<(), PagesError> {
    let (w, h) = pdfium::displayed_size(input, page)?;
    private_file(at)?;
    std::fs::write(at, pdfium::blank_pdf(w, h)?)?;
    Ok(())
}

/// Concatenates every page of `inputs`, in order, into `out`. With
/// `blank_before_even`, a blank page sized like the preceding page (see
/// [`MergeOpts`]) follows each odd-length input but the last. Document-level
/// data (outlines, metadata) is not carried over. Returns the path written
/// (see module docs).
///
/// # Errors
/// `InvalidInput` (no inputs, bad paths), `EncryptedInput`, `Damaged`, `Qpdf`,
/// `Sidecar`, `Pdfium`, `Io`.
pub fn merge(inputs: &[PathBuf], out: &Path, opts: MergeOpts) -> Result<PathBuf, PagesError> {
    if inputs.is_empty() {
        return Err(PagesError::InvalidInput("no inputs to merge".into()));
    }
    let counts = inputs
        .iter()
        .map(|p| open(p))
        .collect::<Result<Vec<_>, _>>()?;
    // Blank pages (sized from document data): private, removed on drop,
    // after the qpdf run.
    let blanks = private_dir()?;
    let mut args: Vec<OsString> = vec!["--empty".into(), "--pages".into()];
    for (i, (input, &count)) in inputs.iter().zip(&counts).enumerate() {
        args.extend([input.into(), "1-z".into()]);
        if opts.blank_before_even && count % 2 == 1 && i + 1 < inputs.len() {
            let blank = blanks.path().join(format!("blank-{i}.pdf"));
            blank_like(input, count - 1, &blank)?;
            args.extend([blank.into(), "1".into()]);
        }
    }
    let refs: Vec<&Path> = inputs.iter().map(PathBuf::as_path).collect();
    write_output(out, &refs, |tmp| {
        args.extend(["--".into(), tmp.into()]);
        qpdf_op(&args)
    })
}

/// Interleaves `a` and `b` into `out`: a1, b1, a2, b2, ... with the longer
/// input's leftover pages appended. `reverse_b` takes `b` last page first (a
/// back-side scan). Returns the path written (see module docs).
///
/// # Errors
/// `InvalidInput` (bad paths), `EncryptedInput`, `Damaged`, `Qpdf`, `Sidecar`,
/// `Pdfium`, `Io`.
pub fn alternate_mix(
    a: &Path,
    b: &Path,
    reverse_b: bool,
    out: &Path,
) -> Result<PathBuf, PagesError> {
    open(a)?;
    open(b)?;
    let b_range = if reverse_b { "z-1" } else { "1-z" };
    write_output(out, &[a, b], |tmp| {
        let args: [OsString; 9] = [
            "--empty".into(),
            "--collate".into(),
            "--pages".into(),
            a.into(),
            "1-z".into(),
            b.into(),
            b_range.into(),
            "--".into(),
            tmp.into(),
        ];
        qpdf_op(&args)
    })
}

/// Joins every page of the PDFs `parts` (private temp files), in order,
/// into `out` (copy-on-save, checked against `inputs`); returns the path
/// written. The file list goes through a 0600 @argfile: argv has a size
/// limit.
///
/// # Errors
/// `InvalidInput` (bad `out`), `Qpdf`, `Sidecar`, `Io`.
pub(crate) fn join(parts: &[PathBuf], out: &Path, inputs: &[&Path]) -> Result<PathBuf, PagesError> {
    let mut lines: Vec<String> = vec!["--empty".into(), "--pages".into()];
    // ponytail: lossy, as a non-UTF-8 TMPDIR is not a real setup.
    lines.extend(parts.iter().map(|p| p.to_string_lossy().into_owned()));
    lines.push("--".into());
    let list = secret_file(&lines)?;
    let mut at = OsString::from("@");
    at.push(list.path());
    write_output(out, inputs, |tmp| qpdf_op(&[at, tmp.into()]))
}
